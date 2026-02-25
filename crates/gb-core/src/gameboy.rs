use crate::Cycles;
use crate::apu::{Apu, ApuDiagnostics};
use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cartridge::save::CartridgeSave;
use crate::config::GbConfig;
use crate::cpu::Cpu;
use crate::debug::{DebugBreakReason, DebugEvent, DebugSink, Debugger, Watchpoint};
use crate::dma::OamDma;
use crate::error::GbError;
use crate::interrupts::InterruptController;
use crate::joypad::GbButton;
use crate::ppu::{Ppu, PpuFrameDiagnostics, SCREEN_HEIGHT, SCREEN_WIDTH};
use crate::scheduler::Clock;
use crate::timers::Timers;

pub struct GameBoy {
    pub cpu: Cpu,
    pub bus: Bus,
    pub clock: Clock,
    pub timers: Timers,
    pub ppu: Ppu,
    pub apu: Apu,
    pub interrupts: InterruptController,
    pub dma: OamDma,
    pub config: GbConfig,
    debugger: Debugger,
}

impl GameBoy {
    pub fn new(config: GbConfig, rom: Vec<u8>) -> Result<Self, GbError> {
        let cartridge = Cartridge::from_rom_bytes(rom)?;
        let mut cpu = Cpu::new();
        let mut bus = Bus::new(cartridge);

        if config.use_boot_rom {
            if let Some(boot_rom) = config.boot_rom {
                bus.install_boot_rom(boot_rom);
            } else if config.force_post_boot_state {
                cpu.apply_post_boot_state();
                bus.apply_post_boot_state();
            }
        } else if config.force_post_boot_state {
            cpu.apply_post_boot_state();
            bus.apply_post_boot_state();
        }

        let mut gameboy = Self {
            cpu,
            bus,
            clock: Clock::new(),
            timers: Timers::new(),
            ppu: Ppu::new(),
            apu: Apu::new(),
            interrupts: InterruptController::new(),
            dma: OamDma::new(),
            config,
            debugger: Debugger::new(),
        };
        gameboy.sync_debug_to_bus();

        Ok(gameboy)
    }

    pub fn step(&mut self) -> Result<Cycles, GbError> {
        self.sync_interrupts_from_bus();
        self.sync_timers_from_bus();

        if self.debugger.enabled() {
            let pc = self.cpu.registers().pc;
            let opcode = self.bus.peek8(pc)?;
            self.debugger.on_instruction(pc, opcode);
            if self.debugger.has_break() {
                return Ok(0);
            }
        }

        let cycles = self.cpu.step(&mut self.bus, &mut self.interrupts)?;
        self.sync_interrupts_from_bus();
        self.clock.tick(cycles);
        self.timers.tick(cycles, &mut self.interrupts);
        if let Some(source_high) = self.bus.take_dma_start_request() {
            self.dma.start(source_high, &mut self.bus)?;
        }
        self.dma.tick(cycles, &mut self.bus)?;
        self.bus.tick_serial(cycles, &mut self.interrupts);
        self.ppu.tick(cycles, &mut self.bus, &mut self.interrupts)?;
        let apu_write_mask = self.bus.take_apu_write_mask();
        self.apu.tick(cycles, &mut self.bus.io, apu_write_mask)?;

        if let Some(vector) = self.cpu.take_last_serviced_interrupt_vector() {
            self.debugger
                .on_event(DebugEvent::InterruptServiced { vector });
        }
        for event in self.bus.take_debug_events() {
            self.debugger.on_event(event);
        }

        self.sync_timers_to_bus();
        self.sync_interrupts_to_bus();

        Ok(cycles)
    }

    pub fn run_cycles(&mut self, budget: Cycles) -> Result<Cycles, GbError> {
        let mut spent = 0;
        while spent < budget {
            let cycles = self.step()?;
            spent = spent.wrapping_add(cycles);
            if self.debugger.has_break() {
                break;
            }
        }
        Ok(spent)
    }

    #[must_use]
    pub fn take_frame(&mut self) -> Option<[u8; SCREEN_WIDTH * SCREEN_HEIGHT]> {
        self.ppu.take_frame()
    }

    #[must_use]
    pub fn take_audio_samples(&mut self) -> Vec<i16> {
        let mut out = Vec::new();
        self.drain_audio_samples_into(&mut out);
        out
    }

    pub fn drain_audio_samples_into(&mut self, out: &mut Vec<i16>) {
        self.apu.drain_samples_into(out);
    }

