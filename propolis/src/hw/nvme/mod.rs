use std::sync::{Arc, Mutex};

use crate::common::*;
use crate::dispatch::DispCtx;
use crate::hw::pci;
use crate::util::regmap::RegMap;

use lazy_static::lazy_static;

mod bits;
mod queue;

use bits::*;

#[derive(Default)]
struct CtrlState {
    enabled: bool,
    ready: bool,
    admin_subq_base: u64,
    admin_compq_base: u64,
    admin_subq_size: u16,
    admin_compq_size: u16,
}

#[derive(Default)]
struct NvmeState {
    ctrl: CtrlState,
}

#[derive(Default)]
pub struct PciNvme {
    state: Mutex<NvmeState>,
}

impl PciNvme {
    pub fn create(vendor: u16, device: u16) -> Arc<pci::DeviceInst> {
        let builder = pci::Builder::new(pci::Ident {
            vendor_id: vendor,
            device_id: device,
            sub_vendor_id: vendor,
            sub_device_id: device,
            class: pci::bits::CLASS_STORAGE,
            subclass: pci::bits::SUBCLASS_NVM,
            prog_if: pci::bits::PROGIF_ENTERPRISE_NVMHCI,
            ..Default::default()
        });

        builder
            // XXX: add room for doorbells
            .add_bar_mmio64(pci::BarN::BAR0, CONTROLLER_REG_SZ as u64)
            // BAR0/1 are used for the main config and doorbell registers
            // BAR2 is for the optional index/data registers
            // Place MSIX in BAR4 for now
            .add_cap_msix(pci::BarN::BAR4, 1024)
            .finish(Arc::new(PciNvme::default()))
    }

    fn ctrlr_cfg_write(&self, val: u32) {
        let mut state = self.state.lock().unwrap();

        if !state.ctrl.enabled {
            // TODO: apply any necessary config changes
        }

        let now_enabled = val & CC_EN != 0;
        if now_enabled && !state.ctrl.enabled {
            state.ctrl.enabled = true;
            // TODO: actual enabling
            //
            // - setup admin queues
            //
        } else if !now_enabled && state.ctrl.enabled {
            state.ctrl.enabled = false;
            state.ctrl.ready = false;
            // TODO: actual disabling
            //
            // When this field transitions from ‘1’ to ‘0’, the controller is
            // reset (referred to as a Controller Reset).  The reset deletes all
            // I/O Submission Queues and I/O Completion Queues, resets the Admin
            // Submission Queue and Completion Queue, and brings the hardware to
            // an idle state.  The reset does not affect PCI Express registers
            // nor the Admin Queue registers (AQA, ASQ, or ACQ).  All other
            // controller registers defined in this section and internal
            // controller state (e.g., Feature values defined in section 5.12.1
            // that are not persistent across power states) are reset to their
            // default values.  The controller shall ensure that there is no
            // data loss for commands  that have had corresponding completion
            // queue entries posted to an I/O Completion Queue prior to the
            // reset operation.
        }
    }
}
impl PciNvme {
    fn reg_ctrl_read(&self, id: &CtrlrReg, ro: &mut ReadOp, _ctx: &DispCtx) {
        match id {
            CtrlrReg::CtrlrCaps => {
                // MPSMIN = MPSMAX = 0 (4k pages)
                // CCS = 0x1 - NVM command set
                // DSTRD = 0 - standard (32-bit) doorbell stride
                // TO = 0 - 0 * 500ms to wait for controller ready
                // AMS = 0x0 - no additional abitrary mechs (besides RR)
                // CQR = 0x1 - contig queues required for now
                // MQES = 0xfff - 4k (zeros-based)
                ro.write_u64(CAP_CCS | CAP_CQR | 0x0fff);
            }
            CtrlrReg::Version => {
                ro.write_u32(NVME_VER_1_0);
            }

            CtrlrReg::IntrMaskSet | CtrlrReg::IntrMaskClear => {
                // Only MSI-X is exposed for now, so this is undefined
                ro.fill(0);
            }

            CtrlrReg::CtrlrCfg => {
                let state = self.state.lock().unwrap();
                let mut val = if state.ctrl.enabled { 1 } else { 0 };
                val |= 4 << 20 // IOCQES 23:20 - 2^4 = 16 bytes
                | 6 << 16; // IOSQES 19:16 - 2^6 = 64 bytes
                ro.write_u32(val);
            }
            CtrlrReg::CtrlrStatus => {
                let state = self.state.lock().unwrap();
                let mut val = 0;

                if state.ctrl.ready {
                    val |= CSTS_READY;
                }
                ro.write_u32(val);
            }
            CtrlrReg::AdminQueueAttr => {
                let state = self.state.lock().unwrap();
                ro.write_u32(
                    state.ctrl.admin_subq_size as u32
                        | (state.ctrl.admin_compq_size as u32) << 16,
                );
            }
            CtrlrReg::AdminSubQAddr => {
                let state = self.state.lock().unwrap();
                ro.write_u64(state.ctrl.admin_subq_base);
            }
            CtrlrReg::AdminCompQAddr => {
                let state = self.state.lock().unwrap();
                ro.write_u64(state.ctrl.admin_compq_base);
            }
            CtrlrReg::Reserved => {
                ro.fill(0);
            }
            CtrlrReg::DoorBellSubQ0 | CtrlrReg::DoorBellCompQ0 => {
                // The host should not read from the doorbells, and the contents
                // can be vendor/implementation specific (in our case, zeroed).
                ro.fill(0);
            }
        }
    }
    fn reg_ctrl_write(&self, id: &CtrlrReg, wo: &mut WriteOp, _ctx: &DispCtx) {
        match id {
            CtrlrReg::CtrlrCaps
            | CtrlrReg::Version
            | CtrlrReg::CtrlrStatus
            | CtrlrReg::Reserved => {
                // Read-only registers
            }
            CtrlrReg::IntrMaskSet | CtrlrReg::IntrMaskClear => {
                // Only MSI-X is exposed for now, so this is undefined
            }

            CtrlrReg::CtrlrCfg => {
                self.ctrlr_cfg_write(wo.read_u32());
            }
            CtrlrReg::AdminQueueAttr => {
                let mut state = self.state.lock().unwrap();
                if !state.ctrl.enabled {
                    let val = wo.read_u32();
                    // bits 27:16 - ACQS, zeroes-based
                    let compq: u16 = ((val >> 16) & 0xfff) as u16 + 1;
                    // bits 27:16 - ASQS, zeroes-based
                    let subq: u16 = (val & 0xfff) as u16 + 1;

                    state.ctrl.admin_compq_size = compq;
                    state.ctrl.admin_subq_size = subq;
                }
            }
            CtrlrReg::AdminSubQAddr => {
                let mut state = self.state.lock().unwrap();
                if !state.ctrl.enabled {
                    state.ctrl.admin_subq_base =
                        wo.read_u64() & PAGE_MASK as u64;
                }
            }
            CtrlrReg::AdminCompQAddr => {
                let mut state = self.state.lock().unwrap();
                if !state.ctrl.enabled {
                    state.ctrl.admin_compq_base =
                        wo.read_u64() & PAGE_MASK as u64;
                }
            }

            CtrlrReg::DoorBellSubQ0 => {
                todo!("ring admin subq doorbell");
            }
            CtrlrReg::DoorBellCompQ0 => {
                todo!("ring admin compq doorbell");
            }
        }
    }
}

