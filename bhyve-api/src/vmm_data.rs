#![allow(non_camel_case_types)]

use bitflags::bitflags;

#[repr(u16)]
pub enum VmmDataClass {
    Meta = 0,
    Version = 1,
    Register = 2,
    Msr = 3,
    Fpu = 4,
    Lapic = 5,
    VmmArch = 6,
    IoApic = 7,
    AtPit = 8,
    AtPic = 9,
    Hpet = 10,
    PmTimer = 11,
    Rtc = 12,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_lapic_page {
    pub vlp_id: u32,
    pub vlp_version: u32,
    pub vlp_tpr: u32,
    pub vlp_apr: u32,
    pub vlp_ldr: u32,
    pub vlp_dfr: u32,
    pub vlp_svr: u32,
    pub vlp_isr: [u32; 8],
    pub vlp_tmr: [u32; 8],
    pub vlp_irr: [u32; 8],
    pub vlp_esr: u32,
    pub vlp_lvt_cmci: u32,
    pub vlp_icr: u64,
    pub vlp_lvt_timer: u32,
    pub vlp_lvt_thermal: u32,
    pub vlp_lvt_pcint: u32,
    pub vlp_lvt_lint0: u32,
    pub vlp_lvt_lint1: u32,
    pub vlp_lvt_error: u32,
    pub vlp_icr_timer: u32,
    pub vlp_dcr_timer: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_lapic {
    pub vl_lapic: vdi_lapic_page,
    pub vl_msr_apicbase: u64,
    pub vl_timer_target: u64,
    pub vl_esr_pending: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_ioapic {
    pub vi_pin_reg: [u64; 32],
    pub vi_pin_level: [u32; 32],
    pub vi_id: u32,
    pub vi_reg_sel: u32,
}

bitflags! {
    // vac_status bits:
    // - 0b00001 status latched
    // - 0b00010 output latched
    // - 0b00100 control register sel
    // - 0b01000 output latch sel
    // - 0b10000 free-running timer
    #[repr(C)]
    #[derive(Default)]
    pub struct VdiAtpitStatus: u8 {
        const STATUS_LATCHED = (1 << 0);
        const OUTPUT_LATCHED = (1 << 1);
        const CR_REG_SEL = (1 << 2);
        const OL_REG_SEL = (1 << 3);
        const FREE_RUNNING = (1 << 4);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_atpit_channel {
    pub vac_initial: u16,
    pub vac_reg_cr: u16,
    pub vac_reg_ol: u16,
    pub vac_reg_status: u8,
    pub vac_mode: u8,
    pub vac_status: VdiAtpitStatus,
    pub vac_time_target: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_atpit {
    pub va_channel: [vdi_atpit_channel; 3],
}

bitflags! {
    // vac_status bits:
    // - 0b00000001 ready
    // - 0b00000010 auto EOI
    // - 0b00000100 poll
    // - 0b00001000 rotate
    // - 0b00010000 special full nested
    // - 0b00100000 read isr next
    // - 0b01000000 intr raised
    // - 0b10000000 special mask mode
    #[repr(C)]
    #[derive(Default)]
    pub struct VdiAtpicStatus: u8 {
        const READY = (1 << 0);
        const AUTO_EOI = (1 << 1);
        const POLL = (1 << 2);
        const ROTATE = (1 << 3);
        const SPECIAL_FULL_NESTED = (1 << 4);
        const READ_ISR_NEXT = (1 << 5);
        const INTR_RAISED = (1 << 6);
        const SPECIAL_MASK_MODE = (1 << 7);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_atpic_chip {
    pub vac_icw_state: u8,
    pub vac_status: VdiAtpicStatus,
    pub vac_reg_irr: u8,
    pub vac_reg_isr: u8,
    pub vac_reg_imr: u8,
    pub vac_irq_base: u8,
    pub vac_lowprio: u8,
    pub vac_elc: u8,
    pub vac_level: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_atpic {
    pub va_chip: [vdi_atpic_chip; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_hpet_timer {
    pub vht_config: u64,
    pub vht_msi: u64,
    pub vht_comp_val: u32,
    pub vht_comp_rate: u32,
    pub vht_time_base: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_hpet {
    pub vh_config: u64,
    pub vh_isr: u64,
    pub vh_count_base: u32,
    pub vh_time_base: u64,
    pub vh_timers: [vdi_hpet_timer; 8],
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct vdi_pm_timer {
    pub vpt_time_base: u64,
    pub vpt_val_base: u32,
    pub vpt_ioport: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct vdi_rtc {
    pub vr_content: [u8; 128],
    pub vr_addr: u8,
    pub vr_time_base: u64,
    pub vr_rtc_sec: u64,
    pub vr_rtc_nsec: u64,
}
impl Default for vdi_rtc {
    fn default() -> Self {
        Self {
            vr_content: [0u8; 128],
            vr_addr: 0,
            vr_time_base: 0,
            vr_rtc_sec: 0,
            vr_rtc_nsec: 0,
        }
    }
}
