#![allow(
    clippy::too_many_lines,
    reason = "LR35902 opcode execution is intentionally implemented as a single explicit matrix."
)]
#![allow(
    clippy::many_single_char_names,
    reason = "CPU register naming and opcode matrix decomposition naturally use short hardware names."
)]

use crate::Cycles;
use crate::bus::Memory;
use crate::cpu::core::Cpu;
use crate::cpu::decode::Instruction;
use crate::cpu::registers::Flags;
use crate::error::CpuError;

pub fn execute(
    cpu: &mut Cpu,
    mem: &mut dyn Memory,
    instruction: Instruction,
) -> Result<Cycles, CpuError> {
    match instruction {
        Instruction::Base(opcode) => execute_base(cpu, mem, opcode),
        Instruction::PrefixCb => {
            let cb_opcode = cpu.fetch8(mem)?;
            execute_cb(cpu, mem, cb_opcode)
        }
    }
}

fn execute_base(cpu: &mut Cpu, mem: &mut dyn Memory, opcode: u8) -> Result<Cycles, CpuError> {
    if is_invalid_base_opcode(opcode) {
        return Err(CpuError::UnsupportedOpcode(opcode));
    }

    let x = opcode >> 6;
    let y = (opcode >> 3) & 0x07;
    let z = opcode & 0x07;
    let p = y >> 1;
    let q = y & 0x01;

    match x {
        0 => execute_block0(cpu, mem, y, z, p, q),
        1 => execute_block1(cpu, mem, y, z),
        2 => execute_block2(cpu, mem, y, z),
        3 => execute_block3(cpu, mem, opcode, y, z, p, q),
        _ => Err(CpuError::UnsupportedOpcode(opcode)),
    }
}

