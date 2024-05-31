// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright 2022 Oxide Computer Company

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{bail, Result};
use clap::Parser;
use futures::StreamExt;
use slog::info;
use tokio::net::TcpListener;
use tokio_util::codec::FramedRead;

use rfb::rfb::{
    ColorFormat, ColorSpecification, PixelFormat, ProtoVersion, Resolution,
    SecurityType, SecurityTypes,
};
use rfb::{self, pixel_formats::rgb_888, rfb::ClientMessageDecoder};

mod shared;
use shared::{order_to_shift, ExampleBackend, Image};

const WIDTH: usize = 1024;
const HEIGHT: usize = 768;

#[derive(Parser, Debug)]
/// A simple VNC server that displays a single image or color, in a given pixel format
///
/// By default, the server will display the Oxide logo image using little-endian RGBx as its pixel format. To specify an alternate image or color, use the `-i` flag:
/// ./example-server -i test-tubes
/// ./example-server -i red
///
/// To specify an alternate pixel format, use the `--big-endian` flag and/or the ordering flags. The
/// server will transform the input image/color to the requested pixel format and use the format
/// for the RFB protocol.
///
/// For example, to use big-endian xRGB:
/// ./example-server --big-endian true -r 1 -g 2 -b 3
///
struct Args {
    /// Image/color to display from the server
    #[clap(value_enum, short, long, default_value_t = Image::Oxide)]
    image: Image,

    /// Pixel endianness
    #[clap(long, default_value_t = false, action = clap::ArgAction::Set)]
    big_endian: bool,

    /// Byte mapping to red (4-byte RGB pixel, endian-agnostic)
    #[clap(short, long, default_value_t = 0)]
    red_order: u8,

    /// Byte mapping to green (4-byte RGB pixel, endian-agnostic)
    #[clap(short, long, default_value_t = 1)]
    green_order: u8,

    /// Byte mapping to blue (4-byte RGB pixel, endian-agnostic)
    #[clap(short, long, default_value_t = 2)]
    blue_order: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let log = shared::build_logger();

    let args = Args::parse();
    validate_order(args.red_order, args.green_order, args.blue_order)?;

    let pf = PixelFormat {
        bits_per_pixel: rgb_888::BITS_PER_PIXEL,
        depth: rgb_888::DEPTH,
        big_endian: args.big_endian,
        color_spec: ColorSpecification::ColorFormat(ColorFormat {
            red_max: rgb_888::MAX_VALUE,
            green_max: rgb_888::MAX_VALUE,
            blue_max: rgb_888::MAX_VALUE,
            red_shift: order_to_shift(args.red_order),
            green_shift: order_to_shift(args.green_order),
            blue_shift: order_to_shift(args.blue_order),
        }),
    };
    info!(
        log,
        "Starting server: image: {:?}, pixel format; {:#?}", args.image, pf
    );

    let backend = ExampleBackend {
        display: args.image,
        rgb_order: (args.red_order, args.green_order, args.blue_order),
        big_endian: args.big_endian,
    };

    let listener = TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        9000,
    ))
    .await
    .unwrap();

    loop {
        let (mut sock, addr) = listener.accept().await.unwrap();

        info!(log, "New connection from {:?}", addr);
        let log_child = log.new(slog::o!("sock" => addr));

        let init_res = rfb::server::initialize(
            &mut sock,
            rfb::server::InitParams {
                version: ProtoVersion::Rfb38,

                sec_types: SecurityTypes(vec![
                    SecurityType::None,
                    SecurityType::VncAuthentication,
                ]),

                name: "rfb-example-server".to_string(),

                resolution: Resolution {
                    width: WIDTH as u16,
                    height: HEIGHT as u16,
                },
                format: pf.clone(),
            },
        )
        .await;

        if let Err(e) = init_res {
            slog::info!(log_child, "Error during client init {:?}", e);
            continue;
        }

        let be_clone = backend.clone();
        let input_pf = pf.clone();
        tokio::spawn(async move {
            let mut output_pf = input_pf.clone();
            let mut decoder =
                FramedRead::new(sock, ClientMessageDecoder::default());
            loop {
                let msg = match decoder.next().await {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        slog::info!(
                            log_child,
                            "Error reading client msg: {:?}",
                            e
                        );
                        return;
                    }
                    None => {
                        return;
                    }
                };
                let sock = decoder.get_mut();

                use rfb::rfb::ClientMessage;

                match msg {
                    ClientMessage::SetPixelFormat(out_pf) => {
                        output_pf = out_pf;
                    }
                    ClientMessage::FramebufferUpdateRequest(_req) => {
                        let fbu = be_clone.generate(WIDTH, HEIGHT).await;

                        let fbu = fbu.transform(&input_pf, &output_pf);

                        if let Err(e) = fbu.write_to(sock).await {
                            slog::info!(
                                log_child,
                                "Error sending FrambufferUpdate: {:?}",
                                e
                            );
                            return;
                        }
                    }
                    _ => {
                        slog::debug!(log_child, "RX: Client msg {:?}", msg);
                    }
                }
            }
        });
    }
}

fn validate_order(r: u8, g: u8, b: u8) -> Result<()> {
    if r > 3 || g > 3 || b > 3 {
        bail!("r/g/b must have ordering of 0, 1, 2, or 3");
    }

    if r == g || r == b || g == b {
        bail!("r/g/b must have unique orderings");
    }

    Ok(())
}
