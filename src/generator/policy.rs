//! Weighted workload policy, independent of instruction encoding.

use rand::distributions::WeightedIndex;

use crate::{InstructionClass, Result};

/// High-level generator behavior. Actions lower to zero or more instructions.
/// Platform/state-dependent actions without a lowering implementation return an
/// error when enabled; they are never silently replaced with unrelated opcodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenerationAction {
    /// Materialize a random signed 32-bit value in an integer register.
    RegFsm,
    FpuFsm,
    Exception,
    DescendPrivilege,
    WaitForInterruptTrap,
    WaitForInterruptNoTrap,
    SendIpi,
    SendLocalInterrupt,
    ClearInterrupt,
    EpcFsm,
    TvecFsm,
    MachineCsr,
    /// Release generator register-pollution bookkeeping. Currently a no-op:
    /// the stateless operand generator does not reserve polluted registers.
    FreePolluted,
    /// Copy a register, then use the copy as a load address.
    CreateAddressDependency,
    ClearMdt,
    ResetMcm,
    WriteInstruction,
    WaitForInstruction,
}

/// Either an ordinary instruction-family selection or a generator behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenerationChoice {
    Instruction(InstructionClass),
    Action(GenerationAction),
}

impl From<InstructionClass> for GenerationChoice {
    fn from(class: InstructionClass) -> Self {
        Self::Instruction(class)
    }
}

impl From<GenerationAction> for GenerationChoice {
    fn from(action: GenerationAction) -> Self {
        Self::Action(action)
    }
}

use GenerationAction as A;
use GenerationChoice::{Action, Instruction};
use InstructionClass as C;

/// Stable sampling order. Unsupported behaviors are disabled until their
/// required platform/state model exists. FP and atomic families are opt-in.
pub const DEFAULT_CHOICE_WEIGHTS: &[(GenerationChoice, f64)] = &[
    (Instruction(C::Alu), 0.1),
    (Instruction(C::Alu64), 0.1),
    (Instruction(C::MulDiv), 0.1),
    (Instruction(C::MulDiv64), 0.1),
    (Instruction(C::Jal), 0.01),
    (Instruction(C::Jalr), 0.01),
    (Instruction(C::Branch), 0.4),
    (Instruction(C::Memory), 0.5),
    (Instruction(C::Memory64), 0.5),
    (Instruction(C::Amo), 0.0),
    (Instruction(C::Amo64), 0.0),
    (Instruction(C::Fence), 1e-06),
    (Instruction(C::FloatMemory), 0.0),
    (Instruction(C::DoubleMemory), 0.0),
    (Instruction(C::Float), 0.0),
    (Instruction(C::Float64), 0.0),
    (Instruction(C::Double), 0.0),
    (Instruction(C::Double64), 0.0),
    (Instruction(C::Csr), 0.0),
    (Action(A::RegFsm), 0.1),
    (Action(A::CreateAddressDependency), 2.0),
    (Action(A::Exception), 0.1),
    (Action(A::FreePolluted), 0.0),
    (Action(A::FpuFsm), 0.0),
    (Action(A::TvecFsm), 0.0),
    (Action(A::EpcFsm), 0.0),
    (Action(A::MachineCsr), 0.0),
    (Action(A::DescendPrivilege), 0.0),
    (Action(A::WaitForInterruptTrap), 0.0),
    (Action(A::WaitForInterruptNoTrap), 0.0),
    (Action(A::SendIpi), 0.0),
    (Action(A::SendLocalInterrupt), 0.0),
    (Action(A::ClearInterrupt), 0.0),
    (Action(A::WriteInstruction), 0.0),
    (Action(A::WaitForInstruction), 0.0),
    (Action(A::ClearMdt), 0.0),
    (Action(A::ResetMcm), 0.0),
];

pub(super) fn default_choice_weights(
    is_64bit: bool,
    authorize_privileges: bool,
) -> Vec<(GenerationChoice, f64)> {
    DEFAULT_CHOICE_WEIGHTS
        .iter()
        .map(|&(choice, weight)| {
            let disabled = matches!(choice, Instruction(class)
            if (!is_64bit && class.requires_rv64()) || (!authorize_privileges && class == C::Csr));
            (choice, if disabled { 0.0 } else { weight })
        })
        .collect()
}

pub(super) fn choice_distribution(
    weights: &[(GenerationChoice, f64)],
) -> Result<WeightedIndex<f64>> {
    let total: f64 = weights.iter().map(|(_, w)| w).sum();
    if weights.iter().any(|(_, w)| !w.is_finite() || *w < 0.0) || !total.is_finite() || total <= 0.0
    {
        return Err("choice weights must be finite, nonnegative, and have a positive total".into());
    }
    Ok(WeightedIndex::new(weights.iter().map(|(_, w)| *w))?)
}
