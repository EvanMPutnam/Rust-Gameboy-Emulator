use gb_core::bus::{Bus, Memory};
use gb_core::cartridge::Cartridge;
use gb_core::cpu::{Cpu, Flags, Registers};
use gb_core::error::CpuError;
use gb_core::interrupts::InterruptController;

fn test_rom(program: &[u8], cart_type: u8, rom_code: u8, ext_ram_code: u8) -> Vec<u8> {
    let banks = if rom_code == 0x01 { 4 } else { 2 };

    let mut rom = vec![0u8; banks * 0x4000];
    rom[0x0147] = cart_type;
    rom[0x0148] = rom_code;
    rom[0x0149] = ext_ram_code;
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom
}

fn cpu_and_bus(program: &[u8]) -> (Cpu, Bus, InterruptController) {
    let rom = test_rom(program, 0x00, 0x00, 0x00);
    let cart = Cartridge::from_rom_bytes(rom).expect("rom should parse");
    (Cpu::new(), Bus::new(cart), InterruptController::new())
}

#[test]
fn registers_mask_af_low_nibble() {
    let mut regs = Registers::default();
    regs.set_af(0x12FF);

    assert_eq!(regs.a, 0x12);
    assert_eq!(regs.af(), 0x12F0);
}

#[test]
fn executes_ld_a_imm_and_xor_a() {
    let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[0x3E, 0x42, 0xAF, 0x00]);

    let cycles_1 = cpu.step(&mut bus, &mut interrupts).expect("ld should run");
    assert_eq!(cycles_1, 8);
    assert_eq!(cpu.registers().a, 0x42);

    let cycles_2 = cpu.step(&mut bus, &mut interrupts).expect("xor should run");
    assert_eq!(cycles_2, 4);
    assert_eq!(cpu.registers().a, 0x00);
    assert!(cpu.registers().flag(Flags::Z));
    assert!(!cpu.registers().flag(Flags::N));
    assert!(!cpu.registers().flag(Flags::H));
    assert!(!cpu.registers().flag(Flags::C));
}

#[test]
fn executes_jp_and_jr() {
    let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[
        0xC3, 0x06, 0x01, 0x00, 0x00, 0x00, 0x3E, 0x99, 0x18, 0x01, 0x00, 0x00,
    ]);

    let jump_cycles = cpu.step(&mut bus, &mut interrupts).expect("jp should run");
    assert_eq!(jump_cycles, 16);
    assert_eq!(cpu.registers().pc, 0x0106);

    let ld_cycles = cpu.step(&mut bus, &mut interrupts).expect("ld should run");
    assert_eq!(ld_cycles, 8);
    assert_eq!(cpu.registers().a, 0x99);

    let relative_cycles = cpu.step(&mut bus, &mut interrupts).expect("jr should run");
    assert_eq!(relative_cycles, 12);
    assert_eq!(cpu.registers().pc, 0x010B);
}

#[test]
fn executes_call_and_ret() {
    let (mut cpu, mut bus, mut interrupts) =
        cpu_and_bus(&[0xCD, 0x06, 0x01, 0x3E, 0x11, 0x00, 0x3E, 0x22, 0xC9]);

    let call_cycles = cpu
        .step(&mut bus, &mut interrupts)
        .expect("call should run");
    assert_eq!(call_cycles, 24);
    assert_eq!(cpu.registers().pc, 0x0106);

    let sub_ld = cpu
        .step(&mut bus, &mut interrupts)
        .expect("sub ld should run");
    assert_eq!(sub_ld, 8);
    assert_eq!(cpu.registers().a, 0x22);

    let ret_cycles = cpu.step(&mut bus, &mut interrupts).expect("ret should run");
    assert_eq!(ret_cycles, 16);
    assert_eq!(cpu.registers().pc, 0x0103);

    let post_ld = cpu
        .step(&mut bus, &mut interrupts)
        .expect("post ld should run");
    assert_eq!(post_ld, 8);
    assert_eq!(cpu.registers().a, 0x11);
}

