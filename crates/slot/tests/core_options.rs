mod common;

use slot::link_kind::{serial_option, LinkKind};
use slot_retro::LibretroCore;
use slot_store::{Blending, Core, Platform};

fn dylib_for(core: Core) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor")
        .join(slot::core::dylib_name(core))
}

fn every_serial_mode() -> Vec<&'static str> {
    let carts = [
        ("BPEE", "POKEMON EMER"),
        ("AWRE", "ADVANCEWARS"),
        ("AW2E", "ADVANCE WARS2"),
        ("AMFE", "METROID4"),
    ];
    let mut modes = Vec::new();
    for (code, title) in carts {
        for auto in [LinkKind::Cable, LinkKind::Wireless] {
            for chosen in [LinkKind::Cable, LinkKind::Wireless] {
                let mode = serial_option(chosen, auto, code, title);
                if !modes.contains(&mode) {
                    modes.push(mode);
                }
            }
        }
    }
    modes
}

fn every_option_is_one_the_core_has(core: &LibretroCore, what: &str) {
    let declared = core.declared_options();
    assert!(
        !declared.is_empty(),
        "{what}: the core declared no options at all, so this test would pass on anything"
    );
    let set = core.options();
    assert!(!set.is_empty(), "{what}: no options were set");
    for (key, value) in set {
        let Some(values) = declared.get(&key) else {
            let mut known: Vec<_> = declared.keys().cloned().collect();
            known.sort();
            panic!("{what}: the core declares no option {key:?}. It declares: {known:?}");
        };
        assert!(
            values.contains(&value),
            "{what}: the core declares {key:?} as {values:?}, and slot set it to {value:?}"
        );
    }
}

#[test]
fn every_option_slot_sets_is_one_the_core_declares() {
    let _g = common::core_lock();
    let mut ran = 0;
    for which in Core::ALL {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        for serial in every_serial_mode() {
            for bios in [false, true] {
                for (colour, platform) in [false, true]
                    .into_iter()
                    .flat_map(|c| Platform::ALL.map(|p| (c, p)))
                {
                    for palette in
                        std::iter::once(None).chain(slot_store::GbPalette::all().map(Some))
                    {
                        let mut core = LibretroCore::open(&path).expect("open core");
                        slot::core::apply_core_options(
                            &mut core,
                            which,
                            platform,
                            serial,
                            bios,
                            colour,
                            slot_store::Blending::Off,
                            palette,
                        );
                        every_option_is_one_the_core_has(
                            &core,
                            &format!(
                                "{} {platform:?} serial={serial} bios={bios} colour={colour} \
                                 palette={palette:?}",
                                which.as_str()
                            ),
                        );
                        ran += 1;
                    }
                }
            }
        }
    }
    if ran == 0 {
        eprintln!("no cores on this host, nothing was checked");
    }
}

#[test]
fn both_cores_declare_a_colour_correction_option() {
    let _g = common::core_lock();
    for (which, key) in [
        (Core::Mgba, "mgba_color_correction"),
        (Core::Gpsp, "gpsp_color_correction"),
    ] {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let core = LibretroCore::open(&path).expect("open core");
        assert!(
            core.declared_options().contains_key(key),
            "{} declares no {key}",
            which.as_str()
        );
    }
}

#[test]
fn a_cable_session_leaves_the_bios_to_the_card() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_link_options(&mut core, Core::Mgba, 1);
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_use_bios").map(String::as_str),
        None,
        "link mode overrode the card's BIOS"
    );
    assert_eq!(set.get("mgba_link").map(String::as_str), Some("on"));
    assert_eq!(
        set.get("mgba_link_player").map(String::as_str),
        Some("1"),
        "the port this device drives did not reach the core"
    );
    every_option_is_one_the_core_has(&core, "mgba cable session");
}

#[test]
fn a_cable_session_sets_nothing_on_gpsp() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpsp dylib on this host, skipping");
        return;
    }
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_link_options(&mut core, Core::Gpsp, 0);
    assert!(
        core.options().is_empty(),
        "a cable session reached into gpSP, which has no cable to offer"
    );
}

