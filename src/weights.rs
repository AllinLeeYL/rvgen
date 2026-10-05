//! Instruction weights: how often each opcode appears in the workload.
//!
//! The orchestrator owns one [`InstrWeights`] and hands it to every basic
//! block; blocks only sample from it and never decide the instruction mix
//! themselves.
use std::collections::HashMap;

use anyhow::{Result, anyhow, bail, ensure};
use rand::Rng;
use rand::distr::{Distribution, weighted::WeightedIndex};

use crate::riscv::{InstructionClass, Opcode};
use crate::target::Target;

/// Opcodes generated as control flow: the six base conditional branches,
/// `jalr`, and the compressed `c.beqz`, `c.bnez`, `c.j`, `c.jr` and `c.jalr`.
/// They only exist as entanglement sites (see [`crate::entangle`]), so they
/// are drawn like any other opcode, but only when entangling.
pub fn is_control_flow(opcode: Opcode) -> bool {
    matches!(
        opcode,
        Opcode::Beq
            | Opcode::Bne
            | Opcode::Blt
            | Opcode::Bge
            | Opcode::Bltu
            | Opcode::Bgeu
            | Opcode::Jalr
            | Opcode::CBeqz
            | Opcode::CBnez
            | Opcode::CJ
            | Opcode::CJr
            | Opcode::CJalr
    )
}

/// Why the workload can never contain `opcode`, if so. Such opcodes cannot be
/// given a nonzero weight.
pub fn unweightable_reason(opcode: Opcode) -> Option<&'static str> {
    if is_control_flow(opcode) {
        return None;
    }
    if matches!(
        opcode.class(),
        InstructionClass::Branch | InstructionClass::Jal | InstructionClass::Jalr
    ) {
        return Some("only the conditional branches, jalr, c.j, c.jr and c.jalr are generated");
    }
    matches!(
        opcode,
        Opcode::Ecall | Opcode::Ebreak | Opcode::CEbreak | Opcode::Sret | Opcode::Mret
    )
    .then_some("it would end the program or leave the handled privilege level")
}

/// Control-flow opcodes without an explicit weight get this fraction of the
/// default weight, which keeps roughly the branch density the generator had
/// before branches were weighted.
pub const CONTROL_FLOW_DEFAULT_SCALE: f64 = 0.25;

/// Relative opcode weights, as requested by the user. An opcode's weight is
/// its own entry if it has one, else its class's entry, else `default`; a
/// weight of 0 excludes the opcode.
#[derive(Debug, Clone)]
pub struct InstrWeights {
    pub default: f64,
    pub classes: HashMap<InstructionClass, f64>,
    pub opcodes: HashMap<Opcode, f64>,
}

impl Default for InstrWeights {
    /// Every opcode equally likely (control flow a bit less, see
    /// [`CONTROL_FLOW_DEFAULT_SCALE`]).
    fn default() -> Self {
        Self {
            default: 1.0,
            classes: HashMap::new(),
            opcodes: HashMap::new(),
        }
    }
}

impl InstrWeights {
    pub fn get(&self, opcode: Opcode) -> f64 {
        if let Some(&weight) = self.opcodes.get(&opcode) {
            weight
        } else if let Some(&weight) = self.classes.get(&opcode.class()) {
            weight
        } else if is_control_flow(opcode) {
            self.default * CONTROL_FLOW_DEFAULT_SCALE
        } else {
            self.default
        }
    }

    /// Check the weights against the target. Weights that could never take
    /// effect are errors rather than silent no-ops.
    pub fn checked(self, target: &Target, entangle: bool) -> Result<Self> {
        check_weight("default weight", self.default)?;
        for (&class, &weight) in &self.classes {
            check_weight(&format!("weight of class {class:?}"), weight)?;
            ensure!(
                weight == 0.0
                    || entangle
                    || !matches!(class, InstructionClass::Branch | InstructionClass::Jal | InstructionClass::Jalr),
                "class {class:?} is only generated with entanglement (self-check on, \
                 no --no-entangle), so it cannot be given a nonzero weight"
            );
        }
        for (&opcode, &weight) in &self.opcodes {
            check_weight(&format!("weight of {opcode}"), weight)?;
            if weight == 0.0 {
                continue;
            }
            if let Some(reason) = unweightable_reason(opcode) {
                bail!("{opcode} cannot have a nonzero weight: {reason}");
            }
            ensure!(
                target.supports(opcode),
                "{opcode} is not supported by this --isa/--xlen"
            );
            ensure!(
                !target.disabled_opcodes.contains(&opcode),
                "{opcode} is in --disabled-instrs"
            );
            ensure!(
                entangle || !is_control_flow(opcode),
                "{opcode} is only generated with entanglement (self-check on, \
                 no --no-entangle), so it cannot be given a nonzero weight"
            );
        }
        Ok(self)
    }

