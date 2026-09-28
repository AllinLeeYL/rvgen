//! Machine-mode code emission. Every instruction is checked against Target.
use anyhow::{Result, anyhow, ensure};

use super::memory::MemoryLayout;
use super::{Runtime, SCRATCH_BASE_REGISTER, SMC_BASE_REGISTER, Target};
use crate::riscv::{Csr, Extension, FReg, Instruction, RoundingMode, XReg, Xlen};
use crate::utils::ElfSymbol;

const ADDRESS_LOAD_SIZE: usize = 8;

/// Address references stay symbolic until the complete text size is known.
enum AddressTarget {
    TextOffset(usize),
    Section(&'static str, u64),
}

struct AddressFixup {
    offset: usize,
    register: XReg,
    target: AddressTarget,
}

pub(super) struct RuntimeCode {
    pub bytes: Vec<u8>,
    exit_offset: usize,
    trap_offset: usize,
    fixups: Vec<AddressFixup>,
}

impl RuntimeCode {
    pub fn assemble(body: &[u8], runtime: &Runtime) -> Result<Self> {
        let target = runtime.target();
        let mut bytes = Vec::new();
        let mut fixups = Vec::new();
        emit_startup(&mut bytes, &mut fixups, runtime)?;
        bytes.extend_from_slice(body);
        if bytes.len() % 4 != 0 {
            target.emit(Instruction::cnop(), &mut bytes)?;
        }
        let (writer_offset, exit_offset) = emit_exit(&mut bytes, target)?;
        let trap_offset = emit_trap_handler(&mut bytes, writer_offset, target)?;
        fixups.extend([
            AddressFixup {
                offset: 0,
                register: XReg::T0,
                target: AddressTarget::TextOffset(trap_offset),
            },
            AddressFixup {
                offset: writer_offset,
                register: XReg::T0,
                target: AddressTarget::Section(".tohost", 0),
            },
        ]);
        Ok(Self {
            bytes,
            exit_offset,
            trap_offset,
            fixups,
        })
    }

    pub fn resolve_addresses(&mut self, layout: &MemoryLayout) -> Result<()> {
        let text_addr = layout.get(".text")?.address;
        for fixup in &self.fixups {
            let destination = match fixup.target {
                AddressTarget::TextOffset(offset) => text_addr.checked_add(offset as u64),
                AddressTarget::Section(name, offset) => {
                    layout.get(name)?.address.checked_add(offset)
                }
            }
            .ok_or_else(|| anyhow!("runtime address overflow"))?;
            let pc = text_addr
                .checked_add(fixup.offset as u64)
                .ok_or_else(|| anyhow!("runtime address overflow"))?;
            let pair = address_load(fixup.register, i128::from(destination) - i128::from(pc))?;
            for (index, instruction) in pair.into_iter().enumerate() {
                let start = fixup.offset + index * 4;
                self.bytes[start..start + 4]
                    .copy_from_slice(&instruction.encode()?.bits().to_le_bytes());
            }
        }
        Ok(())
    }

    pub fn symbols(&self, layout: &MemoryLayout) -> Result<Vec<ElfSymbol>> {
        let text = layout.index(".text")?;
        let mut symbols = vec![
            ElfSymbol {
                name: "_start".into(),
                section: text,
                offset: 0,
                size: self.bytes.len() as u64,
                symbol_type: 2,
            },
            ElfSymbol {
                name: "rvgen_trap".into(),
                section: text,
                offset: self.trap_offset as u64,
                size: 8,
                symbol_type: 2,
            },
            ElfSymbol {
                name: "_exit".into(),
                section: text,
                offset: self.exit_offset as u64,
                size: 0,
                symbol_type: 2,
            },
        ];
        for (name, symbol) in [
            (".tohost", "tohost"),
            (".fromhost", "fromhost"),
            (".scratch", "rvgen_scratch"),
        ] {
            symbols.push(ElfSymbol {
                name: symbol.into(),
                section: layout.index(name)?,
                offset: 0,
                size: layout.get(name)?.request.size as u64,
                symbol_type: 1,
            });
        }
        if let Ok(section) = layout.index(".smc") {
            symbols.push(ElfSymbol {
                name: "rvgen_smc".into(),
                section,
                offset: 0,
                size: layout.get(".smc")?.request.size as u64,
                symbol_type: 1,
            });
        }
        Ok(symbols)
    }
}

fn reserve_address_load(
    text: &mut Vec<u8>,
    fixups: &mut Vec<AddressFixup>,
    register: XReg,
    target: AddressTarget,
) {
    fixups.push(AddressFixup {
        offset: text.len(),
        register,
        target,
    });
    text.extend_from_slice(&[0; ADDRESS_LOAD_SIZE]);
}

fn emit_startup(
    text: &mut Vec<u8>,
    fixups: &mut Vec<AddressFixup>,
    runtime: &Runtime,
) -> Result<()> {
    let target = runtime.target();
    text.extend_from_slice(&[0; ADDRESS_LOAD_SIZE]); // mtvec's handler offset is resolved later.
    target.emit(
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::T0,
            csr: Csr::MTVEC,
        },
        text,
    )?;
    target.emit(
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::ZERO,
            csr: Csr::SSCRATCH,
        },
        text,
    )?;
    target.emit(
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::ZERO,
            csr: Csr::MSCRATCH,
        },
        text,
    )?;
    if target.has(Extension::F) {
        emit_fpu_setup(text, target)?;
    }
    for index in 1..32 {
        target.emit(
            Instruction::Addi {
                rd: XReg::new(index)?,
                rs1: XReg::ZERO,
                imm: 0,
            },
            text,
        )?;
    }
    reserve_address_load(
        text,
        fixups,
        SCRATCH_BASE_REGISTER,
        AddressTarget::Section(".scratch", runtime.scratch_window().base_offset()),
    );
    if runtime.memory().request(".smc").is_some() {
        reserve_address_load(
            text,
            fixups,
            SMC_BASE_REGISTER,
            AddressTarget::Section(".smc", 0),
        );
    }
    Ok(())
}