fn execute_block0(
    cpu: &mut Cpu,
    mem: &mut dyn Memory,
    y: u8,
    z: u8,
    p: u8,
    q: u8,
) -> Result<Cycles, CpuError> {
    match z {
        0 => match y {
            0 => Ok(4), // NOP
            1 => {
                // LD (a16),SP
                let addr = cpu.fetch16(mem)?;
                let [sp_lo, sp_hi] = cpu.registers().sp.to_le_bytes();
                mem.write8(addr, sp_lo)?;
                mem.write8(addr.wrapping_add(1), sp_hi)?;
                Ok(20)
            }
            2 => {
                // STOP 0
                let _ = cpu.fetch8(mem)?;
                Ok(4)
            }
            3 => {
                // JR e8
                let offset = i8::from_ne_bytes([cpu.fetch8(mem)?]);
                cpu.jump_relative(offset);
                Ok(12)
            }
            4..=7 => {
                // JR cc,e8
                let offset = i8::from_ne_bytes([cpu.fetch8(mem)?]);
                if cond_true(cpu, y - 4) {
                    cpu.jump_relative(offset);
                    Ok(12)
                } else {
                    Ok(8)
                }
            }
            _ => Err(CpuError::UnsupportedOpcode((y << 3) | z)),
        },
        1 => {
            if q == 0 {
                // LD rp[p],d16
                let value = cpu.fetch16(mem)?;
                set_rp(cpu, p, value);
                Ok(12)
            } else {
                // ADD HL,rp[p]
                let rhs = read_rp(cpu, p);
                add_hl(cpu, rhs);
                Ok(8)
            }
        }
        2 => {
            if q == 0 {
                // LD (rp[p]),A and LD (HL+/-),A
                let addr = match p {
                    0 => cpu.registers().bc(),
                    1 => cpu.registers().de(),
                    2 | 3 => cpu.registers().hl(),
                    _ => return Err(CpuError::UnsupportedOpcode((y << 3) | z)),
                };
                let a = cpu.registers().a;
                mem.write8(addr, a)?;
                if p == 2 {
                    cpu.registers_mut().set_hl(addr.wrapping_add(1));
                } else if p == 3 {
                    cpu.registers_mut().set_hl(addr.wrapping_sub(1));
                }
                Ok(8)
            } else {
                // LD A,(rp[p]) and LD A,(HL+/-)
                let addr = match p {
                    0 => cpu.registers().bc(),
                    1 => cpu.registers().de(),
                    2 | 3 => cpu.registers().hl(),
                    _ => return Err(CpuError::UnsupportedOpcode((y << 3) | z)),
                };
                let value = mem.read8(addr)?;
                cpu.registers_mut().a = value;
                if p == 2 {
                    cpu.registers_mut().set_hl(addr.wrapping_add(1));
                } else if p == 3 {
                    cpu.registers_mut().set_hl(addr.wrapping_sub(1));
                }
                Ok(8)
            }
        }
        3 => {
            if q == 0 {
                // INC rp[p]
                let value = read_rp(cpu, p).wrapping_add(1);
                set_rp(cpu, p, value);
            } else {
                // DEC rp[p]
                let value = read_rp(cpu, p).wrapping_sub(1);
                set_rp(cpu, p, value);
            }
            Ok(8)
        }
        4 => {
            // INC r[y]
            let value = read_r8(cpu, mem, y)?;
            let result = value.wrapping_add(1);
            write_r8(cpu, mem, y, result)?;

            let half = (value & 0x0F) + 1 > 0x0F;
            cpu.registers_mut().set_flag(Flags::Z, result == 0);
            cpu.registers_mut().set_flag(Flags::N, false);
            cpu.registers_mut().set_flag(Flags::H, half);

            if y == 6 { Ok(12) } else { Ok(4) }
        }
        5 => {
            // DEC r[y]
            let value = read_r8(cpu, mem, y)?;
            let result = value.wrapping_sub(1);
            write_r8(cpu, mem, y, result)?;

            let half = value.trailing_zeros() >= 4;
            cpu.registers_mut().set_flag(Flags::Z, result == 0);
            cpu.registers_mut().set_flag(Flags::N, true);
            cpu.registers_mut().set_flag(Flags::H, half);

            if y == 6 { Ok(12) } else { Ok(4) }
        }
        6 => {
            // LD r[y],d8
            let imm = cpu.fetch8(mem)?;
            write_r8(cpu, mem, y, imm)?;
            if y == 6 { Ok(12) } else { Ok(8) }
        }
        7 => match y {
            0 => {
                // RLCA
                let a = cpu.registers().a;
                let result = a.rotate_left(1);
                cpu.registers_mut().a = result;
                cpu.registers_mut().set_flag(Flags::Z, false);
                cpu.registers_mut().set_flag(Flags::N, false);
                cpu.registers_mut().set_flag(Flags::H, false);
                cpu.registers_mut().set_flag(Flags::C, (a & 0x80) != 0);
                Ok(4)
            }
            1 => {
                // RRCA
                let a = cpu.registers().a;
                let result = a.rotate_right(1);
                cpu.registers_mut().a = result;
                cpu.registers_mut().set_flag(Flags::Z, false);
                cpu.registers_mut().set_flag(Flags::N, false);
                cpu.registers_mut().set_flag(Flags::H, false);
                cpu.registers_mut().set_flag(Flags::C, (a & 0x01) != 0);
                Ok(4)
            }
            2 => {
                // RLA
                let a = cpu.registers().a;
                let carry_in = u8::from(cpu.registers().flag(Flags::C));
                let result = (a << 1) | carry_in;
                cpu.registers_mut().a = result;
                cpu.registers_mut().set_flag(Flags::Z, false);
                cpu.registers_mut().set_flag(Flags::N, false);
                cpu.registers_mut().set_flag(Flags::H, false);
                cpu.registers_mut().set_flag(Flags::C, (a & 0x80) != 0);
                Ok(4)
            }
            3 => {
                // RRA
                let a = cpu.registers().a;
                let carry_in = if cpu.registers().flag(Flags::C) {
                    0x80
                } else {
                    0x00
                };
                let result = (a >> 1) | carry_in;
                cpu.registers_mut().a = result;
                cpu.registers_mut().set_flag(Flags::Z, false);
                cpu.registers_mut().set_flag(Flags::N, false);
                cpu.registers_mut().set_flag(Flags::H, false);
                cpu.registers_mut().set_flag(Flags::C, (a & 0x01) != 0);
                Ok(4)
            }
            4 => {
                // DAA
                daa(cpu);
                Ok(4)
            }
            5 => {
                // CPL
                let a = cpu.registers().a;
                cpu.registers_mut().a = !a;
                cpu.registers_mut().set_flag(Flags::N, true);
                cpu.registers_mut().set_flag(Flags::H, true);
                Ok(4)
            }
            6 => {
                // SCF
                cpu.registers_mut().set_flag(Flags::N, false);
                cpu.registers_mut().set_flag(Flags::H, false);
                cpu.registers_mut().set_flag(Flags::C, true);
                Ok(4)
            }
            7 => {
                // CCF
                let carry = cpu.registers().flag(Flags::C);
                cpu.registers_mut().set_flag(Flags::N, false);
                cpu.registers_mut().set_flag(Flags::H, false);
                cpu.registers_mut().set_flag(Flags::C, !carry);
                Ok(4)
            }
            _ => Err(CpuError::UnsupportedOpcode((y << 3) | z)),
        },
        _ => Err(CpuError::UnsupportedOpcode((y << 3) | z)),
    }
}

