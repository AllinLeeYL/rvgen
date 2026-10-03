//! Instruction weights: how often each opcode appears in the workload.
//!
//! The orchestrator owns a [`WeightPolicy`] and draws one [`InstrWeights`] per
//! basic block before generation; blocks only sample from the weights they are
//! handed and never decide the instruction mix themselves.
use std::collections::HashMap;

use anyhow::{Result, anyhow, bail, ensure};
use rand::Rng;
use rand::distr::{Distribution, weighted::WeightedIndex};
use rand::rngs::StdRng;

use crate::riscv::{InstructionClass, Opcode};
use crate::target::Target;

/// Opcodes generated as control flow: the six base conditional branches and
/// `jalr`. They only exist as entanglement sites (see [`crate::entangle`]), so
/// they are drawn like any other opcode, but only when entangling.
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
        return Some("only the base conditional branches and jalr are generated");
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

/// Relative opcode weights. Opcodes without an explicit weight get `default`;
/// a weight of 0 excludes the opcode.
#[derive(Debug, Clone)]
pub struct InstrWeights {
    default: f64,
    opcodes: HashMap<Opcode, f64>,
}

impl Default for InstrWeights {
    fn default() -> Self {
        Self::uniform()
    }
}

impl InstrWeights {
    /// Every opcode equally likely: the generator's historical behavior.
    pub fn uniform() -> Self {
        Self::with_default(1.0)
    }

    /// Weight `default` for every opcode not set explicitly.
    pub fn with_default(default: f64) -> Self {
        Self {
            default,
            opcodes: HashMap::new(),
        }
    }

    pub fn set(&mut self, opcode: Opcode, weight: f64) -> &mut Self {
        self.opcodes.insert(opcode, weight);
        self
    }

    /// Give every opcode of `class` the same weight.
    pub fn set_class(&mut self, class: InstructionClass, weight: f64) -> &mut Self {
        for opcode in class.opcodes() {
            self.set(opcode, weight);
        }
        self
    }

    pub fn get(&self, opcode: Opcode) -> f64 {
        self.opcodes.get(&opcode).copied().unwrap_or_else(|| {
            if is_control_flow(opcode) {
                self.default * CONTROL_FLOW_DEFAULT_SCALE
            } else {
                self.default
            }
        })
    }

    /// A sampler over `candidates` (the opcodes legal at the sampling site).
    /// Fails if every candidate has zero weight or a weight is invalid.
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

/// Weights as requested by the user: opcode weights beat class weights, which
/// beat `default`.
#[derive(Debug, Clone)]
pub struct WeightSpec {
    pub default: f64,
    pub classes: Vec<(InstructionClass, f64)>,
    pub opcodes: Vec<(Opcode, f64)>,
}

impl Default for WeightSpec {
    fn default() -> Self {
        Self {
            default: 1.0,
            classes: Vec::new(),
            opcodes: Vec::new(),
        }
    }
}

impl WeightSpec {
    /// Check the spec against the target and resolve it. Weights that could
    /// never take effect are errors rather than silent no-ops.
    pub fn build(&self, target: &Target, entangle: bool) -> Result<InstrWeights> {
        check_weight("default weight", self.default)?;
        let mut weights = InstrWeights::with_default(self.default);
        for &(class, weight) in &self.classes {
            check_weight(&format!("weight of class {class:?}"), weight)?;
            ensure!(
                weight == 0.0
                    || entangle
                    || !matches!(class, InstructionClass::Branch | InstructionClass::Jalr),
                "class {class:?} is only generated with entanglement (self-check on, \
                 no --no-entangle), so it cannot be given a nonzero weight"
            );
            weights.set_class(class, weight);
        }
        for &(opcode, weight) in &self.opcodes {
            check_weight(&format!("weight of {opcode}"), weight)?;
            if weight == 0.0 {
                weights.set(opcode, weight);
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
            weights.set(opcode, weight);
        }
        Ok(weights)
    }
}

fn check_weight(what: &str, weight: f64) -> Result<()> {
    ensure!(
        weight.is_finite() && weight >= 0.0,
        "{what} must be a finite number >= 0, got {weight}"
    );
    Ok(())
}

/// Where in the program a block's weights are drawn for.
#[derive(Debug, Clone, Copy)]
pub struct BlockContext {
    pub hart: usize,
    pub block: usize,
    pub budget: usize,
}

/// Decides the instruction mix. The orchestrator calls [`WeightPolicy::draw`]
/// once per basic block, in program order, before that hart is generated.
pub trait WeightPolicy {
    fn draw(&mut self, ctx: &BlockContext, rng: &mut StdRng) -> InstrWeights;
}

/// Every block uses the same uniform weights.
pub struct Uniform;

impl WeightPolicy for Uniform {
    fn draw(&mut self, _ctx: &BlockContext, _rng: &mut StdRng) -> InstrWeights {
        InstrWeights::uniform()
    }
}

/// Every block uses the same fixed weights.
pub struct Fixed(pub InstrWeights);

impl WeightPolicy for Fixed {
    fn draw(&mut self, _ctx: &BlockContext, _rng: &mut StdRng) -> InstrWeights {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryRegion, Permissions};
    use crate::riscv::{Extension, Xlen};
    use rand::SeedableRng;
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
            0,
            0,
        )
        .unwrap()
    }

