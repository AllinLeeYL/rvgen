//! Bare-metal entry, trap handling, and Spike HTIF termination.
//!
//! HTIF discovers the `tohost` and `fromhost` ELF symbols and treats an odd
//! value written to `tohost` as `(exit_status << 1) | 1`. Normal completion
//! writes 1 to `tohost`; traps write 3 (failure).

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::riscv::{Csr, Instruction, XReg};
use crate::utils::{ElfSection, ElfSymbol, SHF_ALLOC, SHF_WRITE, build_elf_with_symbols};

pub fn pc_relative(delta: i64) -> Result<[u32; 2]> {
    let hi = (delta + 0x800) >> 12;
    if !(-0x80000..=0x7ffff).contains(&hi) {
        bail!("runtime target exceeds AUIPC range");
    }
    let lo = (delta - (hi << 12)) as i32;
    let auipc = Instruction::Auipc {
        rd: XReg::X5,
        imm: hi as i32,
    }
    .encode()?
    .bits();
    let addi = Instruction::Addi {
        rd: XReg::X5,
        rs1: XReg::X5,
        imm: lo,
    }
    .encode()?
    .bits();
    Ok([auipc, addi])
}

fn append_words(dst: &mut Vec<u8>, words: &[u32]) {
    for &w in words {
        dst.extend_from_slice(&w.to_le_bytes());
    }
}

