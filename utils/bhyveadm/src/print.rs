use std::io::Result;

use crate::ioctl_helper::VmmHdl;
use bhyve_api::{self, VmmDataClass as Vdc};

enum Component {
    IoApic,
    AtPit,
    AtPic,
    Hpet,
    PmTimer,
    Rtc,
}

impl Component {
    fn parse(inp: &str) -> Option<Self> {
        match inp {
            "ioapic" => Some(Component::IoApic),
            "atpit" | "pit" => Some(Component::AtPit),
            "atpic" | "pic" => Some(Component::AtPic),
            "hpet" => Some(Component::Hpet),
            "pmtimer" => Some(Component::PmTimer),
            "rtc" => Some(Component::Rtc),
            _ => None,
        }
    }
}

enum CpuComponent {
    Lapic,
}
impl CpuComponent {
    fn parse(inp: &str) -> Option<Self> {
        match inp {
            "lapic" => Some(CpuComponent::Lapic),
            _ => None,
        }
    }
}

fn print_ioapic(hdl: &VmmHdl) -> Result<()> {
    let ioapic: bhyve_api::vdi_ioapic = hdl.get_data(-1, Vdc::IoApic, 1, 0)?;

    println!(
        "### IOAPIC ###\nid:\t{:x}\nregsel:\t{:x}",
        ioapic.vi_id, ioapic.vi_reg_sel
    );
    for (i, redir) in ioapic.vi_pin_reg.iter().enumerate() {
        println!("redir_reg{}:\t{:016x}", i, redir);
    }
    for (i, level) in ioapic.vi_pin_level.iter().enumerate() {
        println!("pin{}_level:\t{}", i, level);
    }
    Ok(())
}

fn print_atpit(hdl: &VmmHdl) -> Result<()> {
    let atpit: bhyve_api::vdi_atpit = hdl.get_data(-1, Vdc::AtPit, 1, 0)?;
    println!("### ATPIT ###");
    for (num, chan) in atpit.va_channel.iter().enumerate() {
        println!("chan{}_counter:\t{:04x}", num, chan.vac_initial);
        println!("chan{}_reg_cr:\t{:04x}", num, chan.vac_reg_cr);
        println!("chan{}_reg_ol:\t{:04x}", num, chan.vac_reg_ol);
        println!("chan{}_reg_status:\t{:02x}", num, chan.vac_reg_status);
        println!("chan{}_mode:\t{:02x}", num, chan.vac_mode);
        // TODO: decode  mode
        println!("chan{}_status:\t{:02x}", num, chan.vac_status);
        // TODO: decode status bits
        println!("chan{}_time_target:\t{}", num, chan.vac_time_target);
    }
    Ok(())
}

fn print_atpic(hdl: &VmmHdl) -> Result<()> {
    let atpic: bhyve_api::vdi_atpic = hdl.get_data(-1, Vdc::AtPic, 1, 0)?;
    println!("### ATPIC ###");
    for (num, chip) in atpic.va_chip.iter().enumerate() {
        println!("chip{}_state:\t{:02x}", num, chip.vac_icw_state);
        // TODO: decode state
        println!("chip{}_status:\t{:02x}", num, chip.vac_status);
        // TODO: decode status
        println!("chip{}_irr:\t{:08b}", num, chip.vac_reg_isr);
        println!("chip{}_isr:\t{:08b}", num, chip.vac_reg_irr);
        println!("chip{}_imr:\t{:08b}", num, chip.vac_reg_imr);
        println!("chip{}_irq_base:\t{:02x}", num, chip.vac_irq_base);
        println!("chip{}_low_prio:\t{:02x}", num, chip.vac_lowprio);
        println!("chip{}_elc:\t{:08b}", num, chip.vac_elc);
        for (i, level) in chip.vac_level.iter().enumerate() {
            println!("chip{}_pin{}_level:\t{}", num, i, level);
        }
    }
    Ok(())
}
fn print_hpet(hdl: &VmmHdl) -> Result<()> {
    let hpet: bhyve_api::vdi_hpet = hdl.get_data(-1, Vdc::Hpet, 1, 0)?;
    println!("### HPET ###");
    println!("dev_cfg:\t{:016x}", hpet.vh_config);
    println!("isr:\t{:016x}", hpet.vh_isr);
    println!("counter_base:\t{:08x}", hpet.vh_count_base);
    println!("time_base:\t{:016x}", hpet.vh_time_base);
    for (n, tmr) in hpet.vh_timers.iter().enumerate() {
        println!("timer{}_cfg:\t{:016x}", n, tmr.vht_config);
        println!("timer{}_msi:\t{:016x}", n, tmr.vht_msi);
        println!("timer{}_comp_val:\t{:08x}", n, tmr.vht_comp_val);
        println!("timer{}_comp_rate:\t{:08x}", n, tmr.vht_comp_rate);
        println!("timer{}_time_base:\t{:16}", n, tmr.vht_time_base);
    }

    Ok(())
}
fn print_pmtimer(hdl: &VmmHdl) -> Result<()> {
    let pmtimer: bhyve_api::vdi_pm_timer =
        hdl.get_data(-1, Vdc::PmTimer, 1, 0)?;

    println!("### PMTIMER ###");
    println!("time_base:\t{}", pmtimer.vpt_time_base);
    println!("val_base:\t{:08x}", pmtimer.vpt_val_base);

    Ok(())
}

