use gb_core::apu::Apu;

fn write_reg(io: &mut [u8; 0x80], write_mask: &mut [bool; 0x80], index: usize, value: u8) {
    io[index] = value;
    write_mask[index] = true;
}

fn tick(apu: &mut Apu, io: &mut [u8; 0x80], write_mask: &mut [bool; 0x80], cycles: u32) {
    apu.tick(cycles, io, write_mask)
        .expect("apu tick should pass");
    write_mask.fill(false);
}

fn max_abs_sample(samples: &[i16]) -> i16 {
    samples.iter().map(|sample| sample.abs()).max().unwrap_or(0)
}

#[test]
fn channel_trigger_generates_audio_samples() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x11;

    write_reg(&mut io, &mut write_mask, 0x11, 0xC0);
    write_reg(&mut io, &mut write_mask, 0x12, 0xF3);
    write_reg(&mut io, &mut write_mask, 0x13, 0x70);
    write_reg(&mut io, &mut write_mask, 0x14, 0x80);

    tick(&mut apu, &mut io, &mut write_mask, 20_000);

    let samples = apu.take_samples();
    assert!(!samples.is_empty());
    assert_ne!(io[0x26] & 0x01, 0);
}

#[test]
fn nr52_disable_clears_channel_state() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x11;
    write_reg(&mut io, &mut write_mask, 0x11, 0xC0);
    write_reg(&mut io, &mut write_mask, 0x12, 0xF3);
    write_reg(&mut io, &mut write_mask, 0x14, 0x80);

    tick(&mut apu, &mut io, &mut write_mask, 10_000);

    write_reg(&mut io, &mut write_mask, 0x26, 0x00);
    tick(&mut apu, &mut io, &mut write_mask, 10_000);

    assert_eq!(io[0x26] & 0x0F, 0);
    assert_eq!(io[0x11], 0);
    assert_eq!(apu.take_samples(), Vec::<i16>::new());
}

#[test]
fn stereo_routing_applies_nr51_mask() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x10;

    write_reg(&mut io, &mut write_mask, 0x11, 0xC0);
    write_reg(&mut io, &mut write_mask, 0x12, 0xF3);
    write_reg(&mut io, &mut write_mask, 0x13, 0x70);
    write_reg(&mut io, &mut write_mask, 0x14, 0x80);

    tick(&mut apu, &mut io, &mut write_mask, 30_000);
    let samples = apu.take_samples();

    assert!(!samples.is_empty());

    let mut saw_non_zero_left = false;
    let mut all_right_zero = true;

    for frame in samples.chunks_exact(2) {
        if frame[0] != 0 {
            saw_non_zero_left = true;
        }
        if frame[1] != 0 {
            all_right_zero = false;
        }
    }

    assert!(saw_non_zero_left);
    assert!(all_right_zero);
}

#[test]
fn length_counter_expires_without_constant_register_reloads() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x11;

    write_reg(&mut io, &mut write_mask, 0x11, 0xFF);
    write_reg(&mut io, &mut write_mask, 0x12, 0xF3);
    write_reg(&mut io, &mut write_mask, 0x13, 0x70);
    write_reg(&mut io, &mut write_mask, 0x14, 0xC0);

    tick(&mut apu, &mut io, &mut write_mask, 1_000);
    assert_ne!(io[0x26] & 0x01, 0);

    tick(&mut apu, &mut io, &mut write_mask, 100_000);
    assert_eq!(io[0x26] & 0x01, 0);
}

#[test]
fn trigger_reloads_envelope_volume_without_rewriting_envelope_register() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x22;

    write_reg(&mut io, &mut write_mask, 0x16, 0xC0);
    write_reg(&mut io, &mut write_mask, 0x17, 0xF1);
    write_reg(&mut io, &mut write_mask, 0x18, 0x40);
    write_reg(&mut io, &mut write_mask, 0x19, 0x80);

    tick(&mut apu, &mut io, &mut write_mask, 140_000);
    let before_retrigger = apu.take_samples();
    let before_peak = max_abs_sample(&before_retrigger);

    write_reg(&mut io, &mut write_mask, 0x19, 0x80);
    tick(&mut apu, &mut io, &mut write_mask, 4_096);
    let after_retrigger = apu.take_samples();
    let after_peak = max_abs_sample(&after_retrigger);

    assert!(after_peak > before_peak);
}

#[test]
fn channel_one_sweep_overflow_disables_channel() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x11;

    write_reg(&mut io, &mut write_mask, 0x10, 0x11);
    write_reg(&mut io, &mut write_mask, 0x11, 0x80);
    write_reg(&mut io, &mut write_mask, 0x12, 0xF3);
    write_reg(&mut io, &mut write_mask, 0x13, 0xD0);
    write_reg(&mut io, &mut write_mask, 0x14, 0x87);

    tick(&mut apu, &mut io, &mut write_mask, 20_000);
    assert_eq!(io[0x26] & 0x01, 0);
}

#[test]
fn wave_dac_disable_turns_channel_off_immediately() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    io[0x26] = 0x80;
    io[0x24] = 0x77;
    io[0x25] = 0x44;
    io[0x30] = 0xFF;

    write_reg(&mut io, &mut write_mask, 0x1A, 0x80);
    write_reg(&mut io, &mut write_mask, 0x1B, 0xFF);
    write_reg(&mut io, &mut write_mask, 0x1C, 0x20);
    write_reg(&mut io, &mut write_mask, 0x1D, 0x40);
    write_reg(&mut io, &mut write_mask, 0x1E, 0x80);

    tick(&mut apu, &mut io, &mut write_mask, 10_000);
    assert_ne!(io[0x26] & 0x04, 0);

    write_reg(&mut io, &mut write_mask, 0x1A, 0x00);
    tick(&mut apu, &mut io, &mut write_mask, 16);
    assert_eq!(io[0x26] & 0x04, 0);
}

#[test]
fn writes_are_ignored_while_apu_power_is_off() {
    let mut apu = Apu::new();
    let mut io = [0u8; 0x80];
    let mut write_mask = [false; 0x80];

    write_reg(&mut io, &mut write_mask, 0x12, 0xF3);
    write_reg(&mut io, &mut write_mask, 0x14, 0x80);
    tick(&mut apu, &mut io, &mut write_mask, 32);

    assert_eq!(io[0x12], 0);
    assert_eq!(io[0x14], 0);
    assert_eq!(io[0x26] & 0x0F, 0);
}
