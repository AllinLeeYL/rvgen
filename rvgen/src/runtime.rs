//! Bare-metal entry, trap handling, and Spike HTIF termination.
//!
//! HTIF discovers the `tohost` and `fromhost` ELF symbols and treats an odd
//! value written to `tohost` as (exit_status << 1) | 1.

use crate::{
    riscv::*,
    utils::elfbuilder::{ElfBuilder, ElfSection, ElfSymbol, SHF_ALLOC, SHF_WRITE},
    Result,
};

fn pc_relative(delta: i64) -> Result<[u32; 2]> {
    let hi = (delta + 0x800) >> 12;
    if !(-0x80000..=0x7ffff).contains(&hi) {
        return Err("runtime target exceeds AUIPC range".into());
    }
    Ok([
        rv32i_auipc(5, hi as i32),
        rv32i_addi(5, 5, (delta - (hi << 12)) as i32),
    ])
}

fn append(bytes: &mut Vec<u8>, words: &[u32]) {
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
}

pub fn executable(body: &[u8], is_64bit: bool, start_addr: u64) -> Result<Vec<u8>> {
    if start_addr & 3 != 0 || body.len() & 1 != 0 {
        return Err(
            "entry must be four-byte aligned and body must contain whole instructions".into(),
        );
    }
    // Install a direct-mode trap handler before entering the generated body.
    let mut text = vec![0; 8];
    append(&mut text, &[zicsr_csrrw(0, 5, CSR::MTVEC)]);
    text.extend_from_slice(body);
    if text.len() & 3 != 0 {
        text.extend_from_slice(&0x0001_u16.to_le_bytes());
    } // c.nop
    append(&mut text, &[rv32i_addi(31, 0, 1)]); // HTIF pass
    let writer_offset = text.len();
    text.extend_from_slice(&[0; 8]); // PC-relative address of tohost
    if !is_64bit {
        append(&mut text, &[rv32i_sw(5, 0, 4)]);
    } // upper mailbox word
    append(
        &mut text,
        &[
            rv32i_fence(0x33),
            if is_64bit {
                rv64i_sd(5, 31, 0)
            } else {
                rv32i_sw(5, 31, 0)
            },
            rv32i_fence(0x33),
            rv32i_jal(0, 0), // park until the host consumes the exit request
        ],
    );
    let trap_offset = text.len();
    append(
        &mut text,
        &[
            rv32i_addi(31, 0, 3), // HTIF fail: exit status 1
            rv32i_jal(0, (writer_offset as i64 - trap_offset as i64 - 4) as i32),
        ],
    );
    let text_end = start_addr
        .checked_add(text.len() as u64)
        .ok_or("runtime address overflow")?;
    let tohost_addr = text_end.checked_add(63).ok_or("runtime address overflow")? & !63;
    let fromhost_addr = tohost_addr
        .checked_add(64)
        .ok_or("runtime address overflow")?;
    let entry_pair = pc_relative(i64::try_from(trap_offset)?)?;
    let writer_pair = pc_relative(i64::try_from(tohost_addr - start_addr)? - writer_offset as i64)?;
    for (offset, words) in [(0, entry_pair), (writer_offset, writer_pair)] {
        for (i, word) in words.iter().enumerate() {
            text[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
    let text_size = text.len() as u64;
    let mut text_section = ElfSection::new(".text", text);
    text_section.addr = start_addr;
    let mut tohost = ElfSection::new(".tohost", vec![0; 8]);
    tohost.addr = tohost_addr;
    tohost.flags = SHF_ALLOC | SHF_WRITE;
    tohost.align = 64;
    let mut fromhost = tohost.clone();
    fromhost.name = ".fromhost".into();
    fromhost.addr = fromhost_addr;
    let symbols = [
        ElfSymbol {
            name: "_start".into(),
            section: 0,
            offset: 0,
            size: text_size,
            symbol_type: 2,
        },
        ElfSymbol {
            name: "rvgen_trap".into(),
            section: 0,
            offset: trap_offset as u64,
            size: 8,
            symbol_type: 2,
        },
        ElfSymbol {
            name: "tohost".into(),
            section: 1,
            offset: 0,
            size: 8,
            symbol_type: 1,
        },
        ElfSymbol {
            name: "fromhost".into(),
            section: 2,
            offset: 0,
            size: 8,
            symbol_type: 1,
        },
    ];
    ElfBuilder.build_with_symbols(
        &[text_section, tohost, fromhost],
        &symbols,
        is_64bit,
        start_addr,
    )
}