fn print_rtc(hdl: &VmmHdl) -> Result<()> {
    let rtc: bhyve_api::vdi_rtc = hdl.get_data(-1, Vdc::Rtc, 1, 0)?;

    println!("### RTC ###");
    println!("reg_addr:\t{:02x}", rtc.vr_addr);
    println!("time_base:\t{}", rtc.vr_time_base);
    println!("time_rtc:\t{}", rtc.vr_rtc_sec);
    for n in 0..8 {
        let idx = n * 2;
        println!(
            "data{}:\t{:016x}{:016x}",
            idx,
            rtc.vr_content[idx],
            rtc.vr_content[idx + 1]
        );
    }

    Ok(())
}

fn print_lapic(hdl: &VmmHdl, vcpu: i32) -> Result<()> {
    let lapic: bhyve_api::vdi_lapic = hdl.get_data(vcpu, Vdc::Lapic, 1, 0)?;
    println!("{:?}", lapic);
    Ok(())
}

pub fn do_print(vm: &str, components: &[String]) -> Result<()> {
    let hdl = VmmHdl::open(vm)?;

    for comp in components.iter() {
        let lower = comp.to_lowercase();
        if let Some(c) = Component::parse(&lower) {
            let _ = match c {
                Component::IoApic => print_ioapic(&hdl),
                Component::AtPit => print_atpit(&hdl),
                Component::AtPic => print_atpic(&hdl),
                Component::Hpet => print_hpet(&hdl),
                Component::PmTimer => print_pmtimer(&hdl),
                Component::Rtc => print_rtc(&hdl),
            };
        } else {
            eprintln!("unrecognized component: {}", comp);
        }
    }

    Ok(())
}
pub fn do_print_cpu(vm: &str, vcpu: u32, components: &[String]) -> Result<()> {
    let hdl = VmmHdl::open(vm)?;

    if vcpu > bhyve_api::VM_MAXCPU as u32 {
        eprintln!("invalid vcpu ID: {}", vcpu);
        // TODO: real error handling
        return Ok(());
    }
    let vcpu: i32 = vcpu as i32;

    for comp in components.iter() {
        let lower = comp.to_lowercase();
        if let Some(c) = CpuComponent::parse(&lower) {
            let _ = match c {
                CpuComponent::Lapic => print_lapic(&hdl, vcpu),
            };
        } else {
            eprintln!("unrecognized component: {}", comp);
        }
    }

    Ok(())
}

pub fn component_list() -> Result<()> {
    let comp = ["ioapic", "atpit", "atpic", "hpet", "pmtimer", "rtc"];
    println!("VM-wide components:");
    for c in comp.iter() {
        println!("\t{}", c);
    }
    println!("Per-CPU components:");
    let cpu_comp = ["lapic"];
    for c in cpu_comp.iter() {
        println!("\t{}", c);
    }
    Ok(())
}
