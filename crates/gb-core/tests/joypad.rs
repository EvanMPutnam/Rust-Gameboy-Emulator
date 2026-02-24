use gb_core::interrupts::{Interrupt, InterruptController};
use gb_core::joypad::{GbButton, Joypad};

#[test]
fn p1_select_lines_switch_between_dpad_and_buttons() {
    let mut joypad = Joypad::new();
    let mut interrupts = InterruptController::new();

    joypad.write_p1(0x20);
    joypad.press(GbButton::Right, &mut interrupts);
    assert_eq!(joypad.read_p1() & 0x0F, 0x0E);

    joypad.write_p1(0x10);
    assert_eq!(joypad.read_p1() & 0x0F, 0x0F);

    joypad.press(GbButton::A, &mut interrupts);
    assert_eq!(joypad.read_p1() & 0x0F, 0x0E);
}

#[test]
fn pressed_buttons_are_active_low_and_release_restores_high() {
    let mut joypad = Joypad::new();
    let mut interrupts = InterruptController::new();
    joypad.write_p1(0x10);

    joypad.press(GbButton::Start, &mut interrupts);
    assert_eq!(joypad.read_p1() & 0x08, 0);

    joypad.release(GbButton::Start);
    assert_ne!(joypad.read_p1() & 0x08, 0);
}

#[test]
fn joypad_interrupt_requests_on_high_to_low_transition() {
    let mut joypad = Joypad::new();
    let mut interrupts = InterruptController::new();

    joypad.write_p1(0x20);
    joypad.press(GbButton::Left, &mut interrupts);
    assert_ne!(interrupts.iflags & Interrupt::Joypad.bit(), 0);

    interrupts.iflags = 0;
    joypad.press(GbButton::Left, &mut interrupts);
    assert_eq!(interrupts.iflags & Interrupt::Joypad.bit(), 0);
}
