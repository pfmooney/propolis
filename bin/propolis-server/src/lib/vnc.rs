// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use propolis::hw::ps2::ctrl::PS2Ctrl;
use propolis::hw::qemu::ramfb::{Frame, FramebufferSpec, RamFb};

use futures::StreamExt;
use rfb::encodings::{EncodingType, RawEncoding};
use rfb::pixel_formats::fourcc;
use rfb::rfb::{
    ClientMessage, FramebufferUpdate, FramebufferUpdateRequest, Position,
    ProtoVersion, ProtocolError, Rectangle, Resolution, SecurityType,
    SecurityTypes,
};
use slog::{error, trace, Logger};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Notify};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio_util::codec::FramedRead;

/// Arbitrary maximum valid resolution
const MAX_RES: Resolution = Resolution { width: 1920, height: 1200 };
const UNINIT_RES: Resolution = Resolution { width: 1024, height: 768 };
const UNINIT_FOURCC: u32 = fourcc::FOURCC_XR24;
const SERVER_NAME: &str = "propolis-vnc";
/// Frame interval (in us) for 10fps
const FRAME_US_10FPS: usize = 1000000 / 10;

struct Devices {
    keyboard: Arc<PS2Ctrl>,
    display: Arc<RamFb>,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum FrameKind {
    Valid,
    Generated,
}

#[derive(Default)]
struct State {
    devices: Option<Devices>,
    is_stopped: bool,
}

#[derive(Default)]
struct ClientState {
    last_frame: Option<(Frame, FrameKind)>,
    fbu_req: Option<FramebufferUpdateRequest>,
    encodings: BTreeSet<EncodingType>,
}

#[derive(Default)]
pub struct Client {
    hup: Option<oneshot::Sender<()>>,
    id: Option<String>,
}

pub struct VncServer {
    state: Mutex<State>,
    client: Mutex<Client>,
    notify: Notify,
    /// Minimum frame interval (in us)
    frame_int_us: usize,
    log: Logger,
}

#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("Invalid FourCC {0}")]
    InvalidFourCC(u32),
    #[error("VNC initialization error {0:?}")]
    InitError(#[from] rfb::server::InitError),
    #[error("VNC server is stopped")]
    ServerStopped,
}

/// Alias trait to cut down on verbosity
pub trait Connection: AsyncRead + AsyncWrite + Unpin + Send + 'static {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> Connection
    for rfb::tungstenite::BinaryWs<T>
{
}
impl Connection for tokio::net::TcpStream {}
impl Connection for Box<dyn Connection> {}

impl VncServer {
    pub fn new(log: Logger) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            client: Mutex::new(Client::default()),
            notify: Notify::new(),
            frame_int_us: FRAME_US_10FPS,
            log,
        })
    }
    pub fn attach(&self, ps2: Arc<PS2Ctrl>, fb: Arc<RamFb>) {
        let mut state = self.state.lock().unwrap();
        state.devices = Some(Devices { keyboard: ps2, display: fb });
    }
    pub async fn connect(
        self: &Arc<Self>,
        mut conn: impl Connection,
        client_id: String,
    ) -> Result<(), ConnectError> {
        let (resolution, fourcc) = {
            let state = self.state.lock().unwrap();
            if state.is_stopped {
                return Err(ConnectError::ServerStopped);
            }

            state
                .devices
                .as_ref()
                .map(|devs| devs.display.read_spec())
                .filter(spec_valid)
                .map(|spec| {
                    (
                        Resolution {
                            width: spec.width as u16,
                            height: spec.height as u16,
                        },
                        spec.fourcc,
                    )
                })
                .unwrap_or((UNINIT_RES, UNINIT_FOURCC))
        };

        let format = fourcc::fourcc_to_pixel_format(fourcc)
            .ok_or(ConnectError::InvalidFourCC(fourcc))?;

        let _client_init = rfb::server::initialize(
            &mut conn,
            rfb::server::InitParams {
                version: ProtoVersion::Rfb38,
                // vncviewer won't work without offering VncAuth, even though it
                // doesn't ask to use it.
                sec_types: SecurityTypes(vec![
                    SecurityType::None,
                    SecurityType::VncAuthentication,
                ]),
                name: SERVER_NAME.to_string(),
                resolution,
                format,
            },
        )
        .await?;

        let hup_recv = self.replace_client(client_id).await;

        let this = self.clone();
        tokio::spawn(async move {
            if let Err(e) = this.run(conn, hup_recv).await {
                error!(this.log, "VNC error, hanging up: {:?}", e);
            }
            this.hup_client();
        });

        Ok(())
    }

    async fn replace_client(&self, new_id: String) -> oneshot::Receiver<()> {
        let mut client = self.wait_client_gone().await;

        let (send, recv) = oneshot::channel();
        client.id = Some(new_id);
        client.hup = Some(send);

        recv
    }
    fn hup_client(&self) {
        let mut client = self.client.lock().unwrap();
        client.hup.take();
        client.id.take();
        self.notify.notify_one();
    }
    async fn wait_client_gone(&self) -> MutexGuard<Client> {
        loop {
            {
                let mut client = self.client.lock().unwrap();
                // tell any existing client to hang up
                if let Some(hup) = client.hup.take() {
                    let _ = hup.send(());
                }
                // and once it is gone, go on to install ourself as active
                if client.id.is_none() {
                    return client;
                }
                drop(client);
            }

            self.notify.notified().await;
        }
    }

    async fn run(
        &self,
        conn: impl Connection,
        mut close_recv: oneshot::Receiver<()>,
    ) -> Result<(), ProtocolError> {
        let mut decoder =
            FramedRead::new(conn, rfb::rfb::ClientMessageDecoder::default());
        let mut cstate: ClientState = Default::default();
        loop {
            tokio::select! {
                biased;

                _ = &mut close_recv => {
                    return Ok(());
                },
                msg = decoder.next() => {
                    let msg = match msg {
                        Some(Err(e)) => {
                            return Err(e);
                        }
                        None => {
                            // Client disconnect
                            return Ok(());
                        }
                        Some(Ok(m)) => m,
                    };
                    self.handle_msg(decoder.get_mut(), msg, &mut cstate).await;
                }
                _ = self.wait_for_next_frame(&mut cstate) => {
                    self.send_fbu(decoder.get_mut(), &mut cstate).await?;
                }
            }
        }
    }

    async fn handle_msg(
        &self,
        _conn: &mut impl Connection,
        msg: ClientMessage,
        cstate: &mut ClientState,
    ) {
        match msg {
            ClientMessage::KeyEvent(ke) => {
                let state = self.state.lock().unwrap();
                trace!(self.log, "VNC key event: {:?}", ke);
                if let Some(devs) = state.devices.as_ref() {
                    devs.keyboard.key_event(ke);
                }
            }
            ClientMessage::PointerEvent(pe) => {
                trace!(self.log, "VNC pointer event: {:?}", pe);
                // TODO: wire to tablet device
            }
            ClientMessage::ClientCutText(_) => {
                trace!(self.log, "Ignoring VNC CutText request");
            }
            ClientMessage::FramebufferUpdateRequest(req) => {
                cstate.fbu_req = Some(req);
            }
            ClientMessage::SetPixelFormat(pf) => {
                // TODO: actually handle pixel format stuff
                slog::warn!(self.log, "Unhandled SetPixelFormat({:?})", pf);
            }
            ClientMessage::SetEncodings { encodings, unknown } => {
                cstate.encodings = encodings.into_iter().collect();
                slog::trace!(self.log, "SetEncodings({:?})", cstate.encodings);
                if !unknown.is_empty() {
                    slog::debug!(
                        self.log,
                        "Unrecognized SetEncodings values: {:?}",
                        unknown
                    );
                }
            }
        }
    }
    async fn send_fbu(
        &self,
        conn: &mut impl Connection,
        cstate: &mut ClientState,
    ) -> Result<(), ProtocolError> {
        let fbu = {
            let (frame, _kind) = cstate.last_frame.as_ref().unwrap();
            let r = Rectangle {
                position: Position { x: 0, y: 0 },
                dimensions: Resolution {
                    width: frame.spec.width as u16,
                    height: frame.spec.height as u16,
                },
                data: Box::new(RawEncoding::new(frame.data.clone())),
            };
            FramebufferUpdate(vec![r])
        };
        fbu.write_to(conn).await?;
        conn.flush().await?;

        // With the FBU sent, the existing request is fulfilled
        cstate.fbu_req = None;

        Ok(())
    }

    fn update_frame(&self, cstate: &mut ClientState) -> bool {
        let state = self.state.lock().unwrap();

        if let Some(new_valid_frame) = state.devices.as_ref().and_then(|devs| {
            devs.display.read_framebuffer(|spec| {
                if spec_valid(spec) {
                    // Only currently accepted fourcc is xRGB (32bpp)
                    Some(32)
                } else {
                    None
                }
            })
        }) {
            cstate.last_frame = Some((new_valid_frame, FrameKind::Valid));
            true
        } else {
            match cstate.last_frame.as_ref() {
                Some((_, FrameKind::Generated)) => {
                    // Reuse existing generated frame
                    false
                }
                _ => {
                    // Fill out a blank frame if none is already in place
                    cstate.last_frame =
                        Some((blank_frame(), FrameKind::Generated));
                    true
                }
            }
        }
    }
    async fn wait_for_next_frame(&self, cstate: &mut ClientState) {
        if cstate.fbu_req.is_none() {
            // If an update has not been requested, we will wait indefinitely
            futures::future::pending::<()>().await;
        }

        loop {
            let wait_len_us = match cstate
                .last_frame
                .as_ref()
                .map(|(frame, kind)| (kind, frame.when.elapsed()))
            {
                None | Some((FrameKind::Generated, _)) => {
                    // If there is no previous frame, or the existing frame is a
                    // generated blank, do not delay in attempting an update.
                    if self.update_frame(cstate) {
                        return;
                    }
                    // If the update resulted in no change, wait the default
                    // interval to check again
                    self.frame_int_us as u64
                }
                Some((FrameKind::Valid, age)) => {
                    let since_last = age.as_micros() as usize;
                    if since_last >= self.frame_int_us {
                        self.update_frame(cstate);
                        return;
                    }
                    (self.frame_int_us - since_last) as u64
                }
            };
            sleep(Duration::from_micros(wait_len_us)).await
        }
    }

    pub async fn stop(&self) {
        {
            let mut state = self.state.lock().unwrap();
            state.is_stopped = true;
            state.devices = None;
        }

        let _client = self.wait_client_gone().await;
    }
}

