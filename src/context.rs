use crate::options::OneOpts;
use crate::riscv::fields::Xlen;
use crate::riscv::instruction::{Extension, Opcode};
use crate::riscv::registers::PrivilegeLevel;

struct Target {
    xlen: Xlen,
    extensions: Vec<Extension>,
    privilege: PrivilegeLevel,
}

pub struct GenContext {
    target: Target,
    // pc: u64,
    // registers: RegisterState,
    // memory: MemoryState,
    // privilege: PrivilegeLevel,
    pub disabled_instrs: Vec<Opcode>,
}

impl GenContext {
    pub fn new(opts: &OneOpts) -> Self {
        let disabled_instrs = opts
            .common
            .disabled_instrs
            .iter()
            .filter_map(|s| s.parse::<Opcode>().ok())
            .collect();
        Self { disabled_instrs }
    }

    pub fn is_disabled(&self, opcode: Opcode) -> bool {
        self.disabled_instrs.contains(&opcode)
    }
}
