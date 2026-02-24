use gb_core::interrupts::{Interrupt, InterruptController};
use gb_core::timers::Timers;

#[test]
fn timer_overflow_requests_interrupt_and_reloads_tma() {
    let mut timers = Timers::new();
    let mut interrupts = InterruptController::new();

    timers.apply_bus_regs(gb_core::bus::TimerRegs {
        div: 0,
        tima: 0xFF,
        tma: 0x3C,
        tac: 0b101,
    });

    timers.tick(16, &mut interrupts);
    let regs = timers.export_regs();

    assert_eq!(regs.tima, 0x3C);
    assert_eq!(
        interrupts.iflags & Interrupt::Timer.bit(),
        Interrupt::Timer.bit()
    );
}