impl pci::Device for PciNvme {
    fn bar_rw(&self, bar: pci::BarN, mut rwo: RWOp, ctx: &DispCtx) {
        assert_eq!(bar, pci::BarN::BAR0);
        CONTROLLER_REGS.process(&mut rwo, |id, rwo| match rwo {
            RWOp::Read(ro) => self.reg_ctrl_read(id, ro, ctx),
            RWOp::Write(wo) => self.reg_ctrl_write(id, wo, ctx),
        });
    }

    fn attach(
        &self,
        lintr_pin: Option<pci::INTxPin>,
        msix_hdl: Option<pci::MsixHdl>,
    ) {
        assert!(lintr_pin.is_none());
        assert!(msix_hdl.is_none());
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum CtrlrReg {
    CtrlrCaps,
    Version,
    IntrMaskSet,
    IntrMaskClear,
    CtrlrCfg,
    CtrlrStatus,
    AdminQueueAttr,
    AdminSubQAddr,
    AdminCompQAddr,
    Reserved,

    // XXX: single doorbell for prototype
    DoorBellSubQ0,
    DoorBellCompQ0,
}
// XXX: single doorbell for prototype
const CONTROLLER_REG_SZ: usize = 0x2000;
lazy_static! {
    static ref CONTROLLER_REGS: RegMap<CtrlrReg> = {
        let layout = [
            (CtrlrReg::CtrlrCaps, 8),
            (CtrlrReg::Version, 4),
            (CtrlrReg::IntrMaskSet, 4),
            (CtrlrReg::IntrMaskClear, 4),
            (CtrlrReg::CtrlrCfg, 4),
            (CtrlrReg::Reserved, 4),
            (CtrlrReg::CtrlrStatus, 4),
            (CtrlrReg::Reserved, 4),
            (CtrlrReg::AdminQueueAttr, 4),
            (CtrlrReg::AdminSubQAddr, 8),
            (CtrlrReg::AdminCompQAddr, 8),
            (CtrlrReg::Reserved, 0xec8),
            (CtrlrReg::Reserved, 0x100),
            // XXX: hardcode a single doorbell with 0 stride for now
            (CtrlrReg::DoorBellSubQ0, 4),
            (CtrlrReg::DoorBellCompQ0, 4),
            // XXX: pad out to next power of 2
            (CtrlrReg::Reserved, 0x1000 - 8),
        ];
        RegMap::create_packed(
            CONTROLLER_REG_SZ,
            &layout,
            Some(CtrlrReg::Reserved),
        )
    };
}

enum AdminCmd {
    DeleteIoSubQ,
    CreateIoSubQ,
    GetLogPage,
    DeleteIoCompQ,
    CreateIoCompQ,
    Identify,
    Abort,
    SetFeatures,
    GetFeatures,
    AsyncEventReq,
}
impl AdminCmd {
    const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode {
            0x0 => Some(AdminCmd::DeleteIoSubQ),
            0x1 => Some(AdminCmd::CreateIoSubQ),
            0x2 => Some(AdminCmd::GetLogPage),
            0x4 => Some(AdminCmd::DeleteIoCompQ),
            0x5 => Some(AdminCmd::CreateIoCompQ),
            0x6 => Some(AdminCmd::Identify),
            0x8 => Some(AdminCmd::Abort),
            0x9 => Some(AdminCmd::SetFeatures),
            0xa => Some(AdminCmd::GetFeatures),
            0xc => Some(AdminCmd::AsyncEventReq),
            _ => None,
        }
    }
}

enum NvmCmd {
    Flush,
    Write,
    Read,
}
impl NvmCmd {
    const fn from_opcode(opcode: u8) -> Option<Self> {
        match opcode {
            0x0 => Some(NvmCmd::Flush),
            0x1 => Some(NvmCmd::Write),
            0x2 => Some(NvmCmd::Read),
            _ => None,
        }
    }
}
