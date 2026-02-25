use gb_core::apu::{Apu, ApuDiagnostics};

fn write_reg(io: &mut [u8; 0x80], write_mask: &mut u128, index: usize, value: u8) {
    io[index] = value;
    *write_mask |= 1u128 << index;
}

fn tick(apu: &mut Apu, io: &mut [u8; 0x80], write_mask: &mut u128, cycles: u32) {
    apu.tick(cycles, io, *write_mask)
        .expect("apu tick should pass");
    *write_mask = 0;
}

fn run_script() -> Vec<i16> {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = 0u128;

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x33;

    write_reg(&mut io, &mut write_mask, 0x16, 0x80);
    write_reg(&mut io, &mut write_mask, 0x17, 0xE2);
    write_reg(&mut io, &mut write_mask, 0x18, 0x20);
    write_reg(&mut io, &mut write_mask, 0x19, 0x80);

    write_reg(&mut io, &mut write_mask, 0x1A, 0x80);
    write_reg(&mut io, &mut write_mask, 0x1C, 0x20);
    write_reg(&mut io, &mut write_mask, 0x1D, 0x40);
    write_reg(&mut io, &mut write_mask, 0x1E, 0x80);

    for index in 0..16 {
        write_reg(&mut io, &mut write_mask, 0x30 + index, 0xF0);
    }

    tick(&mut apu, &mut io, &mut write_mask, 10_000);
    tick(&mut apu, &mut io, &mut write_mask, 10_000);
    tick(&mut apu, &mut io, &mut write_mask, 10_000);

    apu.take_samples()
}

fn run_script_with_diagnostics(include_channel_stats: bool) -> (Vec<i16>, ApuDiagnostics) {
    let mut apu = Apu::new();
    apu.set_diagnostics_enabled(true, include_channel_stats);

    let mut io = [0u8; 0x80];
    let mut write_mask = 0u128;

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x33;

    write_reg(&mut io, &mut write_mask, 0x16, 0x80);
    write_reg(&mut io, &mut write_mask, 0x17, 0xE2);
    write_reg(&mut io, &mut write_mask, 0x18, 0x20);
    write_reg(&mut io, &mut write_mask, 0x19, 0x80);

    write_reg(&mut io, &mut write_mask, 0x1A, 0x80);
    write_reg(&mut io, &mut write_mask, 0x1C, 0x20);
    write_reg(&mut io, &mut write_mask, 0x1D, 0x40);
    write_reg(&mut io, &mut write_mask, 0x1E, 0x80);

    for index in 0..16 {
        write_reg(&mut io, &mut write_mask, 0x30 + index, 0xF0);
    }

    tick(&mut apu, &mut io, &mut write_mask, 10_000);
    tick(&mut apu, &mut io, &mut write_mask, 10_000);
    tick(&mut apu, &mut io, &mut write_mask, 10_000);

    let diagnostics = apu
        .diagnostics()
        .expect("apu diagnostics should be available");
    (apu.take_samples(), diagnostics)
}

#[test]
fn fixed_script_produces_identical_samples() {
    let first = run_script();
    let second = run_script();
    assert_eq!(first, second);
}

#[test]
fn diagnostics_are_deterministic_for_fixed_script() {
    let (samples_a, diagnostics_a) = run_script_with_diagnostics(true);
    let (samples_b, diagnostics_b) = run_script_with_diagnostics(true);

    assert_eq!(samples_a, samples_b);
    assert_eq!(diagnostics_a, diagnostics_b);
    assert!(diagnostics_a.channel_stats.is_some());
    assert!(diagnostics_a.total_samples > 0);
}