    fn imzicsr() -> Target {
        target(&[Extension::I, Extension::M, Extension::Zicsr], &[])
    }

    fn spec(classes: &[(InstructionClass, f64)], opcodes: &[(Opcode, f64)]) -> WeightSpec {
        WeightSpec {
            default: 1.0,
            classes: classes.to_vec(),
            opcodes: opcodes.to_vec(),
        }
    }

    #[test]
    fn opcode_beats_class_beats_default() {
        let weights = spec(
            &[(InstructionClass::MulDiv, 4.0)],
            &[(Opcode::Mulh, 9.0), (Opcode::Sub, 0.0)],
        )
        .build(&imzicsr(), true)
        .unwrap();
        assert_eq!(weights.get(Opcode::Mul), 4.0);
        assert_eq!(weights.get(Opcode::Mulh), 9.0);
        assert_eq!(weights.get(Opcode::Add), 1.0);
        assert_eq!(weights.get(Opcode::Sub), 0.0);
    }

    #[test]
    fn opcode_beats_class_regardless_of_flag_order() {
        // `build` applies classes before opcodes, whatever order they came in.
        let weights = WeightSpec {
            default: 1.0,
            classes: vec![(InstructionClass::Alu, 2.0)],
            opcodes: vec![(Opcode::Add, 7.0)],
        }
        .build(&imzicsr(), true)
        .unwrap();
        assert_eq!(weights.get(Opcode::Add), 7.0);
        assert_eq!(weights.get(Opcode::Xor), 2.0);
    }

    #[test]
    fn control_flow_defaults_to_a_fraction_of_the_default() {
        let weights = WeightSpec {
            default: 4.0,
            ..WeightSpec::default()
        }
        .build(&imzicsr(), true)
        .unwrap();
        assert_eq!(weights.get(Opcode::Add), 4.0);
        assert_eq!(weights.get(Opcode::Beq), 4.0 * CONTROL_FLOW_DEFAULT_SCALE);
        assert_eq!(weights.get(Opcode::Jalr), 4.0 * CONTROL_FLOW_DEFAULT_SCALE);
        // An explicit weight, even through a class, is taken as given.
        let weights = spec(&[(InstructionClass::Branch, 3.0)], &[])
            .build(&imzicsr(), true)
            .unwrap();
        assert_eq!(weights.get(Opcode::Bltu), 3.0);
    }

    #[test]
    fn invalid_weights_are_rejected() {
        let t = imzicsr();
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(spec(&[], &[(Opcode::Add, bad)]).build(&t, true).is_err());
            assert!(spec(&[(InstructionClass::Alu, bad)], &[]).build(&t, true).is_err());
            let default = WeightSpec { default: bad, ..WeightSpec::default() };
            assert!(default.build(&t, true).is_err());
        }
    }

    #[test]
    fn weights_that_cannot_take_effect_are_rejected() {
        let t = imzicsr();
        let rejected = |opcode| spec(&[], &[(opcode, 1.0)]).build(&t, true).is_err();
        assert!(rejected(Opcode::Ecall), "excluded for safety");
        assert!(rejected(Opcode::Jal), "never generated");
        assert!(rejected(Opcode::CBeqz), "compressed branches are never generated");
        assert!(rejected(Opcode::FaddS), "F is not enabled");
        // Zero is always accepted: it excludes nothing that could appear.
        assert!(spec(&[], &[(Opcode::Ecall, 0.0), (Opcode::FaddS, 0.0)]).build(&t, true).is_ok());
        // Disabled opcodes cannot be revived by a weight.
        let no_xor = target(&[Extension::I, Extension::Zicsr], &[Opcode::Xor]);
        assert!(spec(&[], &[(Opcode::Xor, 1.0)]).build(&no_xor, true).is_err());
    }

    #[test]
    fn branch_weights_need_entanglement() {
        let t = imzicsr();
        assert!(spec(&[], &[(Opcode::Beq, 2.0)]).build(&t, false).is_err());
        assert!(spec(&[], &[(Opcode::Jalr, 2.0)]).build(&t, false).is_err());
        assert!(spec(&[(InstructionClass::Branch, 2.0)], &[]).build(&t, false).is_err());
        assert!(spec(&[(InstructionClass::Jalr, 2.0)], &[]).build(&t, false).is_err());
        assert!(spec(&[(InstructionClass::Branch, 0.0)], &[(Opcode::Beq, 0.0)]).build(&t, false).is_ok());
        assert!(spec(&[(InstructionClass::Alu, 2.0)], &[]).build(&t, false).is_ok());
    }

    #[test]
    fn sampler_never_draws_a_zero_weight_opcode() {
        let weights = spec(&[], &[(Opcode::Add, 0.0), (Opcode::Sub, 5.0), (Opcode::Xor, 1.0)])
            .build(&imzicsr(), true)
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
                .build(&imzicsr(), true)
                .unwrap()
                .sampler(&[Opcode::Add, Opcode::Sub])
                .is_err()
        );
    }
}