#[test]
fn a_named_palette_forces_a_plain_game_boy_without_hardware_presets() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let p = slot_store::GbPalette::parse("SGB 1-A").unwrap();
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_core_options(
        &mut core,
        Core::Mgba,
        Platform::Gb,
        "auto",
        false,
        false,
        slot_store::Blending::Off,
        Some(p),
    );
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_gb_model").map(String::as_str),
        Some("Game Boy")
    );
    assert_eq!(
        set.get("mgba_gb_colors_preset").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        set.get("mgba_gb_colors").map(String::as_str),
        Some("SGB 1-A")
    );
    drop(core);

    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_core_options(
        &mut core,
        Core::Mgba,
        Platform::Gb,
        "auto",
        false,
        false,
        slot_store::Blending::Off,
        None,
    );
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_gb_model").map(String::as_str),
        Some("Autodetect")
    );
    assert_eq!(
        set.get("mgba_gb_colors_preset").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        set.get("mgba_gb_colors").map(String::as_str),
        Some("GBC Dark Green →A"),
        "palettes off changed what a Game Boy cart gets today"
    );
}

#[test]
fn only_a_cart_without_the_colour_flag_is_game_boy_only() {
    let d = tempfile::tempdir().unwrap();
    let rom = |name: &str, cgb: u8| {
        let mut bytes = vec![0u8; 0x8000];
        bytes[0x143] = cgb;
        let p = d.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    assert!(slot::core::dmg_only(&rom("dmg.gb", 0x00)));
    assert!(!slot::core::dmg_only(&rom("dual.gbc", 0x80)));
    assert!(!slot::core::dmg_only(&rom("cgb.gbc", 0xC0)));
    assert!(!slot::core::dmg_only(&d.path().join("missing.gb")));
}

#[test]
fn a_linked_game_never_takes_a_named_palette() {
    let d = tempfile::tempdir().unwrap();
    let mut bytes = vec![0u8; 0x8000];
    bytes[0x146] = 0x03;
    let rom = d.path().join("sgb.gb");
    std::fs::write(&rom, bytes).unwrap();
    let p = slot_store::GbPalette::DEFAULT;
    let gb = slot_store::Platform::Gb;
    assert_eq!(slot::core::palette_for(Some(p), gb, &rom, None), Some(p));
    for player in [0, 1] {
        assert_eq!(
            slot::core::palette_for(Some(p), gb, &rom, Some(player)),
            None,
            "player {player}: a linked pair would run two models of one cart"
        );
    }
    assert_eq!(slot::core::palette_for(None, gb, &rom, None), None);
    assert_eq!(
        slot::core::palette_for(Some(p), slot_store::Platform::Gba, &rom, None),
        None
    );
}

#[test]
fn mgba_always_answers_the_game_boy_player_logo() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    for palette in [None, slot_store::GbPalette::parse("SGB 1-A")] {
        let mut core = LibretroCore::open(&path).expect("open core");
        slot::core::apply_core_options(
            &mut core,
            Core::Mgba,
            Platform::Gb,
            "auto",
            false,
            false,
            slot_store::Blending::Off,
            palette,
        );
        let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
        assert_eq!(
            set.get("mgba_force_gbp").map(String::as_str),
            Some("ON"),
            "Pokemon Pinball Ruby and Sapphire only rumbles on a Game Boy Player"
        );
    }
}

#[test]
fn every_frame_blending_slot_sets_is_one_the_core_declares() {
    let _g = common::core_lock();
    for which in Core::ALL {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        for blending in Blending::MGBA {
            let mut core = LibretroCore::open(&path).expect("open core");
            slot::core::apply_core_options(
                &mut core,
                which,
                Platform::Gba,
                "auto",
                false,
                false,
                blending,
                None,
            );
            every_option_is_one_the_core_has(
                &core,
                &format!("{} blending={blending:?}", which.as_str()),
            );
            let (key, value) = slot::core::blend_option(which, blending);
            assert!(
                core.options().iter().any(|(k, v)| k == key && v == value),
                "{} was never told {key}={value} for {blending:?}",
                which.as_str()
            );
        }
    }
}

