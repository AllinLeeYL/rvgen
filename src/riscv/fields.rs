//! Architectural operands. Register numbers and CSR addresses are checked once
//! on construction; instruction-specific restrictions are checked on encoding.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperandError {
    pub field: &'static str,
    pub value: u32,
    pub max: u32,
}
impl fmt::Display for OperandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} must be in 0..={}, got {}",
            self.field, self.max, self.value
        )
    }
}
impl std::error::Error for OperandError {}

macro_rules! bounded_operand {
    ($name:ident, $storage:ty, $max:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($storage);
        impl $name {
            pub const fn new(value: $storage) -> Result<Self, OperandError> {
                if value <= $max {
                    Ok(Self(value))
                } else {
                    Err(OperandError {
                        field: stringify!($name),
                        value: value as u32,
                        max: $max,
                    })
                }
            }
            pub const fn index(self) -> $storage {
                self.0
            }
            pub(super) const fn constant(value: $storage) -> Self {
                match Self::new(value) {
                    Ok(operand) => operand,
                    Err(_) => panic!("invalid architectural constant"),
                }
            }
        }
        impl TryFrom<$storage> for $name {
            type Error = OperandError;
            fn try_from(value: $storage) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
        impl From<$name> for $storage {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}
bounded_operand!(XReg, u8, 31);
bounded_operand!(FReg, u8, 31);
bounded_operand!(Csr, u16, 4095);

impl fmt::Display for XReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "x{}", self.0)
    }
}
impl fmt::Display for FReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "f{}", self.0)
    }
}
impl Csr {
    /// Returns `true` for CSRs whose read value is implementation-dependent
    /// and may differ between RISC-V implementations. This includes:
    /// - Machine counter/timers: mcycle, minstret, mhpmcounter3–31 (0xB00–0xB1F)
    ///   and their RV32 high halves (0xB80–0xB9F)
    /// - User-visible counter shadows: cycle, time, instret, hpmcounter3–31
    ///   (0xC00–0xC1F) and their high halves (0xC80–0xC9F)
    /// - Read-only machine ID CSRs: mvendorid, marchid, mimpid, mhartid,
    ///   mconfigptr (0xF11–0xF15)
    pub const fn is_implementation_dependent(&self) -> bool {
        let addr = self.index();
        matches!(addr,
            0xB00..=0xB1F   // mcycle, minstret, mhpmcounter3–31
            | 0xB80..=0xB9F // mcycleh, minstreth, mhpmcounterh3–31
            | 0xC00..=0xC1F // cycle, time, instret, hpmcounter3–31
            | 0xC80..=0xC9F // cycleh, timeh, instreth, hpmcounterh3–31
            | 0xF11..=0xF15 // mvendorid, marchid, mimpid, mhartid, mconfigptr
        )
    }
}
impl fmt::Display for Csr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:03x}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xlen {
    X32,
    X64,
}
impl Xlen {
    pub const fn bits(self) -> u8 {
        match self {
            Self::X32 => 32,
            Self::X64 => 64,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoundingMode {
    Rne = 0,
    Rtz = 1,
    Rdn = 2,
    Rup = 3,
    Rmm = 4,
    #[default]
    Dyn = 7,
}
impl RoundingMode {
    pub const ALL: &'static [Self] = &[
        Self::Rne,
        Self::Rtz,
        Self::Rdn,
        Self::Rup,
        Self::Rmm,
        Self::Dyn,
    ];
}
impl TryFrom<u8> for RoundingMode {
    type Error = OperandError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL
            .iter()
            .copied()
            .find(|&rm| rm as u8 == value)
            .ok_or(OperandError {
                field: "rounding mode (5 and 6 are reserved)",
                value: value as u32,
                max: 7,
            })
    }
}

/// Atomic acquire/release bits; `AcqRel` sets both bits.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Ordering {
    #[default]
    Relaxed = 0,
    Release = 1,
    Acquire = 2,
    AcqRel = 3,
}
impl Ordering {
    pub const fn acquire(self) -> bool {
        (self as u8 & 2) != 0
    }
    pub const fn release(self) -> bool {
        (self as u8 & 1) != 0
    }
    pub const fn from_bits(aq: bool, rl: bool) -> Self {
        match (aq, rl) {
            (false, false) => Self::Relaxed,
            (false, true) => Self::Release,
            (true, false) => Self::Acquire,
            (true, true) => Self::AcqRel,
        }
    }
}

/// Common FENCE immediate fields. Custom I/O/R/W masks may also be passed
/// directly to `Instruction::Fence` (predecessor and successor use four bits each).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum FenceOrdering {
    RwRw = 0x033,
    Tso = 0x833,
    RwW = 0x031,
    RRw = 0x023,
    RR = 0x022,
    WW = 0x011,
}
impl FenceOrdering {
    pub const fn bits(self) -> u16 {
        self as u16
    }
}
