//! Low-level bit encoders, grouped by ISA extension.
//!
//! Encoders retain their existing names, operand order, and bounds checks.

pub mod rv32a;
pub mod rv32c;
pub mod rv32d;
pub mod rv32f;
pub mod rv32i;
pub mod rv32m;
pub mod rv64a;
pub mod rv64c;
pub mod rv64d;
pub mod rv64f;
pub mod rv64i;
pub mod rv64m;
pub mod rvprivileged;
pub mod rvprotoinstrs;
pub mod zicsr;
pub mod zifencei;

pub use rv32a::*;
pub use rv32c::*;
pub use rv32d::*;
pub use rv32f::*;
pub use rv32i::*;
pub use rv32m::*;
pub use rv64a::*;
pub use rv64c::*;
pub use rv64d::*;
pub use rv64f::*;
pub use rv64i::*;
pub use rv64m::*;
pub use rvprivileged::*;
pub use zicsr::*;
pub use zifencei::*;