#[test]
fn both_cores_declare_a_frame_blending_option() {
    let _g = common::core_lock();
    for (which, key) in [
        (Core::Mgba, "mgba_interframe_blending"),
        (Core::Gpsp, "gpsp_frame_mixing"),
    ] {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let core = LibretroCore::open(&path).expect("open core");
        assert!(
            core.declared_options().contains_key(key),
            "{} declares no {key}",
            which.as_str()
        );
    }
}

const PANEL: std::time::Duration = std::time::Duration::from_micros(16_742);
const PRESENTS: usize = 40;
const CHANGED_AT: usize = 30;

fn flicker_rom() -> Vec<u8> {
    const MVN_R3_R3: u32 = 0xe1e0_3003;
    let mut rom = common::gba_rom();
    rom[0xc0 + 9 * 4..0xc0 + 10 * 4].copy_from_slice(&MVN_R3_R3.to_le_bytes());
    rom
}

fn frames_with_mix_set_at(which: Core, at: Option<usize>) -> Vec<Vec<u8>> {
    use slot::audio::Ring;
    use slot::emu::{CoreState, EmuHandle, Speed};
    use std::time::{Duration, Instant};

    let d = tempfile::tempdir().expect("a temporary directory");
    let rom = d.path().join("blend.gba");
    std::fs::write(&rom, flicker_rom()).expect("write the test rom");
    let mut core = LibretroCore::open(&dylib_for(which)).expect("open core");
    slot::core::apply_core_options(
        &mut core,
        which,
        Platform::Gba,
        "auto",
        false,
        false,
        Blending::Off,
        None,
    );
    let ring = std::sync::Arc::new(Ring::new(1 << 16));
    let emu = EmuHandle::spawn(Box::new(core), rom, ring, None, None);
    let deadline = Instant::now() + Duration::from_secs(10);
    while emu.state() == CoreState::Loading {
        assert!(Instant::now() < deadline, "the core never loaded");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(emu.state(), CoreState::Ready);
    emu.set_driven(true);
    emu.set_speed(Speed::Normal);
    while !emu.locked() {
        assert!(Instant::now() < deadline, "the worker never locked");
        std::thread::sleep(Duration::from_millis(1));
    }
    let start = emu.published_count();
    let mut frames = Vec::new();
    for i in 0..PRESENTS {
        if at == Some(i) {
            let (key, value) = slot::core::blend_option(which, Blending::Mix);
            emu.set_option(key, value);
        }
        emu.tick(PANEL);
        assert!(
            emu.wait_frame(Duration::from_secs(2)),
            "a tick's frame never arrived"
        );
        frames.push(emu.latest_frame().expect("nothing published").to_vec());
    }
    assert_eq!(
        emu.published_count() - start,
        PRESENTS as u64,
        "a tick came late and the worker ran a frame of its own, so the runs are not comparable"
    );
    frames
}

#[test]
fn a_frame_blending_change_reaches_a_running_core_without_a_reload() {
    let _g = common::core_lock();
    for which in Core::ALL {
        if !dylib_for(which).exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let plain = frames_with_mix_set_at(which, None);
        let mixed = frames_with_mix_set_at(which, Some(CHANGED_AT));
        assert!(
            plain[..CHANGED_AT] == mixed[..CHANGED_AT],
            "{}: the two runs differ before anything was changed, so they cannot be compared",
            which.as_str()
        );
        assert!(
            plain[CHANGED_AT..]
                .windows(2)
                .all(|w| w[0][..3] != w[1][..3]),
            "{}: the test rom stopped flickering, so blending would have nothing to show",
            which.as_str()
        );
        let px0 = |frame: &[u8]| [frame[0], frame[1], frame[2]];
        for i in CHANGED_AT + 3..PRESENTS {
            assert_eq!(
                px0(&mixed[i]),
                px0(&mixed[i - 1]),
                "{}: frame {i} still flickers after mixing was set mid-game",
                which.as_str()
            );
            assert!(
                px0(&mixed[i]).iter().all(|c| (0x40..0xc0).contains(c)),
                "{}: frame {i} shows {:02x?}, not a mix of the black and white it flickers between",
                which.as_str(),
                px0(&mixed[i])
            );
        }
    }
}
