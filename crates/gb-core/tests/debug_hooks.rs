use std::cell::RefCell;
use std::rc::Rc;

use gb_core::debug::{DebugBreakReason, DebugEvent, DebugSink, Watchpoint};
use gb_core::{GameBoy, GbConfig};

fn rom_with_program(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
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
    ] {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
    }

    hash ^ u64::from(gb.cpu.registers().pc) ^ (u64::from(gb.clock.now()) << 16)
}

struct SharedSink {
    events: Rc<RefCell<Vec<DebugEvent>>>,
}

impl DebugSink for SharedSink {
    fn on_event(&mut self, event: &DebugEvent) {
        self.events.borrow_mut().push(*event);
    }
}

#[test]
fn pc_breakpoint_stops_execution_at_expected_pc() {
    let mut gb =
        GameBoy::new(GbConfig::default(), rom_with_program(&[0x00, 0x00])).expect("init gb");
    gb.set_debug_enabled(true);
    gb.add_pc_breakpoint(0x0100);

    let spent = gb.run_cycles(64).expect("run cycles");

    assert_eq!(spent, 0);
    assert_eq!(
        gb.take_debug_break(),
        Some(DebugBreakReason::PcBreakpoint { pc: 0x0100 })
    );
}

#[test]
fn watchpoint_fires_only_for_configured_address_and_access() {
    let mut gb = GameBoy::new(
        GbConfig::default(),
        rom_with_program(&[
            0x3E, 0x12, // LD A,12
            0xEA, 0x00, 0xC0, // LD (C000),A
            0xEA, 0x01, 0xC0, // LD (C001),A
            0x00,
        ]),
    )
    .expect("init gb");

    gb.set_debug_enabled(true);
    gb.add_watchpoint(Watchpoint::write(0xC000));

    let _ = gb.run_cycles(256).expect("run cycles");

    assert_eq!(
        gb.take_debug_break(),
        Some(DebugBreakReason::WatchpointWrite {
            addr: 0xC000,
            value: 0x12
        })
    );
}

#[test]
fn debug_disabled_emits_no_events_and_keeps_deterministic_results() {
    let rom = rom_with_program(&[0x00, 0x00, 0x00, 0x00]);

    let mut baseline = GameBoy::new(GbConfig::default(), rom.clone()).expect("init baseline");
    baseline.run_cycles(8_192).expect("run baseline");

    let shared_events = Rc::new(RefCell::new(Vec::new()));
    let sink = SharedSink {
        events: Rc::clone(&shared_events),
    };

    let mut debug_disabled = GameBoy::new(GbConfig::default(), rom).expect("init debug");
    debug_disabled.set_debug_sink(Some(Box::new(sink)));
    debug_disabled.set_debug_enabled(false);
    debug_disabled
        .run_cycles(8_192)
        .expect("run debug disabled");

    assert_eq!(state_hash(&baseline), state_hash(&debug_disabled));
    assert!(shared_events.borrow().is_empty());
}

#[test]
fn debug_sink_receives_cpu_and_memory_events_when_enabled() {
    let shared_events = Rc::new(RefCell::new(Vec::new()));
    let sink = SharedSink {
        events: Rc::clone(&shared_events),
    };

    let mut gb = GameBoy::new(
        GbConfig::default(),
        rom_with_program(&[0x3E, 0x77, 0xEA, 0x00, 0xC0, 0x00]),
    )
    .expect("init gb");
    gb.set_debug_sink(Some(Box::new(sink)));
    gb.set_debug_enabled(true);

    gb.run_cycles(128).expect("run with debug");

    let events = shared_events.borrow();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, DebugEvent::CpuStep { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        DebugEvent::MemWrite {
            addr: 0xC000,
            value: 0x77
        }
    )));
}
