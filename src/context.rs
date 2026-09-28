use crate::options::OneOpts;
use crate::riscv::fields::Xlen;
use crate::riscv::instruction::{Extension, Opcode};
use crate::riscv::registers::PrivilegeLevel;

struct Target {
    xlen: Xlen,
    extensions: Vec<Extension>,
    privilege: PrivilegeLevel,
}

struct MemoryBlock {
    addr: usize,
    size: usize,
    flags: u64,
}

#[derive(Default)]
struct MemoryState {
    blocks: Vec<MemoryBlock>,
}

pub struct GenContext {
    target: Target,
    // pc: u64,
    // registers: RegisterState,
    memory: MemoryState,
    // privilege: PrivilegeLevel,
    pub disabled_instrs: Vec<Opcode>,
}

impl GenContext {
    pub fn new(opts: &OneOpts) -> Self {
        Self { 
            target: Target { 
                xlen: Xlen::X64, 
                extensions: opts.common.isa.clone(),
                privilege: PrivilegeLevel::Machine 
            },
            memory: MemoryState::default(),
            disabled_instrs: opts.common.disabled_instrs.clone(),
        }
    }

    pub fn is_disabled(&self, opcode: Opcode) -> bool {
        self.disabled_instrs.contains(&opcode)
    }
}
