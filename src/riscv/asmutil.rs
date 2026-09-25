//! Small assembly helpers with explicit XLEN behavior.
use super::{Instruction, XReg, Xlen};

/// Split a 32-bit bit pattern into a LUI upper field and signed 12-bit low
/// immediate. The low sign bit carries into the upper field. Recombination
/// uses wrapping 32-bit arithmetic.
pub const fn split_imm32(value: u32) -> (i32, i32) {
    let carry = (value >> 11) & 1;
    (
        ((value >> 12).wrapping_add(carry) & 0xfffff) as i32,
        (value & 0xfff) as i32 - (carry as i32 * 4096),
    )
}

/// Materialize a signed 32-bit value. RV64 uses ADDIW to obtain the correct
/// sign extension even when the LUI carry crosses bit 31 (e.g. 0x7fff_ffff).
/// This deliberately returns a predictable two-instruction sequence.
pub fn load_imm32(rd: XReg, value: i32, xlen: Xlen) -> [Instruction; 2] {
    let (upper, lower) = split_imm32(value as u32);
    [
        Instruction::Lui { rd, imm: upper },
        match xlen {
            Xlen::X32 => Instruction::Addi {
                rd,
                rs1: rd,
                imm: lower,
            },
            Xlen::X64 => Instruction::Addiw {
                rd,
                rs1: rd,
                imm: lower,
            },
        },
    ]
}

/// Interpret the low XLEN bits as a signed two's-complement number.
pub const fn twos_complement(value: u64, xlen: Xlen) -> i64 {
    match xlen {
        Xlen::X32 => value as u32 as i32 as i64,
        Xlen::X64 => value as i64,
    }
}
/// Return the XLEN-wide bit pattern, truncating to 32 bits for RV32.
pub const fn to_unsigned(value: i64, xlen: Xlen) -> u64 {
    match xlen {
        Xlen::X32 => value as u32 as u64,
        Xlen::X64 => value as u64,
    }
}