/// Wraps a workload body in a machine-mode trap handler and HTIF exit runtime,
/// producing a complete ELF executable. Initializes integer registers and FPU
/// control state before entering the body; requires the F extension.
fn executable(body: &[u8], is_64bit: bool, start_addr: u64) -> Result<Vec<u8>> {
    if start_addr & 3 != 0 || body.len() & 1 != 0 {
        bail!("entry must be four-byte aligned and body must contain whole instructions");
    }

    // Reserve 8 bytes at the start for AUIPC/ADDI that point to the trap handler
    let mut text = vec![0u8; 8];
    let csrrw = Instruction::Csrrw {
        rd: XReg::ZERO,
        rs1: XReg::X5,
        csr: Csr::MTVEC,
    }
    .encode()?
    .bits();
    append_words(&mut text, &[csrrw]);

    // Enable the FPU before accessing fcsr. Set both mstatus.FS bits (Dirty)
    // so the final state does not depend on the boot code's previous FS value.
    // CSRRS preserves the other mstatus fields.
    for instr in [
        Instruction::Lui {
            rd: XReg::X5,
            imm: 0x6, // 0x6000 = 0b11 << 13 (mstatus.FS)
        },
        Instruction::Csrrs {
            rd: XReg::ZERO,
            rs1: XReg::X5,
            csr: Csr::MSTATUS,
        },
        // frm = RNE (round to nearest, ties to even), fflags = 0.
        Instruction::Csrrw {
            rd: XReg::ZERO,
            rs1: XReg::ZERO,
            csr: Csr::FCSR,
        },
    ] {
        instr.append_bytes(&mut text)?;
    }

    // x0 is hardwired to zero. Clear x1-x31 after all setup so scratch
    // registers (including x5) cannot leak boot/runtime state into the body.
    for index in 1..32 {
        Instruction::Addi {
            rd: XReg::new(index)?,
            rs1: XReg::ZERO,
            imm: 0,
        }
        .append_bytes(&mut text)?;
    }

    // Append workload body
    text.extend_from_slice(body);

    // Pad with c.nop if 2-byte aligned
    if text.len() & 3 != 0 {
        text.extend_from_slice(&[1, 0]); // c.nop
    }

    // Normal exit: x31 = 1 (HTIF success)
    let addi_pass = Instruction::Addi {
        rd: XReg::X31,
        rs1: XReg::ZERO,
        imm: 1,
    }
    .encode()?
    .bits();
    append_words(&mut text, &[addi_pass]);

    let writer_offset = text.len();
    // Reserve 8 bytes for AUIPC/ADDI that calculate tohost address into x5
    text.extend_from_slice(&[0u8; 8]);

    let store = if is_64bit {
        Instruction::Sd {
            rs1: XReg::X5,
            rs2: XReg::X31,
            imm: 0,
        }
        .encode()?
        .bits()
    } else {
        Instruction::Sw {
            rs1: XReg::X5,
            rs2: XReg::X31,
            imm: 0,
        }
        .encode()?
        .bits()
    };

    let fence = Instruction::Fence { imm: 0x33 }.encode()?.bits();
    let jal_spin = Instruction::Jal {
        rd: XReg::ZERO,
        imm: 0,
    }
    .encode()?
    .bits();

    if !is_64bit {
        let sw_upper = Instruction::Sw {
            rs1: XReg::X5,
            rs2: XReg::ZERO,
            imm: 4,
        }
        .encode()?
        .bits();
        append_words(&mut text, &[sw_upper]);
    }

    append_words(&mut text, &[fence, store, fence, jal_spin]);

    // Trap handler: x31 = 3 (HTIF failure)
    let trap_offset = text.len();
    let addi_fail = Instruction::Addi {
        rd: XReg::X31,
        rs1: XReg::ZERO,
        imm: 3,
    }
    .encode()?
    .bits();
    let jal_to_writer = Instruction::Jal {
        rd: XReg::ZERO,
        imm: (writer_offset as i64 - trap_offset as i64 - 4) as i32,
    }
    .encode()?
    .bits();
    append_words(&mut text, &[addi_fail, jal_to_writer]);

    let text_end = start_addr
        .checked_add(text.len() as u64)
        .ok_or_else(|| anyhow::anyhow!("runtime address overflow"))?;
    let tohost_addr = (text_end
        .checked_add(63)
        .ok_or_else(|| anyhow::anyhow!("runtime address overflow"))?)
        & !63;
    let fromhost_addr = tohost_addr
        .checked_add(64)
        .ok_or_else(|| anyhow::anyhow!("runtime address overflow"))?;

    let entry_pair = pc_relative(trap_offset as i64)?;
    let writer_pair = pc_relative((tohost_addr - start_addr) as i64 - writer_offset as i64)?;

    for (i, &w) in entry_pair.iter().enumerate() {
        text[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    for (i, &w) in writer_pair.iter().enumerate() {
        text[writer_offset + i * 4..writer_offset + i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }

    let text_len = text.len() as u64;
    let mut code_section = ElfSection::new(".text", text);
    code_section.addr = start_addr;

    let mut tohost_section = ElfSection::new(".tohost", vec![0u8; 8]);
    tohost_section.addr = tohost_addr;
    tohost_section.flags = SHF_ALLOC | SHF_WRITE;
    tohost_section.align = 64;

    let mut fromhost_section = tohost_section.clone();
    fromhost_section.name = ".fromhost".to_string();
    fromhost_section.addr = fromhost_addr;

    let symbols = [
        ElfSymbol {
            name: "_start".to_string(),
            section: 0,
            offset: 0,
            size: text_len,
            symbol_type: 2, // STT_FUNC
        },
        ElfSymbol {
            name: "rvgen_trap".to_string(),
            section: 0,
            offset: trap_offset as u64,
            size: 8,
            symbol_type: 2, // STT_FUNC
        },
        ElfSymbol {
            name: "_exit".to_string(),
            section: 0,
            offset: (trap_offset - 4) as u64,
            size: 0,
            symbol_type: 2, // STT_FUNC
        },
        ElfSymbol {
            name: "tohost".to_string(),
            section: 1,
            offset: 0,
            size: 8,
            symbol_type: 1, // STT_OBJECT
        },
        ElfSymbol {
            name: "fromhost".to_string(),
            section: 2,
            offset: 0,
            size: 8,
            symbol_type: 1, // STT_OBJECT
        },
    ];

    build_elf_with_symbols(
        &[code_section, tohost_section, fromhost_section],
        &symbols,
        is_64bit,
        start_addr,
    )
}

/// Convenience function to generate an executable ELF and write it directly to a file.
pub fn write_executable(
    body: &[u8],
    is_64bit: bool,
    start_addr: u64,
    path: impl AsRef<Path>,
) -> Result<()> {
    let elf_data = executable(body, is_64bit, start_addr)?;
    let p = path.as_ref();
    std::fs::write(p, elf_data).with_context(|| format!("failed to write ELF to {}", p.display()))
}
