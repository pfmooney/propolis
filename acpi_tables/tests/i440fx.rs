use acpi_tables::aml::*;
use acpi_tables::sdt::SDT;

use std::fs::File;
use std::io::prelude::Write;

fn gen_lnk(name: &str, uid: u8, irq: &str) -> Device {
    Device::new(
        name.as_path(),
        vec![
            Name::new("_HID", EISAName::new("PNP0C0F")).into(),
            Name::new("_UID", uid).into(),
            Method::new(
                "_STA",
                0,
                false,
                vec![Return::new(MethodCall::new(
                    "PSTA",
                    vec![irq.as_path().into()],
                ))
                .into()],
            )
            .into(),
            Method::new(
                "_DIS",
                0,
                false,
                vec![
                    Or::new_assign(irq.as_path(), 0x80u8, irq.as_path()).into()
                ],
            )
            .into(),
            Method::new(
                "_CRS",
                0,
                false,
                vec![Return::new(Path::new("_PRS")).into()],
            )
            .into(),
            Method::new("_SRS", 1, false, vec![]).into(),
        ],
    )
}

fn gen_intr_routing_table() -> Method {
    let mut packages: Vec<Box<dyn Aml>> = Vec::new();

    let lnk_paths = [
        Path::new("\\_SB_.PCI0.LPC_.LNKA"),
        Path::new("\\_SB_.PCI0.LPC_.LNKB"),
        Path::new("\\_SB_.PCI0.LPC_.LNKC"),
        Path::new("\\_SB_.PCI0.LPC_.LNKD"),
    ];
    let lnks_path = Path::new("\\_SB_.PCI0.LPC_.LNKS");

    for slot in 0..=31 {
        for func in 0..=3 {
            let dev_addr = (slot as u32) << 16 | 0xffffu32;

            let lnk = match (slot, func) {
                (1, 0) => lnks_path.clone(),
                _ => lnk_paths[(slot + func + 3) % 4 as usize].clone(),
            };
            let pkg = Package::new(vec![
                dev_addr.into(),
                (func as u8).into(),
                lnk.into(),
                0u8.into(),
            ]);
            packages.push(pkg.into());
        }
    }
    Method::new(
        "_PRT",
        0,
        false,
        vec![Return::new(Package::new(packages)).into()],
    )
}

fn gen_cres() -> Box<dyn Aml> {
    Name::new(
        "CRES",
        ResourceTemplate::new(vec![
            AddressSpace::new_bus_number(0u16, 0xffu16).into(),
            AddressSpace::new_io(0xcf8u16, 0xcffu16).into(),
            AddressSpace::new_io(0x0u16, 0xcf7u16).into(),
            AddressSpace::new_io(0xd00u16, 0xffffu16).into(),
            AddressSpace::new_io(0xd00u16, 0xffffu16).into(),
        ]),
    )
    .into()
}