fn execute_block1(cpu: &mut Cpu, mem: &mut dyn Memory, y: u8, z: u8) -> Result<Cycles, CpuError> {
    if y == 6 && z == 6 {
        // HALT
        cpu.set_halted();
        return Ok(4);
    }

    // LD r[y],r[z]
    let value = read_r8(cpu, mem, z)?;
    write_r8(cpu, mem, y, value)?;
    if y == 6 || z == 6 { Ok(8) } else { Ok(4) }
}

fn execute_block2(cpu: &mut Cpu, mem: &mut dyn Memory, y: u8, z: u8) -> Result<Cycles, CpuError> {
    // ALU[y] A,r[z]
    let operand = read_r8(cpu, mem, z)?;
    apply_alu(cpu, y, operand);
    if z == 6 { Ok(8) } else { Ok(4) }
}

fn execute_block3(
    cpu: &mut Cpu,
    mem: &mut dyn Memory,
    opcode: u8,
    y: u8,
    z: u8,
    p: u8,
    q: u8,
) -> Result<Cycles, CpuError> {
    match z {
        0 => match y {
            0..=3 => {
                // RET cc
                if cond_true(cpu, y) {
                    let addr = cpu.pop_word(mem)?;
                    cpu.registers_mut().pc = addr;
                    Ok(20)
                } else {
                    Ok(8)
                }
            }
            4 => {
                // LDH (a8),A
                let offset = cpu.fetch8(mem)?;
                let addr = 0xFF00u16 | u16::from(offset);
                mem.write8(addr, cpu.registers().a)?;
                Ok(12)
            }
            5 => {
                // ADD SP,e8
                let offset = i8::from_ne_bytes([cpu.fetch8(mem)?]);
                let result = add_sp_signed(cpu, offset);
                cpu.registers_mut().sp = result;
                Ok(16)
            }
            6 => {
                // LDH A,(a8)
                let offset = cpu.fetch8(mem)?;
                let addr = 0xFF00u16 | u16::from(offset);
                cpu.registers_mut().a = mem.read8(addr)?;
                Ok(12)
            }
            7 => {
                // LD HL,SP+e8
                let offset = i8::from_ne_bytes([cpu.fetch8(mem)?]);
                let result = add_sp_signed(cpu, offset);
                cpu.registers_mut().set_hl(result);
                Ok(12)
            }
            _ => Err(CpuError::UnsupportedOpcode(opcode)),
        },
        1 => {
            if q == 0 {
                // POP rp2[p]
                let value = cpu.pop_word(mem)?;
                set_rp2(cpu, p, value);
                Ok(12)
            } else {
                match p {
                    0 => {
                        // RET
                        let addr = cpu.pop_word(mem)?;
                        cpu.registers_mut().pc = addr;
                        Ok(16)
                    }
                    1 => {
                        // RETI
                        let addr = cpu.pop_word(mem)?;
                        cpu.registers_mut().pc = addr;
                        cpu.set_ime(true);
                        Ok(16)
                    }
                    2 => {
                        // JP (HL)
                        cpu.registers_mut().pc = cpu.registers().hl();
                        Ok(4)
                    }
                    3 => {
                        // LD SP,HL
                        cpu.registers_mut().sp = cpu.registers().hl();
                        Ok(8)
                    }
                    _ => Err(CpuError::UnsupportedOpcode(opcode)),
                }
            }
        }
        2 => match y {
            0..=3 => {
                // JP cc,a16
                let addr = cpu.fetch16(mem)?;
                if cond_true(cpu, y) {
                    cpu.registers_mut().pc = addr;
                    Ok(16)
                } else {
                    Ok(12)
                }
            }
            4 => {
                // LD (C),A
                let addr = 0xFF00u16 | u16::from(cpu.registers().c);
                mem.write8(addr, cpu.registers().a)?;
                Ok(8)
            }
            5 => {
                // LD (a16),A
                let addr = cpu.fetch16(mem)?;
                mem.write8(addr, cpu.registers().a)?;
                Ok(16)
            }
            6 => {
                // LD A,(C)
                let addr = 0xFF00u16 | u16::from(cpu.registers().c);
                cpu.registers_mut().a = mem.read8(addr)?;
                Ok(8)
            }
            7 => {
                // LD A,(a16)
                let addr = cpu.fetch16(mem)?;
                cpu.registers_mut().a = mem.read8(addr)?;
                Ok(16)
            }
            _ => Err(CpuError::UnsupportedOpcode(opcode)),
        },
        3 => match y {
            0 => {
                // JP a16
                let addr = cpu.fetch16(mem)?;
                cpu.registers_mut().pc = addr;
                Ok(16)
            }
            6 => {
                // DI
                cpu.disable_ime();
                Ok(4)
            }
            7 => {
                // EI
                cpu.schedule_ime_enable();
                Ok(4)
            }
            _ => Err(CpuError::UnsupportedOpcode(opcode)),
        },
        4 => match y {
            0..=3 => {
                // CALL cc,a16
                let target = cpu.fetch16(mem)?;
                if cond_true(cpu, y) {
                    let ret_addr = cpu.registers().pc;
                    cpu.push_word(mem, ret_addr)?;
                    cpu.registers_mut().pc = target;
                    Ok(24)
                } else {
                    Ok(12)
                }
            }
            _ => Err(CpuError::UnsupportedOpcode(opcode)),
        },
        5 => {
            if q == 0 {
                // PUSH rp2[p]
                let value = read_rp2(cpu, p);
                cpu.push_word(mem, value)?;
                Ok(16)
            } else if p == 0 {
                // CALL a16
                let target = cpu.fetch16(mem)?;
                let ret_addr = cpu.registers().pc;
                cpu.push_word(mem, ret_addr)?;
                cpu.registers_mut().pc = target;
                Ok(24)
            } else {
                Err(CpuError::UnsupportedOpcode(opcode))
            }
        }
        6 => {
            // ALU[y] A,d8
            let operand = cpu.fetch8(mem)?;
            apply_alu(cpu, y, operand);
            Ok(8)
        }
        7 => {
            // RST y*8
            let ret_addr = cpu.registers().pc;
            cpu.push_word(mem, ret_addr)?;
            cpu.registers_mut().pc = u16::from(y) * 8;
            Ok(16)
        }
        _ => Err(CpuError::UnsupportedOpcode(opcode)),
    }
}

