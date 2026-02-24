use gb_core::interrupts::{Interrupt, InterruptController};
use gb_core::serial::Serial;

#[test]
fn serial_start_bit_with_internal_clock_begins_transfer() {
    let mut serial = Serial::new();
    let mut interrupts = InterruptController::new();

    serial.write_sb(0x12);
    serial.write_sc(0x80);
    serial.tick(4_096, &mut interrupts);

    assert_eq!(serial.read_sb(), 0x12);
    assert_eq!(interrupts.iflags & Interrupt::Serial.bit(), 0);

    serial.write_sc(0x81);
    assert!(serial.is_transfer_active());
}

#[test]
fn serial_transfer_completes_after_modeled_cycles_and_requests_interrupt() {
    let mut serial = Serial::new();
    let mut interrupts = InterruptController::new();

    serial.write_sb(0xAB);
    serial.write_sc(0x81);
    serial.tick(4_095, &mut interrupts);
    assert!(serial.is_transfer_active());
    assert_eq!(interrupts.iflags & Interrupt::Serial.bit(), 0);

    serial.tick(1, &mut interrupts);
    assert!(!serial.is_transfer_active());
    assert_ne!(interrupts.iflags & Interrupt::Serial.bit(), 0);
}

#[test]
fn serial_uses_deterministic_received_byte_policy() {
    let mut serial = Serial::new();
    let mut interrupts = InterruptController::new();

    serial.write_sb(0x00);
    serial.write_sc(0x81);
    serial.tick(4_096, &mut interrupts);

    assert_eq!(serial.read_sb(), 0xFF);
}