fn emit_fpu_setup(text: &mut Vec<u8>, target: &Target) -> Result<()> {
    // Enable FS before touching FP state, then select RNE and clear flags.
    for instruction in [
        Instruction::Lui {
            rd: XReg::T0,
            imm: 0x6,
        },
        Instruction::Csrrs {
            rd: XReg::ZERO,
            rs1: XReg::T0,
            csr: Csr::MSTATUS,
        },
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::ZERO,
            csr: Csr::FCSR,
        },
    ] {
        target.emit(instruction, text)?;
    }
    for index in 0..32 {
        let rd = FReg::new(index)?;
        // RV32 cannot use FMV.D.X. Integer-to-double conversion initializes the
        // whole D register to +0.0 on RV32, avoiding single-precision NaN boxing.
        let instruction = match (target.has(Extension::D), target.xlen()) {
            (true, Xlen::X64) => Instruction::FmvDX {
                rd,
                rs1: XReg::ZERO,
            },
            (true, Xlen::X32) => Instruction::FcvtDW {
                rd,
                rs1: XReg::ZERO,
                rm: RoundingMode::Rne,
            },
            (false, _) => Instruction::FmvWX {
                rd,
                rs1: XReg::ZERO,
            },
        };
        target.emit(instruction, text)?;
    }
    Ok(())
}

fn emit_exit(text: &mut Vec<u8>, target: &Target) -> Result<(usize, usize)> {
    // Workload fallthrough passes; traps enter the writer with x31 = 3.
    target.emit(
        Instruction::Addi {
            rd: XReg::X31,
            rs1: XReg::ZERO,
            imm: 1,
        },
        text,
    )?;
    let writer_offset = text.len();
    text.extend_from_slice(&[0; ADDRESS_LOAD_SIZE]);
    if target.xlen() == Xlen::X32 {
        target.emit(
            Instruction::Sw {
                rs1: XReg::T0,
                rs2: XReg::ZERO,
                imm: 4,
            },
            text,
        )?;
    }
    let store = match target.xlen() {
        Xlen::X64 => Instruction::Sd {
            rs1: XReg::T0,
            rs2: XReg::X31,
            imm: 0,
        },
        Xlen::X32 => Instruction::Sw {
            rs1: XReg::T0,
            rs2: XReg::X31,
            imm: 0,
        },
    };
    for instruction in [
        Instruction::Fence { imm: 0x33 },
        store,
        Instruction::Fence { imm: 0x33 },
    ] {
        target.emit(instruction, text)?;
    }
    let exit_offset = text.len();
    target.emit(
        Instruction::Jal {
            rd: XReg::ZERO,
            imm: 0,
        },
        text,
    )?;
    Ok((writer_offset, exit_offset))
}

fn emit_trap_handler(text: &mut Vec<u8>, writer_offset: usize, target: &Target) -> Result<usize> {
    let trap_offset = text.len();
    target.emit(
        Instruction::Addi {
            rd: XReg::X31,
            rs1: XReg::ZERO,
            imm: 3,
        },
        text,
    )?;
    let delta = i32::try_from(writer_offset as i128 - text.len() as i128)?;
    target.emit(
        Instruction::Jal {
            rd: XReg::ZERO,
            imm: delta,
        },
        text,
    )?;
    Ok(trap_offset)
}

fn address_load(register: XReg, delta: i128) -> Result<[Instruction; 2]> {
    let hi = (delta + 0x800) >> 12;
    ensure!(
        (-0x80000..=0x7ffff).contains(&hi),
        "runtime target exceeds AUIPC range"
    );
    Ok([
        Instruction::Auipc {
            rd: register,
            imm: hi as i32,
        },
        Instruction::Addi {
            rd: register,
            rs1: register,
            imm: (delta - (hi << 12)) as i32,
        },
    ])
}