fn execute_cb(cpu: &mut Cpu, mem: &mut dyn Memory, opcode: u8) -> Result<Cycles, CpuError> {
    let x = opcode >> 6;
    let y = (opcode >> 3) & 0x07;
    let z = opcode & 0x07;

    match x {
        0 => {
            // Rotates/shifts/swap.
            let value = read_r8(cpu, mem, z)?;
            let (result, carry) = match y {
                0 => (value.rotate_left(1), (value & 0x80) != 0), // RLC
                1 => (value.rotate_right(1), (value & 0x01) != 0), // RRC
                2 => {
                    let carry_in = u8::from(cpu.registers().flag(Flags::C));
                    (((value << 1) | carry_in), (value & 0x80) != 0) // RL
                }
                3 => {
                    let carry_in = if cpu.registers().flag(Flags::C) {
                        0x80
                    } else {
                        0x00
                    };
                    (((value >> 1) | carry_in), (value & 0x01) != 0) // RR
                }
                4 => (value << 1, (value & 0x80) != 0), // SLA
                5 => (((value >> 1) | (value & 0x80)), (value & 0x01) != 0), // SRA
                6 => (value.rotate_right(4), false),    // SWAP
                7 => (value >> 1, (value & 0x01) != 0), // SRL
                _ => return Err(CpuError::UnsupportedCbOpcode(opcode)),
            };

            write_r8(cpu, mem, z, result)?;
            cpu.registers_mut().set_flag(Flags::Z, result == 0);
            cpu.registers_mut().set_flag(Flags::N, false);
            cpu.registers_mut().set_flag(Flags::H, false);
            cpu.registers_mut().set_flag(Flags::C, carry);

            if z == 6 { Ok(16) } else { Ok(8) }
        }
        1 => {
            // BIT y,r[z]
            let value = read_r8(cpu, mem, z)?;
            let mask = 1u8 << y;
            let bit_set = (value & mask) != 0;
            let carry = cpu.registers().flag(Flags::C);

            cpu.registers_mut().set_flag(Flags::Z, !bit_set);
            cpu.registers_mut().set_flag(Flags::N, false);
            cpu.registers_mut().set_flag(Flags::H, true);
            cpu.registers_mut().set_flag(Flags::C, carry);

            if z == 6 { Ok(12) } else { Ok(8) }
        }
        2 => {
            // RES y,r[z]
            let value = read_r8(cpu, mem, z)?;
            let result = value & !(1u8 << y);
            write_r8(cpu, mem, z, result)?;
            if z == 6 { Ok(16) } else { Ok(8) }
        }
        3 => {
            // SET y,r[z]
            let value = read_r8(cpu, mem, z)?;
            let result = value | (1u8 << y);
            write_r8(cpu, mem, z, result)?;
            if z == 6 { Ok(16) } else { Ok(8) }
        }
        _ => Err(CpuError::UnsupportedCbOpcode(opcode)),
    }
}

