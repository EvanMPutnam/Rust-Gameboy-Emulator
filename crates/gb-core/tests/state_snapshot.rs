use gb_core::bus::Memory;
use gb_core::{GameBoy, GbConfig, GbError, StateError};

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

fn state_hash(gb: &GameBoy) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;

    for byte in [
        gb.cpu.registers().a,
        gb.cpu.registers().b,
        gb.cpu.registers().c,
        gb.cpu.registers().d,
        gb.cpu.registers().e,
        gb.cpu.registers().h,
        gb.cpu.registers().l,
        gb.bus.read_if(),
        gb.bus.read_ie(),
        gb.bus.read8(0xC000).expect("read wram"),
    ] {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
    }

    hash ^ u64::from(gb.cpu.registers().pc)
        ^ (u64::from(gb.cpu.registers().sp) << 16)
        ^ (u64::from(gb.clock.now()) << 32)
}

#[test]
fn snapshot_round_trip_restores_state_and_progression() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom(0x03, 0x03)).expect("init gb");
    gb.bus.write8(0x0000, 0x0A).expect("enable ram");
    gb.bus.write8(0xA000, 0x5E).expect("write ext ram");
    gb.run_cycles(20_000).expect("run baseline");

    let snapshot = gb.export_snapshot().expect("export snapshot");

    let mut restored = GameBoy::new(GbConfig::default(), test_rom(0x03, 0x03)).expect("init gb");
    restored
        .import_snapshot(&snapshot)
        .expect("import snapshot");

    assert_eq!(state_hash(&gb), state_hash(&restored));

    gb.run_cycles(10_000).expect("advance original");
    restored.run_cycles(10_000).expect("advance restored");
    assert_eq!(state_hash(&gb), state_hash(&restored));
}

#[test]
fn invalid_magic_and_version_and_checksum_return_typed_errors() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00)).expect("init gb");
    gb.run_cycles(1_024).expect("run baseline");
    let snapshot = gb.export_snapshot().expect("export snapshot");

    let mut bad_magic = snapshot.clone();
    bad_magic[0..4].copy_from_slice(b"BAD!");
    let err = gb
        .import_snapshot(&bad_magic)
        .expect_err("bad magic must fail");
    assert!(matches!(
        err,
        GbError::State(StateError::InvalidMagic { .. })
    ));

    let mut bad_version = snapshot.clone();
    bad_version[4..6].copy_from_slice(&2u16.to_le_bytes());
    let err = gb
        .import_snapshot(&bad_version)
        .expect_err("unsupported version must fail");
    assert!(matches!(
        err,
        GbError::State(StateError::UnsupportedVersion(2))
    ));

    let mut bad_checksum = snapshot;
    let last = bad_checksum.len() - 1;
    bad_checksum[last] ^= 0xFF;
    let err = gb
        .import_snapshot(&bad_checksum)
        .expect_err("checksum mismatch must fail");
    assert!(matches!(
        err,
        GbError::State(StateError::ChecksumMismatch { .. })
    ));
}

#[test]
fn snapshot_resume_is_deterministic_after_restore() {
    let mut baseline = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00)).expect("init gb");
    baseline.run_cycles(30_000).expect("run pre snapshot");
    let snapshot = baseline.export_snapshot().expect("export snapshot");

    let mut resumed = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00)).expect("init gb");
    resumed.import_snapshot(&snapshot).expect("import snapshot");

    baseline.run_cycles(50_000).expect("advance baseline");
    resumed.run_cycles(50_000).expect("advance resumed");

    assert_eq!(state_hash(&baseline), state_hash(&resumed));
}
