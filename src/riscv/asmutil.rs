//! Small assembly helpers with explicit XLEN behavior.
use super::{Csr, Instruction, XReg, Xlen};

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

/// Materialize an XLEN-wide value. Values that fit in 32 bits use
/// [`load_imm32`]; wider RV64 values shift in the remaining 12-bit chunks.
pub fn load_imm(rd: XReg, value: i64, xlen: Xlen) -> Vec<Instruction> {
    if xlen == Xlen::X32 || value == value as i32 as i64 {
        return load_imm32(rd, value as i32, xlen).to_vec();
    }
    let lower = (value << 52) >> 52;
    let mut seq = load_imm(rd, value.wrapping_sub(lower) >> 12, xlen);
    seq.push(Instruction::Slli {
        rd,
        rs1: rd,
        shamt: 12,
    });
    seq.push(Instruction::Addi {
        rd,
        rs1: rd,
        imm: lower as i32,
    });
    seq
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

/// Extract the destination register and CSR address from a CSR instruction.
/// Returns `None` for non-CSR instructions.
pub fn csr_rd_and_addr(instr: &Instruction) -> Option<(XReg, Csr)> {
    match *instr {
        Instruction::Csrrw { rd, csr, .. }
        | Instruction::Csrrs { rd, csr, .. }
        | Instruction::Csrrc { rd, csr, .. }
        | Instruction::Csrrwi { rd, csr, .. }
        | Instruction::Csrrsi { rd, csr, .. }
        | Instruction::Csrrci { rd, csr, .. } => Some((rd, csr)),
        _ => None,
    }
}

/// Replace the CSR a CSR instruction accesses. Other instructions are
/// returned unchanged.
pub fn with_csr(instr: Instruction, csr: Csr) -> Instruction {
    match instr {
        Instruction::Csrrw { rd, rs1, .. } => Instruction::Csrrw { rd, rs1, csr },
        Instruction::Csrrs { rd, rs1, .. } => Instruction::Csrrs { rd, rs1, csr },
        Instruction::Csrrc { rd, rs1, .. } => Instruction::Csrrc { rd, rs1, csr },
        Instruction::Csrrwi { rd, uimm, .. } => Instruction::Csrrwi { rd, uimm, csr },
        Instruction::Csrrsi { rd, uimm, .. } => Instruction::Csrrsi { rd, uimm, csr },
        Instruction::Csrrci { rd, uimm, .. } => Instruction::Csrrci { rd, uimm, csr },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(seq: &[Instruction], xlen: Xlen) -> u64 {
        let mut value = 0i64;
        for instr in seq {
            value = match *instr {
                Instruction::Lui { imm, .. } => ((imm << 12) as i32).into(),
                Instruction::Addi { imm, .. } => value.wrapping_add(imm.into()),
                Instruction::Addiw { imm, .. } => (value as i32).wrapping_add(imm).into(),
                Instruction::Slli { shamt, .. } => value << shamt,
                _ => unreachable!(),
            };
        }
        to_unsigned(value, xlen)
    }

    #[test]
    fn load_imm_materializes_value() {
        for value in [0, 0x7ff, 0x8000_0000, 0x1_2345_6789, -1, i64::MAX, i64::MIN] {
            let seq = load_imm(XReg::A0, value, Xlen::X64);
            assert_eq!(eval(&seq, Xlen::X64), value as u64, "{value:#x}");
        }
        let seq = load_imm(XReg::A0, 0x8000_0000, Xlen::X32);
        assert_eq!(eval(&seq, Xlen::X32), 0x8000_0000);
    }
}
