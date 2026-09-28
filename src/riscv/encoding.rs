//! Shared bit packers. Operands are validated before entering these functions.
//! Signed immediates arrive as their two's-complement u32 bit patterns.
pub(super) fn r(base: u32, rd: u32, rs1: u32, rs2: u32) -> u32 {
    base | (rd << 7) | (rs1 << 15) | (rs2 << 20)
}
pub(super) fn r4(base: u32, rd: u32, rs1: u32, rs2: u32, rs3: u32) -> u32 {
    r(base, rd, rs1, rs2) | (rs3 << 27)
}
pub(super) fn i(base: u32, rd: u32, rs1: u32, imm: u32) -> u32 {
    base | (rd << 7) | (rs1 << 15) | ((imm & 0xfff) << 20)
}
pub(super) fn s(base: u32, rs1: u32, rs2: u32, imm: u32) -> u32 {
    r(base, 0, rs1, rs2) | ((imm & 31) << 7) | ((imm & 0xfe0) << 20)
}
pub(super) fn b(base: u32, rs1: u32, rs2: u32, imm: u32) -> u32 {
    r(base, 0, rs1, rs2)
        | (((imm >> 11) & 1) << 7)
        | ((imm & 0x1e) << 7)
        | ((imm & 0x7e0) << 20)
        | ((imm & 0x1000) << 19)
}
pub(super) fn u(base: u32, rd: u32, imm: u32) -> u32 {
    base | (rd << 7) | ((imm & 0xfffff) << 12)
}
pub(super) fn j(base: u32, rd: u32, imm: u32) -> u32 {
    base | (rd << 7)
        | (imm & 0xff000)
        | ((imm & 0x800) << 9)
        | ((imm & 0x7fe) << 20)
        | ((imm & 0x100000) << 11)
}

pub(super) fn cr(base: u32, rd: u32, rs2: u32) -> u32 {
    base | (rd << 7) | (rs2 << 2)
}
pub(super) fn ca(base: u32, rd: u32, rs2: u32) -> u32 {
    cr(base, rd - 8, rs2 - 8)
}
pub(super) fn ci(base: u32, rd: u32, imm: u32) -> u32 {
    base | (rd << 7) | ((imm & 31) << 2) | ((imm & 32) << 7)
}
pub(super) fn cb_alu(base: u32, rs1: u32, imm: u32) -> u32 {
    ci(base, rs1 - 8, imm)
}
pub(super) fn css(base: u32, rs2: u32, imm: u32) -> u32 {
    base | (rs2 << 2) | (imm << 7)
}
pub(super) fn cl(base: u32, rd: u32, rs1: u32, imm: u32) -> u32 {
    base | ((rd - 8) << 2) | ((rs1 - 8) << 7) | ((imm & 3) << 5) | ((imm & 28) << 8)
}
pub(super) fn ciw(rd: u32, imm: u32) -> u32 {
    ((rd - 8) << 2)
        | ((imm & 0x30) << 7)
        | ((imm & 0x3c0) << 1)
        | ((imm & 4) << 4)
        | ((imm & 8) << 2)
}
pub(super) fn cb(base: u32, rs1: u32, imm: u32) -> u32 {
    base | ((rs1 - 8) << 7)
        | ((imm & 0x100) << 4)
        | ((imm & 0x18) << 7)
        | ((imm & 0xc0) >> 1)
        | ((imm & 6) << 2)
        | ((imm & 0x20) >> 3)
}
pub(super) fn cj(base: u32, imm: u32) -> u32 {
    base | ((imm & 0x800) << 1)
        | ((imm & 0x10) << 7)
        | ((imm & 0x300) << 1)
        | ((imm & 0x400) >> 2)
        | ((imm & 0x40) << 1)
        | ((imm & 0x80) >> 1)
        | ((imm & 0xe) << 2)
        | ((imm & 0x20) >> 3)
}
pub(super) fn addi16sp_imm(imm: u32) -> u32 {
    ((imm >> 4) & 32) | (imm & 16) | ((imm >> 3) & 8) | ((imm >> 6) & 6) | ((imm >> 5) & 1)
}
pub(super) fn lwsp_imm(imm: u32) -> u32 {
    (imm & 0x3c) | ((imm >> 6) & 3)
}
pub(super) fn ldsp_imm(imm: u32) -> u32 {
    (imm & 0x38) | ((imm >> 6) & 7)
}
pub(super) fn swsp_imm(imm: u32) -> u32 {
    (imm & 0x3c) | ((imm >> 6) & 3)
}
pub(super) fn sdsp_imm(imm: u32) -> u32 {
    (imm & 0x38) | ((imm >> 6) & 7)
}
pub(super) fn word_imm(imm: u32) -> u32 {
    ((imm >> 1) & 28) | ((imm >> 1) & 2) | ((imm >> 6) & 1)
}
pub(super) fn double_imm(imm: u32) -> u32 {
    ((imm >> 1) & 28) | ((imm >> 6) & 3)
}
