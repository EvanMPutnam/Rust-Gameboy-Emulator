use gb_core::cartridge::rtc::Rtc;

#[test]
fn latch_captures_stable_snapshot() {
    let mut rtc = Rtc::new();
    assert!(rtc.write_register(0x08, 10));
    rtc.latch();
    assert!(rtc.write_register(0x08, 22));

    assert_eq!(rtc.read_register(0x08), Some(10));
}

#[test]
fn halt_stops_progress_until_resumed() {
    let mut rtc = Rtc::new();
    assert!(rtc.write_register(0x08, 30));
    assert!(rtc.write_register(0x0C, 0x40));
    rtc.tick(120);

    let halted = rtc.export_state();
    assert_eq!(halted.seconds, 30);

    assert!(rtc.write_register(0x0C, 0x00));
    rtc.tick(30);
    let resumed = rtc.export_state();
    assert_eq!(resumed.seconds, 0);
    assert_eq!(resumed.minutes, 1);
}

#[test]
fn day_overflow_sets_carry_and_wraps_counter() {
    let mut rtc = Rtc::new();
    assert!(rtc.write_register(0x08, 59));
    assert!(rtc.write_register(0x09, 59));
    assert!(rtc.write_register(0x0A, 23));
    assert!(rtc.write_register(0x0B, 0xFF));
    assert!(rtc.write_register(0x0C, 0x01));

    rtc.tick(1);
    let state = rtc.export_state();
    assert_eq!(state.day_counter, 0);
    assert!(state.day_carry);
}
