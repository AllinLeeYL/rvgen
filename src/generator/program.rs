//! Core and basic-block storage, lowering, and bytecode emission.

use std::ops::Range;

use rand::{distributions::Distribution, rngs::StdRng};

use crate::{Instruction, Result};

use super::{
    params::partition, policy::choice_distribution, BasicBlockGeneratorParams, CoreGeneratorParams,
    GenerationChoice, GenerationContext,
};

/// A successfully lowered choice and its range in the block's `insts` vector.
/// Empty ranges represent generator-only actions. Ranges describe generation;
/// callers editing `insts` afterward must update or discard these records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedChoice {
    pub choice: GenerationChoice,
    pub instructions: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct BasicBlockGenerator {
    pub params: BasicBlockGeneratorParams,
    pub insts: Vec<Instruction>,
    pub selected_choices: Vec<SelectedChoice>,
}

impl BasicBlockGenerator {
    pub fn new(params: BasicBlockGeneratorParams) -> Self {
        Self {
            params,
            insts: Vec::new(),
            selected_choices: Vec::new(),
        }
    }

    /// Replace this block with exactly `num_insts` workload instructions.
    /// Sequences are never truncated. Choices that cannot fit are excluded for
    /// the remaining budget; an unfillable budget is an error. A zero-output
    /// choice is sampled at most once between successive instruction emissions,
    /// guaranteeing termination even for policies containing only no-op actions.
    /// On error the existing block is retained.
    pub fn generate(
        &mut self,
        prng: &mut StdRng,
        weights: &[(GenerationChoice, f64)],
        context: &GenerationContext,
    ) -> Result<()> {
        choice_distribution(weights)?;
        for &(choice, weight) in weights {
            if weight > 0.0 {
                choice.validate(context)?;
            }
        }
        let mut insts = Vec::new();
        let mut selected_choices = Vec::new();
        while insts.len() < self.params.num_insts {
            let mut eligible = weights.to_vec();
            loop {
                let distribution = choice_distribution(&eligible).map_err(|_| {
                    format!("cannot fill block {}: no choice can emit instructions within the remaining budget of {}",
                        self.params.label, self.params.num_insts - insts.len())
                })?;
                let index = distribution.sample(prng);
                let choice = eligible[index].0;
                let instructions = choice.lower(prng, context)?;
                if instructions.len() > self.params.num_insts - insts.len() {
                    eligible[index].1 = 0.0;
                    continue;
                }
                let start = insts.len();
                insts.extend(instructions);
                selected_choices.push(SelectedChoice {
                    choice,
                    instructions: start..insts.len(),
                });
                if insts.len() != start {
                    break;
                }
                // Suppress every duplicate of the no-op choice until progress.
                for (candidate, weight) in &mut eligible {
                    if *candidate == choice {
                        *weight = 0.0;
                    }
                }
            }
        }
        self.insts = insts;
        self.selected_choices = selected_choices;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CoreGenerator {
    pub params: CoreGeneratorParams,
    pub bbs: Vec<BasicBlockGenerator>,
}

impl CoreGenerator {
    pub fn new(params: CoreGeneratorParams) -> Self {
        Self {
            params,
            bbs: (0..params.num_bbs)
                .map(|i| {
                    BasicBlockGenerator::new(BasicBlockGeneratorParams {
                        num_insts: partition(params.num_insts, params.num_bbs, i),
                        label: format!("bb_{i}"),
                    })
                })
                .collect(),
        }
    }

    pub fn generate(
        &mut self,
        prng: &mut StdRng,
        weights: &[(GenerationChoice, f64)],
        context: &GenerationContext,
    ) -> Result<()> {
        if self.params.num_bbs == 0 {
            return Err("num_bbs must be greater than zero".into());
        }
        let mut bbs = Self::new(self.params).bbs;
        for bb in &mut bbs {
            bb.generate(prng, weights, context)?;
        }
        self.bbs = bbs;
        Ok(())
    }

    pub fn get_bytecode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for instruction in self.bbs.iter().flat_map(|bb| &bb.insts) {
            instruction.append_bytes(&mut bytes);
        }
        bytes
    }

    pub fn get_section_addr(&self) -> u64 {
        0
    }
}
