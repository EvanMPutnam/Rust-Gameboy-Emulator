use gb_core::apu::Apu;

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

#[test]
fn fixed_script_produces_identical_samples() {
    let first = run_script();
    let second = run_script();
    assert_eq!(first, second);
}