#[test]
fn executes_cb_prefix_opcode() {
    let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[0x06, 0x80, 0xCB, 0x00]);

    cpu.step(&mut bus, &mut interrupts)
        .expect("ld b,imm should run");
    let cycles = cpu
        .step(&mut bus, &mut interrupts)
        .expect("cb rlc b should run");

    assert_eq!(cycles, 8);
    assert_eq!(cpu.registers().b, 0x01);
    assert!(cpu.registers().flag(Flags::C));
}

#[test]
fn ei_enables_ime_after_next_instruction_boundary() {
    let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[0xFB, 0x00]);

    cpu.set_ime(false);

    cpu.step(&mut bus, &mut interrupts).expect("ei should run");
    assert!(!cpu.ime());

    cpu.step(&mut bus, &mut interrupts).expect("nop should run");
    assert!(cpu.ime());
}

#[test]
fn ld_hl_indirect_round_trip() {
    let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[0x3E, 0x5A, 0x77, 0x3E, 0x00, 0x7E]);

    cpu.registers_mut().set_hl(0xC123);

    cpu.step(&mut bus, &mut interrupts).expect("ld a,imm");
    cpu.step(&mut bus, &mut interrupts).expect("ld (hl),a");
    cpu.step(&mut bus, &mut interrupts).expect("ld a,imm");
    cpu.step(&mut bus, &mut interrupts).expect("ld a,(hl)");

    assert_eq!(bus.read8(0xC123).expect("wram read"), 0x5A);
    assert_eq!(cpu.registers().a, 0x5A);
}

#[test]
fn executes_cp_imm8_and_updates_flags() {
    let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[0x3E, 0x42, 0xFE, 0x42, 0xFE, 0x50]);

    cpu.step(&mut bus, &mut interrupts).expect("ld a,imm");

    let equal_cycles = cpu.step(&mut bus, &mut interrupts).expect("cp d8 equal");
    assert_eq!(equal_cycles, 8);
    assert!(cpu.registers().flag(Flags::Z));
    assert!(cpu.registers().flag(Flags::N));
    assert!(!cpu.registers().flag(Flags::H));
    assert!(!cpu.registers().flag(Flags::C));
    assert_eq!(cpu.registers().a, 0x42);

    let less_cycles = cpu
        .step(&mut bus, &mut interrupts)
        .expect("cp d8 borrow and half-borrow");
    assert_eq!(less_cycles, 8);
    assert!(!cpu.registers().flag(Flags::Z));
    assert!(cpu.registers().flag(Flags::N));
    assert!(!cpu.registers().flag(Flags::H));
    assert!(cpu.registers().flag(Flags::C));
    assert_eq!(cpu.registers().a, 0x42);
}

#[test]
fn supports_all_base_opcodes_except_known_illegal_values() {
    let illegal = [
        0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD,
    ];

    for opcode in u8::MIN..=u8::MAX {
        let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[opcode, 0x00, 0x00, 0x00]);
        cpu.registers_mut().set_hl(0xC000);
        cpu.registers_mut().sp = 0xFFFE;

        match cpu.step(&mut bus, &mut interrupts) {
            Ok(_) => {
                assert!(
                    !illegal.contains(&opcode),
                    "opcode {opcode:#04X} should be illegal"
                );
            }
            Err(CpuError::UnsupportedOpcode(found)) => {
                assert_eq!(found, opcode);
                assert!(
                    illegal.contains(&opcode),
                    "opcode {opcode:#04X} unexpectedly unsupported"
                );
            }
            Err(other) => panic!("opcode {opcode:#04X} failed with unexpected error: {other:?}"),
        }
    }
}

#[test]
fn supports_all_cb_prefixed_opcodes() {
    for opcode in u8::MIN..=u8::MAX {
        let (mut cpu, mut bus, mut interrupts) = cpu_and_bus(&[0xCB, opcode, 0x00, 0x00]);
        cpu.registers_mut().set_hl(0xC000);
        cpu.registers_mut().sp = 0xFFFE;

        if let Err(err) = cpu.step(&mut bus, &mut interrupts) {
            panic!("cb opcode {opcode:#04X} failed: {err:?}");
        }
    }
}