fn read_r8(cpu: &Cpu, mem: &dyn Memory, index: u8) -> Result<u8, CpuError> {
    match index {
        0 => Ok(cpu.registers().b),
        1 => Ok(cpu.registers().c),
        2 => Ok(cpu.registers().d),
        3 => Ok(cpu.registers().e),
        4 => Ok(cpu.registers().h),
        5 => Ok(cpu.registers().l),
        6 => mem.read8(cpu.registers().hl()).map_err(CpuError::from),
        7 => Ok(cpu.registers().a),
        _ => Err(CpuError::UnsupportedOpcode(index)),
    }
}

fn write_r8(cpu: &mut Cpu, mem: &mut dyn Memory, index: u8, value: u8) -> Result<(), CpuError> {
    match index {
        0 => cpu.registers_mut().b = value,
        1 => cpu.registers_mut().c = value,
        2 => cpu.registers_mut().d = value,
        3 => cpu.registers_mut().e = value,
        4 => cpu.registers_mut().h = value,
        5 => cpu.registers_mut().l = value,
        6 => mem.write8(cpu.registers().hl(), value)?,
        7 => cpu.registers_mut().a = value,
        _ => return Err(CpuError::UnsupportedOpcode(index)),
    }
    Ok(())
}

#[must_use]
fn read_rp(cpu: &Cpu, index: u8) -> u16 {
    match index & 0x03 {
        0 => cpu.registers().bc(),
        1 => cpu.registers().de(),
        2 => cpu.registers().hl(),
        3 => cpu.registers().sp,
        _ => unreachable!("index masked to 0..=3"),
    }
}