/// TCP socket listener for VNC client connections
pub struct TcpSock {
    join_hdl: JoinHandle<()>,
    hup_send: oneshot::Sender<()>,
}
impl TcpSock {
    pub async fn new(
        vnc: Arc<VncServer>,
        addr: SocketAddr,
        log: Logger,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let (hup_send, hup_recv) = oneshot::channel::<()>();
        let join_hdl = tokio::spawn(async move {
            Self::run(listener, vnc, hup_recv, log).await;
        });
        Ok(Self { join_hdl, hup_send })
    }
    pub async fn halt(self) {
        let Self { join_hdl, hup_send } = self;

        // Signal the socket listener to hang up, then wait for it to bail
        let _ = hup_send.send(());
        let _ = join_hdl.await;
    }
    async fn run(
        listener: TcpListener,
        vnc: Arc<VncServer>,
        mut hup_recv: oneshot::Receiver<()>,
        log: Logger,
    ) {
        loop {
            tokio::select! {
                biased;

                _ = &mut hup_recv => {
                    return;
                },
                sock_res = listener.accept() => {
                    match sock_res {
                        Ok((sock, addr)) => {
                            let conn_res = vnc.connect(
                                Box::new(sock) as Box<dyn Connection + 'static>,
                                addr.to_string(),
                            )
                            .await;
                            if let Err(e) = conn_res {
                                error!(&log, "Error during VNC connection: {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!(&log, "VNC TCP listener error: {:?}", e);
                        }
                    }
                },
            };
        }
    }
}

/// Generate a black "filler" frame of default size/format
fn blank_frame() -> Frame {
    // Generate a new "filler" frame, if one isn't already in place
    let width = UNINIT_RES.width as usize;
    let height = UNINIT_RES.height as usize;
    // Black (0) pixels at 32bpp
    let mut data = Vec::new();
    data.resize_with(width * height * 4, Default::default);

    Frame {
        spec: FramebufferSpec {
            width,
            height,
            stride: 0,
            fourcc: UNINIT_FOURCC,
        },
        data,
        when: Instant::now(),
    }
}

/// Check that FB spec is what we consider valid:
/// - Of an appropriate size (not zero or > 1920x1200
/// - Using the xRGB pixel format
fn spec_valid(spec: &FramebufferSpec) -> bool {
    (0..=(MAX_RES.width as usize)).contains(&spec.width)
        && (0..=(MAX_RES.height as usize)).contains(&spec.height)
        && spec.fourcc == fourcc::FOURCC_XR24
}
