use gb_core::bus::Memory;
use gb_core::{GameBoy, GbButton, GbConfig};

fn test_rom(cart_type: u8, ram_code: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = cart_type;
    rom[0x0148] = 0x00;
    rom[0x0149] = ram_code;
    rom[0x0100] = 0xC3;
    rom[0x0101] = 0x00;
    rom[0x0102] = 0x01;
    rom
}

fn hash_samples(samples: &[i16]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for sample in samples {
        for byte in sample.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
        }
    }
    hash
}

#[test]
fn input_events_map_to_expected_joypad_lines() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00)).expect("init gb");

    gb.bus.write8(0xFF00, 0x20).expect("select dpad");
    gb.set_button(GbButton::Right, true).expect("press right");
    assert_eq!(gb.bus.read8(0xFF00).expect("read p1") & 0x01, 0x00);

    gb.set_button(GbButton::Right, false)
        .expect("release right");
    assert_ne!(gb.bus.read8(0xFF00).expect("read p1") & 0x01, 0x00);
}

#[test]
fn frame_and_audio_apis_are_deterministic_over_long_runs() {
    let mut a = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00)).expect("init gb a");
    let mut b = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00)).expect("init gb b");

    let mut frame_hash_a = 0u64;
    let mut frame_hash_b = 0u64;
    let mut audio_hash_a = 0u64;
    let mut audio_hash_b = 0u64;

    for _ in 0..32 {
        a.run_cycles(20_000).expect("run a");
        b.run_cycles(20_000).expect("run b");

        while let Some(frame) = a.take_frame() {
            frame_hash_a ^= u64::from(gb_core::state::checksum32(&frame));
        }
        while let Some(frame) = b.take_frame() {
            frame_hash_b ^= u64::from(gb_core::state::checksum32(&frame));
        }

        let samples_a = a.take_audio_samples();
        let samples_b = b.take_audio_samples();
        audio_hash_a ^= hash_samples(&samples_a);
        audio_hash_b ^= hash_samples(&samples_b);
    }

    assert_eq!(frame_hash_a, frame_hash_b);
    assert_eq!(audio_hash_a, audio_hash_b);
}

#[test]
fn save_import_export_via_frontend_contract_preserves_ram() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom(0x03, 0x03)).expect("init gb");
    gb.bus.write8(0x0000, 0x0A).expect("enable ram");
    gb.bus.write8(0xA234, 0x9C).expect("write ext ram");

    let save = gb.export_save().expect("export save");

    let mut restored = GameBoy::new(GbConfig::default(), test_rom(0x03, 0x03)).expect("init gb");
    restored.import_save(&save).expect("import save");
    restored.bus.write8(0x0000, 0x0A).expect("enable ram");

    assert_eq!(restored.bus.read8(0xA234).expect("read ext ram"), 0x9C);
}
