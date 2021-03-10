use std::sync::{Arc, Mutex};

use crate::common::*;
use crate::dispatch::DispCtx;
use crate::hw::pci;
use crate::util::regmap::RegMap;

use lazy_static::lazy_static;

mod queue;


#[derive(Default)]
struct NvmeState {
}

#[derive(Default)]
struct PciNvme {
    state: Mutex<NvmeState>,
}


impl PciNvme {
    pub fn create(vendor: u16, device: u16) -> Arc<pci::DeviceInst> {
        let mut builder = pci::Builder::new(pci::Ident {
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
}

impl pci::Device for PciNvme {
    fn bar_rw(&self, bar: pci::BarN, rwo: RWOp, ctx: &DispCtx) {
        assert_eq!(bar, pci::BarN::BAR0);
        match rwo {
            RWOp::Read(ro) => {
                unimplemented!("BAR read ({:?} @ {:x})", bar, ro.offset())
            }
            RWOp::Write(wo) => {
                unimplemented!("BAR write ({:?} @ {:x})", bar, wo.offset())
            }
        }
    }

    fn attach(
        &self,
        lintr_pin: Option<pci::INTxPin>,
        msix_hdl: Option<pci::MsixHdl>,
    ) {
        // A device model has no reason to request interrupt resources but not
        // make use of them.
        assert!(lintr_pin.is_none());
        assert!(msix_hdl.is_none());
    }

    fn interrupt_mode_change(&self, mode: pci::IntrMode) {}

    fn msi_update(&self, info: pci::MsiUpdate, ctx: &DispCtx) {}
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
const CONTROLLER_REG_SZ: usize = 0x1000 + 8;
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
