#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Base(u8),
    PrefixCb,
}

#[must_use]
pub fn decode(opcode: u8) -> Instruction {
    if opcode == 0xCB {
        Instruction::PrefixCb
    } else {
        Instruction::Base(opcode)
    }
}