#[test]
fn full_i440fx() {
    let lnk_s = Device::new(
        "LNKS",
        vec![
            Name::new("_ADR", 0x00010000u32).into(),
            Name::new("_UID", 0u8).into(),
            Name::new("_STA", 0xbu8).into(),
            Method::new("_SRS", 1, false, vec![]).into(),
            Method::new("_DIS", 0, false, vec![]).into(),
            Name::new(
                "_PRS",
                ResourceTemplate::new(vec![Interrupt::new(
                    true, false, false, true, 9,
                )
                .into()]),
            )
            .into(),
            Method::new(
                "_CRS",
                0,
                false,
                vec![Return::new(Path::new("_PRS")).into()],
            )
            .into(),
        ],
    );
    // let lnk_a = Device::new(
    //     "LNKA",
    //     vec![
    //         Name::new("_HID", EISAName::new("PNP0C0F")).into(),
    //         Name::new("_UID", 1u8).into(),
    //         Method::new(
    //             "_STA",
    //             0,
    //             false,
    //             vec![Return::new(MethodCall::new(
    //                 "PSTA",
    //                 vec![Path::new("PIRA").into()],
    //             ))
    //             .into()],
    //         )
    //         .into(),
    //         Method::new("_SRS", 1, false, vec![]).into(),
    //         Method::new("_DIS", 0, false, vec![]).into(),
    //         Name::new(
    //             "_PRS",
    //             ResourceTemplate::new(vec![Interrupt::new(
    //                 true, false, false, true, 9,
    //             )
    //             .into()]),
    //         )
    //         .into(),
    //         Method::new(
    //             "_CRS",
    //             0,
    //             false,
    //             vec![Return::new(Path::new("_PRS")).into()],
    //         )
    //         .into(),
    //     ],
    // );

    let irqw = "IRWQ";
    let buf0 = "BUF0";
    let pcrs = Method::new(
        "PCRS",
        1,
        true,
        vec![
            Name::new(
                buf0.as_path(),
                ResourceTemplate::new(vec![Interrupt::new(
                    true, false, false, true, 0,
                )
                .into()]),
            )
            .into(),
            CreateDWordField::new(buf0.as_path(), 0x5u8, irqw.as_path()).into(),
            If::new(
                LNot::new(And::new(Arg(0), 0x80u8)),
                vec![Store::new(irqw.as_path(), Arg(0)).into()],
            )
            .into(),
            Return::new(buf0.as_path()).into(),
        ],
    );

    //sdt::SDT::new(*b"DSDT", )
    let top = Scope::new(
        "\\_SB_",
        vec![Device::new(
            Path::new("PCI0"),
            vec![
                Name::new("_HID", EISAName::new("PNP0A03")).into(),
                Name::new("_ADR", 0u32).into(),
                Name::new("_BBN", 0u8).into(),
                Name::new("_UID", 0u8).into(),
                gen_cres(),
                gen_intr_routing_table().into(),
                Device::new(
                    "LPC_",
                    vec![
                        Name::new("_ADR", 0x00010000u32).into(),
                        Name::new("_UID", ZERO).into(),
                        OpRegion::new(
                            "PPR0",
                            OpRegionSpace::PCIConfig,
                            0x60,
                            0x4,
                        )
                        .into(),
                        Field::new(
                            "PPR0",
                            FieldAccessType::Any,
                            FieldUpdateRule::Preserve,
                            vec![
                                FieldEntry::Named(b"PIRA".clone(), 8),
                                FieldEntry::Named(b"PIRB".clone(), 8),
                                FieldEntry::Named(b"PIRC".clone(), 8),
                                FieldEntry::Named(b"PIRD".clone(), 8),
                            ],
                        )
                        .into(),
                        Method::new(
                            "PSTA",
                            1,
                            false,
                            vec![
                                If::new(
                                    And::new(Arg(0), 0x80u8),
                                    vec![Return::new(0x9u8).into()],
                                )
                                .into(),
                                Else::new(vec![Return::new(0xbu8).into()])
                                    .into(),
                            ],
                        )
                        .into(),
                        pcrs.into(),
                        lnk_s.into(),
                        gen_lnk("LNKA", 1, "PIRA").into(),
                        gen_lnk("LNKB", 2, "PIRB").into(),
                        gen_lnk("LNKC", 3, "PIRC").into(),
                        gen_lnk("LNKD", 3, "PIRD").into(),
                    ],
                )
                .into(),
            ],
        )
        .into()],
    );
    let top_bytes = top.to_aml_bytes();
    let mut dsdt = SDT::new(
        *b"DSDT",
        1,
        *b"OXIDE ",
        *b"PROPOLIS",
        0,
        *b"OXDE",
        0,
        Some(top_bytes.len() as u32),
    );
    dsdt.append_slice(&top_bytes);
    let mut fp = File::create("dsdt.out").unwrap();
    fp.write_all(dsdt.as_slice()).unwrap();
}