    /// A sampler over `candidates` (the opcodes legal at the sampling site).
    /// Fails if every candidate has zero weight.
    pub fn sampler(&self, candidates: &[Opcode]) -> Result<OpcodeSampler> {
        let (opcodes, weights): (Vec<_>, Vec<_>) = candidates
            .iter()
            .map(|&opcode| (opcode, self.get(opcode)))
            .filter(|&(_, weight)| weight != 0.0)
            .unzip();
        ensure!(
            !opcodes.is_empty(),
            "every enabled instruction has zero weight"
        );
        let index = WeightedIndex::new(&weights)
            .map_err(|error| anyhow!("invalid instruction weights: {error}"))?;
        Ok(OpcodeSampler { opcodes, index })
    }
}

/// Draws opcodes according to an [`InstrWeights`] restricted to a candidate set.
pub struct OpcodeSampler {
    opcodes: Vec<Opcode>,
    index: WeightedIndex<f64>,
}

impl OpcodeSampler {
    pub fn sample(&self, rng: &mut (impl Rng + ?Sized)) -> Opcode {
        self.opcodes[self.index.sample(rng)]
    }
}

fn check_weight(what: &str, weight: f64) -> Result<()> {
    ensure!(
        weight.is_finite() && weight >= 0.0,
        "{what} must be a finite number >= 0, got {weight}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryRegion, Permissions};
    use crate::riscv::{Extension, Xlen};
    use rand::{SeedableRng, rngs::StdRng};
    use std::collections::HashSet;

    fn target(isa: &[Extension], disabled: &[Opcode]) -> Target {
        Target::new(
            Xlen::X64,
            isa.iter().copied(),
            [crate::riscv::PrivilegeLevel::Machine],
            disabled.iter().copied().collect::<HashSet<_>>(),
            1,
            100,
            MemoryRegion {
                start: 0x8000_0000,
                size: 0x0800_0000,
                permissions: Permissions::default(),
            },
            Some(0),
            0,
            0,
        )
        .unwrap()
    }

    fn imzicsr() -> Target {
        target(&[Extension::I, Extension::M, Extension::Zicsr], &[])
    }

    fn spec(classes: &[(InstructionClass, f64)], opcodes: &[(Opcode, f64)]) -> InstrWeights {
        InstrWeights {
            default: 1.0,
            classes: classes.iter().copied().collect(),
            opcodes: opcodes.iter().copied().collect(),
        }
    }

    #[test]
    fn opcode_beats_class_beats_default() {
        let weights = spec(
            &[(InstructionClass::MulDiv, 4.0)],
            &[(Opcode::Mulh, 9.0), (Opcode::Sub, 0.0)],
        )
        .checked(&imzicsr(), true)
        .unwrap();
        assert_eq!(weights.get(Opcode::Mul), 4.0);
        assert_eq!(weights.get(Opcode::Mulh), 9.0);
        assert_eq!(weights.get(Opcode::Add), 1.0);
        assert_eq!(weights.get(Opcode::Sub), 0.0);
    }

    #[test]
    fn opcode_beats_class_regardless_of_flag_order() {
        let weights = spec(&[(InstructionClass::Alu, 2.0)], &[(Opcode::Add, 7.0)])
        .checked(&imzicsr(), true)
        .unwrap();
        assert_eq!(weights.get(Opcode::Add), 7.0);
        assert_eq!(weights.get(Opcode::Xor), 2.0);
    }

    #[test]
    fn control_flow_defaults_to_a_fraction_of_the_default() {
        let weights = InstrWeights {
            default: 4.0,
            ..InstrWeights::default()
        }
        .checked(&imzicsr(), true)
        .unwrap();
        assert_eq!(weights.get(Opcode::Add), 4.0);
        assert_eq!(weights.get(Opcode::Beq), 4.0 * CONTROL_FLOW_DEFAULT_SCALE);
        assert_eq!(weights.get(Opcode::Jalr), 4.0 * CONTROL_FLOW_DEFAULT_SCALE);
        // An explicit weight, even through a class, is taken as given.
        let weights = spec(&[(InstructionClass::Branch, 3.0)], &[])
            .checked(&imzicsr(), true)
            .unwrap();
        assert_eq!(weights.get(Opcode::Bltu), 3.0);
    }

    #[test]
    fn invalid_weights_are_rejected() {
        let t = imzicsr();
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(spec(&[], &[(Opcode::Add, bad)]).checked(&t, true).is_err());
            assert!(spec(&[(InstructionClass::Alu, bad)], &[]).checked(&t, true).is_err());
            let default = InstrWeights { default: bad, ..InstrWeights::default() };
            assert!(default.checked(&t, true).is_err());
        }
    }

    #[test]
    fn weights_that_cannot_take_effect_are_rejected() {
        let t = imzicsr();
        let rejected = |opcode| spec(&[], &[(opcode, 1.0)]).checked(&t, true).is_err();
        assert!(rejected(Opcode::Ecall), "excluded for safety");
        assert!(rejected(Opcode::Jal), "never generated");
        assert!(rejected(Opcode::CBeqz), "C is not enabled");
        assert!(rejected(Opcode::CJal), "never generated");
        let c = target(&[Extension::I, Extension::C, Extension::Zicsr], &[]);
        for opcode in [Opcode::CBeqz, Opcode::CBnez, Opcode::CJ, Opcode::CJr, Opcode::CJalr] {
            assert!(spec(&[], &[(opcode, 1.0)]).checked(&c, true).is_ok(), "{opcode}");
            assert!(spec(&[], &[(opcode, 1.0)]).checked(&c, false).is_err(), "{opcode} needs entanglement");
        }
        assert!(rejected(Opcode::FaddS), "F is not enabled");
        // Zero is always accepted: it excludes nothing that could appear.
        assert!(spec(&[], &[(Opcode::Ecall, 0.0), (Opcode::FaddS, 0.0)]).checked(&t, true).is_ok());
        // Disabled opcodes cannot be revived by a weight.
        let no_xor = target(&[Extension::I, Extension::Zicsr], &[Opcode::Xor]);
        assert!(spec(&[], &[(Opcode::Xor, 1.0)]).checked(&no_xor, true).is_err());
    }

    #[test]
    fn branch_weights_need_entanglement() {
        let t = imzicsr();
        assert!(spec(&[], &[(Opcode::Beq, 2.0)]).checked(&t, false).is_err());
        assert!(spec(&[], &[(Opcode::Jalr, 2.0)]).checked(&t, false).is_err());
        assert!(spec(&[(InstructionClass::Branch, 2.0)], &[]).checked(&t, false).is_err());
        assert!(spec(&[(InstructionClass::Jalr, 2.0)], &[]).checked(&t, false).is_err());
        assert!(spec(&[(InstructionClass::Branch, 0.0)], &[(Opcode::Beq, 0.0)]).checked(&t, false).is_ok());
        assert!(spec(&[(InstructionClass::Alu, 2.0)], &[]).checked(&t, false).is_ok());
    }

    #[test]
    fn sampler_never_draws_a_zero_weight_opcode() {
        let weights = spec(&[], &[(Opcode::Add, 0.0), (Opcode::Sub, 5.0), (Opcode::Xor, 1.0)])
            .checked(&imzicsr(), true)
            .unwrap();
        let sampler = weights
            .sampler(&[Opcode::Add, Opcode::Sub, Opcode::Xor])
            .unwrap();
        let mut rng = StdRng::seed_from_u64(1);
        let mut subs = 0;
        for _ in 0..6000 {
            match sampler.sample(&mut rng) {
                Opcode::Add => panic!("zero-weight opcode drawn"),
                Opcode::Sub => subs += 1,
                _ => {}
            }
        }
        // 5/6 of the draws, within a wide margin.
        assert!((4500..5500).contains(&subs), "{subs} subs");
        assert!(
            spec(&[], &[(Opcode::Add, 0.0), (Opcode::Sub, 0.0)])
                .checked(&imzicsr(), true)
                .unwrap()
                .sampler(&[Opcode::Add, Opcode::Sub])
                .is_err()
        );
    }
}
