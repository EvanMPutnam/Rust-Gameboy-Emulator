#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Breakpoints {
    pcs: Vec<u16>,
    opcodes: Vec<u8>,
}

impl Breakpoints {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_pc(&mut self, pc: u16) {
        if !self.pcs.contains(&pc) {
            self.pcs.push(pc);
        }
    }

    pub fn add_opcode(&mut self, opcode: u8) {
        if !self.opcodes.contains(&opcode) {
            self.opcodes.push(opcode);
        }
    }

    pub fn clear(&mut self) {
        self.pcs.clear();
        self.opcodes.clear();
    }

    #[must_use]
    pub fn is_pc_match(&self, pc: u16) -> bool {
        self.pcs.contains(&pc)
    }

    #[must_use]
    pub fn is_opcode_match(&self, opcode: u8) -> bool {
        self.opcodes.contains(&opcode)
    }
}
