//! Build an ELF with explicitly encoded instructions instead of random selection.

use rvgen::{
    riscv::{rv32i_addi, rv32m_mul},
    utils::elfbuilder::ElfBuilder,
};

fn main() -> rvgen::Result<()> {
    let words = [
        rv32i_addi(10, 0, 6),
        rv32i_addi(11, 0, 7),
        rv32m_mul(12, 10, 11),
    ];
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let elf = rvgen::runtime::executable(&bytes, true, 0x8000_0000)?;
    let output = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "encoded.elf".into());
    ElfBuilder.save(&elf, output)
}
