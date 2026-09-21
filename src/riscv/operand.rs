//! Shared validation and sampling rules. Random generation uses the same bounds
//! as encoding, including compressed register subsets and aligned offsets.
use super::{
    fields::*,
    instruction::{EncodeError, Opcode},
};
use Opcode::*;
use rand::{Rng, RngExt};

pub(super) trait Operand: Copy {
    fn bits(self) -> u32;
    fn validate(
        self,
        _op: Opcode,
        _field: &'static str,
        _xlen: Option<Xlen>,
    ) -> Result<(), EncodeError> {
        Ok(())
    }
    fn sample(op: Opcode, field: &'static str, xlen: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self;
}

struct Bounds {
    min: i32,
    max: i32,
    alignment: i32,
    nonzero: bool,
    description: &'static str,
}
impl Bounds {
    const fn new(
        min: i32,
        max: i32,
        alignment: i32,
        nonzero: bool,
        description: &'static str,
    ) -> Self {
        Self {
            min,
            max,
            alignment,
            nonzero,
            description,
        }
    }
    fn validate(&self, op: Opcode, field: &'static str, value: i32) -> Result<(), EncodeError> {
        if (self.min..=self.max).contains(&value)
            && value % self.alignment == 0
            && (!self.nonzero || value != 0)
        {
            Ok(())
        } else {
            Err(EncodeError {
                opcode: op,
                field,
                value: value as i64,
                requirement: self.description,
            })
        }
    }
    fn sample(&self, rng: &mut (impl Rng + ?Sized)) -> i32 {
        loop {
            let value = rng.random_range(self.min / self.alignment..=self.max / self.alignment)
                * self.alignment;
            if !self.nonzero || value != 0 {
                return value;
            }
        }
    }
}
fn immediate_bounds(op: Opcode) -> Bounds {
    match op {
        Lui | Auipc => Bounds::new(
            -524288,
            1048575,
            1,
            false,
            "a signed or unsigned 20-bit upper field",
        ),
        Jal => Bounds::new(
            -1048576,
            1048574,
            2,
            false,
            "an even byte offset in -1048576..=1048574",
        ),
        Beq | Bne | Blt | Bge | Bltu | Bgeu => {
            Bounds::new(-4096, 4094, 2, false, "an even byte offset in -4096..=4094")
        }
        CJ | CJal => Bounds::new(-2048, 2046, 2, false, "an even byte offset in -2048..=2046"),
        CBeqz | CBnez => Bounds::new(-256, 254, 2, false, "an even byte offset in -256..=254"),
        CLui => Bounds::new(-32, 31, 1, true, "a nonzero signed 6-bit upper field"),
        CAddi | CLi | CAndi | CAddiw => Bounds::new(-32, 31, 1, false, "a signed 6-bit immediate"),
        CAddi16sp => Bounds::new(
            -512,
            496,
            16,
            true,
            "a nonzero multiple of 16 in -512..=496",
        ),
        CAddi4spn => Bounds::new(4, 1020, 4, true, "a multiple of 4 in 4..=1020"),
        CLwsp | CSwsp => Bounds::new(0, 252, 4, false, "a multiple of 4 in 0..=252"),
        CLw | CSw => Bounds::new(0, 124, 4, false, "a multiple of 4 in 0..=124"),
        CLdsp | CSdsp => Bounds::new(0, 504, 8, false, "a multiple of 8 in 0..=504"),
        CLd | CSd => Bounds::new(0, 248, 8, false, "a multiple of 8 in 0..=248"),
        _ => Bounds::new(-2048, 2047, 1, false, "a signed 12-bit immediate"),
    }
}
fn compact_register(op: Opcode) -> bool {
    matches!(
        op,
        CAnd | COr
            | CXor
            | CSub
            | CAddw
            | CSubw
            | CAddi4spn
            | CAndi
            | CSrli
            | CSrai
            | CBeqz
            | CBnez
            | CLw
            | CSw
            | CLd
            | CSd
    )
}
fn nonzero_register(op: Opcode) -> bool {
    matches!(
        op,
        CMv | CAdd | CLui | CSlli | CJr | CJalr | CAddiw | CLwsp | CLdsp
    )
}
impl Operand for XReg {
    fn bits(self) -> u32 {
        self.index() as u32
    }
    fn validate(self, op: Opcode, field: &'static str, _: Option<Xlen>) -> Result<(), EncodeError> {
        let index = self.index();
        let requirement = if compact_register(op) && !(8..=15).contains(&index) {
            Some("x8..=x15")
        } else if nonzero_register(op) && index == 0 {
            Some("a nonzero register")
        } else if op == CLui && index == 2 {
            Some("a register other than x2 (sp)")
        } else {
            None
        };
        match requirement {
            None => Ok(()),
            Some(requirement) => Err(EncodeError {
                opcode: op,
                field,
                value: index as i64,
                requirement,
            }),
        }
    }
    fn sample(op: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        if compact_register(op) {
            Self::new(rng.random_range(8..=15)).unwrap()
        } else {
            super::registers::random_int_register(rng)
        }
    }
}
impl Operand for FReg {
    fn bits(self) -> u32 {
        self.index() as u32
    }
    fn sample(_: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        super::registers::random_float_register(rng)
    }
}
impl Operand for Csr {
    fn bits(self) -> u32 {
        self.index() as u32
    }
    fn sample(_: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        Self::new(rng.random_range(0..=4095)).unwrap()
    }
}
impl Operand for i32 {
    fn bits(self) -> u32 {
        self as u32
    }
    fn validate(self, op: Opcode, field: &'static str, _: Option<Xlen>) -> Result<(), EncodeError> {
        immediate_bounds(op).validate(op, field, self)
    }
    fn sample(op: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        immediate_bounds(op).sample(rng)
    }
}
fn small_bounds(op: Opcode, field: &'static str, xlen: Option<Xlen>) -> Bounds {
    if field == "uimm" {
        return Bounds::new(0, 31, 1, false, "a 5-bit unsigned immediate");
    }
    let width = if matches!(op, Slliw | Srliw | Sraiw) {
        32
    } else {
        xlen.unwrap_or(Xlen::X64).bits() as i32
    };
    let compressed = matches!(op, CSlli | CSrli | CSrai);
    Bounds::new(
        if compressed { 1 } else { 0 },
        width - 1,
        1,
        false,
        match (width, compressed) {
            (32, true) => "a shift in 1..=31",
            (32, false) => "a shift in 0..=31",
            (_, true) => "a shift in 1..=63",
            (_, false) => "a shift in 0..=63",
        },
    )
}
impl Operand for u8 {
    fn bits(self) -> u32 {
        self as u32
    }
    fn validate(
        self,
        op: Opcode,
        field: &'static str,
        xlen: Option<Xlen>,
    ) -> Result<(), EncodeError> {
        small_bounds(op, field, xlen).validate(op, field, self as i32)
    }
    fn sample(op: Opcode, field: &'static str, xlen: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        small_bounds(op, field, Some(xlen)).sample(rng) as u8
    }
}
impl Operand for u16 {
    fn bits(self) -> u32 {
        self as u32
    }
    fn validate(self, op: Opcode, field: &'static str, _: Option<Xlen>) -> Result<(), EncodeError> {
        Bounds::new(0, 4095, 1, false, "a 12-bit FENCE immediate").validate(op, field, self as i32)
    }
    fn sample(_: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        rng.random_range(0..=255)
    }
}
impl Operand for RoundingMode {
    fn bits(self) -> u32 {
        self as u32
    }
    fn sample(_: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        Self::ALL[rng.random_range(0..Self::ALL.len())]
    }
}
impl Operand for Ordering {
    fn bits(self) -> u32 {
        self as u32
    }
    fn sample(_: Opcode, _: &'static str, _: Xlen, rng: &mut (impl Rng + ?Sized)) -> Self {
        Self::from_bits(rng.random(), rng.random())
    }
}