    pub fn set_button(&mut self, button: GbButton, pressed: bool) -> Result<(), GbError> {
        if pressed {
            self.bus.joypad.press(button, &mut self.interrupts);
        } else {
            self.bus.joypad.release(button);
        }
        self.sync_interrupts_to_bus();
        Ok(())
    }

    pub fn export_save(&self) -> Result<CartridgeSave, GbError> {
        Ok(self.bus.cartridge.export_save())
    }

    pub fn import_save(&mut self, save: &CartridgeSave) -> Result<(), GbError> {
        self.bus.cartridge.import_save(save)?;
        Ok(())
    }

    pub fn export_snapshot(&self) -> Result<Vec<u8>, GbError> {
        Ok(crate::state::snapshot::encode(self)?)
    }

    pub fn import_snapshot(&mut self, bytes: &[u8]) -> Result<(), GbError> {
        let payload = crate::state::snapshot::decode(bytes)?;
        self.cpu = payload.cpu;
        self.bus = payload.bus;
        self.clock = payload.clock;
        self.timers = payload.timers;
        self.ppu = payload.ppu;
        self.apu = payload.apu;
        self.interrupts = payload.interrupts;
        self.dma = payload.dma;
        self.config = payload.config;

        self.debugger.clear_break();
        self.sync_debug_to_bus();
        Ok(())
    }

    pub fn tick_rtc(&mut self, elapsed_seconds: u64) -> Result<(), GbError> {
        self.bus.cartridge.tick_rtc(elapsed_seconds)?;
        Ok(())
    }

    pub fn set_debug_enabled(&mut self, enabled: bool) {
        self.debugger.set_enabled(enabled);
        self.sync_debug_to_bus();
    }

    pub fn set_ppu_diagnostics_enabled(&mut self, enabled: bool) {
        self.ppu.set_diagnostics_enabled(enabled);
    }

    pub fn set_apu_diagnostics_enabled(&mut self, enabled: bool, include_channel_stats: bool) {
        self.apu
            .set_diagnostics_enabled(enabled, include_channel_stats);
    }

    #[must_use]
    pub fn apu_diagnostics(&self) -> Option<ApuDiagnostics> {
        self.apu.diagnostics()
    }

    #[must_use]
    pub fn take_ppu_frame_diagnostics(&mut self) -> Option<PpuFrameDiagnostics> {
        self.ppu.take_frame_diagnostics()
    }

    pub fn set_debug_sink(&mut self, sink: Option<Box<dyn DebugSink>>) {
        self.debugger.set_sink(sink);
        self.sync_debug_to_bus();
    }

    pub fn add_pc_breakpoint(&mut self, pc: u16) {
        self.debugger.add_pc_breakpoint(pc);
    }

    pub fn add_opcode_breakpoint(&mut self, opcode: u8) {
        self.debugger.add_opcode_breakpoint(opcode);
    }

    pub fn clear_breakpoints(&mut self) {
        self.debugger.clear_breakpoints();
        self.debugger.clear_break();
    }

    pub fn add_watchpoint(&mut self, watchpoint: Watchpoint) {
        self.debugger.add_watchpoint(watchpoint);
        self.sync_debug_to_bus();
    }

    pub fn clear_watchpoints(&mut self) {
        self.debugger.clear_watchpoints();
        self.sync_debug_to_bus();
    }

    pub fn clear_debug_break(&mut self) {
        self.debugger.clear_break();
    }

    #[must_use]
    pub fn debug_break_active(&self) -> bool {
        self.debugger.has_break()
    }

    #[must_use]
    pub fn take_debug_break(&mut self) -> Option<DebugBreakReason> {
        self.debugger.take_break()
    }

    fn sync_interrupts_from_bus(&mut self) {
        self.interrupts.ie = self.bus.read_ie();
        self.interrupts.iflags = self.bus.read_if();
    }

    fn sync_interrupts_to_bus(&mut self) {
        self.bus.write_ie(self.interrupts.ie);
        self.bus.write_if(self.interrupts.iflags);
    }

    fn sync_timers_from_bus(&mut self) {
        let regs = self.bus.timer_regs();
        self.timers.apply_bus_regs(regs);
    }

    fn sync_timers_to_bus(&mut self) {
        let regs = self.timers.export_regs();
        self.bus.set_timer_regs(regs);
    }

    fn sync_debug_to_bus(&mut self) {
        self.bus.configure_debug_tracing(
            self.debugger.memory_trace_enabled(),
            self.debugger.watchpoints(),
        );
    }
}