fn set_rp(cpu: &mut Cpu, index: u8, value: u16) {
    match index {
        0 => cpu.registers_mut().set_bc(value),
        1 => cpu.registers_mut().set_de(value),
        2 => cpu.registers_mut().set_hl(value),
        3 => cpu.registers_mut().sp = value,
        _ => {}
    }
}

#[must_use]
fn read_rp2(cpu: &Cpu, index: u8) -> u16 {
    match index & 0x03 {
        0 => cpu.registers().bc(),
        1 => cpu.registers().de(),
        2 => cpu.registers().hl(),
        3 => cpu.registers().af(),
        _ => unreachable!("index masked to 0..=3"),
    }
}

fn set_rp2(cpu: &mut Cpu, index: u8, value: u16) {
    match index {
        0 => cpu.registers_mut().set_bc(value),
        1 => cpu.registers_mut().set_de(value),
        2 => cpu.registers_mut().set_hl(value),
        3 => cpu.registers_mut().set_af(value),
        _ => {}
    }
}

fn apply_alu(cpu: &mut Cpu, operation: u8, value: u8) {
    match operation {
        0 => add_a(cpu, value, false),       // ADD
        1 => add_a(cpu, value, true),        // ADC
        2 => sub_a(cpu, value, false, true), // SUB
        3 => sub_a(cpu, value, true, true),  // SBC
        4 => {
            // AND
            let result = cpu.registers().a & value;
            cpu.registers_mut().a = result;
            cpu.registers_mut().set_flag(Flags::Z, result == 0);
            cpu.registers_mut().set_flag(Flags::N, false);
            cpu.registers_mut().set_flag(Flags::H, true);
            cpu.registers_mut().set_flag(Flags::C, false);
        }
        5 => {
            // XOR
            let result = cpu.registers().a ^ value;
            cpu.registers_mut().a = result;
            cpu.registers_mut().set_flag(Flags::Z, result == 0);
            cpu.registers_mut().set_flag(Flags::N, false);
            cpu.registers_mut().set_flag(Flags::H, false);
            cpu.registers_mut().set_flag(Flags::C, false);
        }
        6 => {
            // OR
            let result = cpu.registers().a | value;
            cpu.registers_mut().a = result;
            cpu.registers_mut().set_flag(Flags::Z, result == 0);
            cpu.registers_mut().set_flag(Flags::N, false);
            cpu.registers_mut().set_flag(Flags::H, false);
            cpu.registers_mut().set_flag(Flags::C, false);
        }
        7 => {
            // CP
            sub_a(cpu, value, false, false);
        }
        _ => {}
    }
}

fn add_a(cpu: &mut Cpu, value: u8, include_carry: bool) {
    let a = cpu.registers().a;
    let carry_in = u8::from(include_carry && cpu.registers().flag(Flags::C));
    let result16 = u16::from(a) + u16::from(value) + u16::from(carry_in);
    let result = result16.to_le_bytes()[0];

    cpu.registers_mut().a = result;
    cpu.registers_mut().set_flag(Flags::Z, result == 0);
    cpu.registers_mut().set_flag(Flags::N, false);
    cpu.registers_mut()
        .set_flag(Flags::H, (a & 0x0F) + (value & 0x0F) + carry_in > 0x0F);
    cpu.registers_mut().set_flag(Flags::C, result16 > 0x00FF);
}

fn sub_a(cpu: &mut Cpu, value: u8, include_carry: bool, store_result: bool) {
    let a = cpu.registers().a;
    let carry_in = u8::from(include_carry && cpu.registers().flag(Flags::C));
    let subtrahend = u16::from(value) + u16::from(carry_in);
    let result16 = u16::from(a).wrapping_sub(subtrahend);
    let result = result16.to_le_bytes()[0];

    if store_result {
        cpu.registers_mut().a = result;
    }
    cpu.registers_mut().set_flag(Flags::Z, result == 0);
    cpu.registers_mut().set_flag(Flags::N, true);
    cpu.registers_mut().set_flag(
        Flags::H,
        u16::from(a & 0x0F) < (u16::from(value & 0x0F) + u16::from(carry_in)),
    );
    cpu.registers_mut()
        .set_flag(Flags::C, u16::from(a) < subtrahend);
}

fn add_hl(cpu: &mut Cpu, value: u16) {
    let hl = cpu.registers().hl();
    let result = hl.wrapping_add(value);
    let half_carry = (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF;
    let carry = u32::from(hl) + u32::from(value) > 0x0000_FFFF;

    cpu.registers_mut().set_hl(result);
    cpu.registers_mut().set_flag(Flags::N, false);
    cpu.registers_mut().set_flag(Flags::H, half_carry);
    cpu.registers_mut().set_flag(Flags::C, carry);
}

fn add_sp_signed(cpu: &mut Cpu, offset: i8) -> u16 {
    let sp = cpu.registers().sp;
    let offset_u8 = offset.to_ne_bytes()[0];
    let offset_u16 = if offset.is_negative() {
        0xFF00 | u16::from(offset_u8)
    } else {
        u16::from(offset_u8)
    };
    let result = sp.wrapping_add(offset_u16);

    let half_carry = (sp & 0x000F) + (u16::from(offset_u8) & 0x000F) > 0x000F;
    let carry = (sp & 0x00FF) + (u16::from(offset_u8) & 0x00FF) > 0x00FF;
    cpu.registers_mut().set_flag(Flags::Z, false);
    cpu.registers_mut().set_flag(Flags::N, false);
    cpu.registers_mut().set_flag(Flags::H, half_carry);
    cpu.registers_mut().set_flag(Flags::C, carry);

    result
}

fn daa(cpu: &mut Cpu) {
    let mut a = cpu.registers().a;
    let n = cpu.registers().flag(Flags::N);
    let h = cpu.registers().flag(Flags::H);
    let mut c = cpu.registers().flag(Flags::C);
    let mut adjust = 0u8;

    if n {
        if c {
            adjust |= 0x60;
        }
        if h {
            adjust |= 0x06;
        }
        a = a.wrapping_sub(adjust);
    } else {
        if c || a > 0x99 {
            adjust |= 0x60;
            c = true;
        }
        if h || (a & 0x0F) > 0x09 {
            adjust |= 0x06;
        }
        a = a.wrapping_add(adjust);
    }

    cpu.registers_mut().a = a;
    cpu.registers_mut().set_flag(Flags::Z, a == 0);
    cpu.registers_mut().set_flag(Flags::H, false);
    cpu.registers_mut().set_flag(Flags::C, c);
}

#[must_use]
fn cond_true(cpu: &Cpu, cond_index: u8) -> bool {
    match cond_index {
        0 => !cpu.registers().flag(Flags::Z), // NZ
        1 => cpu.registers().flag(Flags::Z),  // Z
        2 => !cpu.registers().flag(Flags::C), // NC
        3 => cpu.registers().flag(Flags::C),  // C
        _ => false,
    }
}

#[must_use]
fn is_invalid_base_opcode(opcode: u8) -> bool {
    matches!(
        opcode,
        0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD
    )
}
