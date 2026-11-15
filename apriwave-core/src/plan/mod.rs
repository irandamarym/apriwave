//! Execution plan (B2: `abdur=<N>` loops, B3: structural set resolution).
//!
//! The engine never reads the meaning of a glyph: iteration counts come only from
//! the `abdur=<N>` tile (L2) and target sets only from the structural `base:`
//! relations of enabled DTI keys and from `def` variable values (S1). The plan is
//! deterministic and validated; the host only carries out the physical effects.

use crate::prelude::*;

use alloc::collections::BTreeSet;
use core::fmt;

use crate::analyze::{ClauseAnalysis, ClauseId, GlobalClause, SlotIndex, SlotValue};
use crate::ast::{
    AblativeDeterminer, Case, ClauseRef, Comparative, GlyphSlot, Quantifier, RelationalBlock,
    TemporalMode,
};
use crate::dti::{Dti, DtiKey};
use crate::session::{PeriodId, Session};
use crate::stage3::{
    resolve_path, slot_content, IndexedAtom, PathResolution, ResolveError, SlotContent,
};

/// S2: how the host must treat a target set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuantifierMode {
    /// Every target (`all`, or a set built only by comparatives/cases).
    All,
    /// At least one of the candidates (`some`); the list stays deterministic.
    AtLeastOne,
}

/// Targets of a step, ordered by key and without duplicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSet {
    pub mode: QuantifierMode,
    pub keys: Vec<DtiKey>,
}

/// One step of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub clause: GlobalClause,
    /// One 0-based index per enclosing `abdur=<N>` loop, outermost first (the
    /// loops of one clause nest in written order).
    pub iteration: Box<[u32]>,
    pub targets: Option<TargetSet>,
}

/// Deterministic, validated execution plan of a period (L1).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub steps: Vec<Step>,
}

/// Errors while building a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    PeriodNotFound(PeriodId),
    /// Loop-generated steps beyond `max_loop_iterations` (L5, `ERR_SCOPE` family).
    LoopIterations,
    /// A set operand could not be resolved to a DTI key.
    Resolve {
        clause: ClauseId,
        error: ResolveError,
    },
    /// A set operand is not a concept (label, sema, numeral).
    UnsupportedOperand {
        clause: ClauseId,
    },
    /// A segmented operand `<K1>§<K2>` has no enabled record with `base:K1` and
    /// `param:K2` (`found` empty) or more than one (`found` lists them).
    Composite {
        clause: ClauseId,
        base: DtiKey,
        param: DtiKey,
        found: Vec<DtiKey>,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodNotFound(id) => write!(f, "period {} not found", id.index()),
            Self::LoopIterations => f.write_str("loop-generated steps beyond the configured limit"),
            Self::Resolve { clause, error } => {
                write!(f, "clause {}: {error}", clause.index())
            }
            Self::UnsupportedOperand { clause } => {
                write!(
                    f,
                    "clause {}: set operand is not a single DTI concept",
                    clause.index()
                )
            }
            Self::Composite {
                clause,
                base,
                param,
                found,
            } => write!(
                f,
                "clause {}: {} enabled records with base:{base} param:{param} (exactly one required)",
                clause.index(),
                found.len()
            ),
        }
    }
}

impl core::error::Error for PlanError {}

/// Set operation applied to an operand (S1).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Base,
    Union,
    Intersect,
    Subtract,
}

struct Planner<'s> {
    session: &'s Session,
    dti: &'s Dti,
    period: PeriodId,
    steps: Vec<Step>,
    /// Steps produced beyond the first occurrence of each clause.
    repeated: u64,
    limit: u64,
}

impl<'s> Planner<'s> {
    fn analysis(&self, clause: ClauseId) -> Option<&'s ClauseAnalysis> {
        self.session.analysis(self.period)?.clause(clause)
    }

    fn block(&self, clause: ClauseId) -> Option<&'s RelationalBlock> {
        match self.session.period(self.period)?.clause(clause.index())? {
            ClauseRef::Relational(block) => Some(block),
            ClauseRef::Definition(_) | ClauseRef::Portability(_) => None,
        }
    }

    /// `abdur=<N>` counts of a clause in written order (v4.1: several loops in
    /// one clause nest, the first written outermost: N₁ × N₂ × …).
    fn loop_counts(&self, clause: ClauseId) -> Vec<u32> {
        self.block(clause).map_or_else(Vec::new, |block| {
            block
                .cases
                .iter()
                .filter_map(|p| p.tile.node.case.loop_count())
                .map(|count| count.get())
                .collect()
        })
    }

    /// Loop-control subordinate: bare `abdur` marker in a clause with a written
    /// or inherited conditional (validated by stage 2).
    fn is_loop_control(&self, clause: ClauseId) -> bool {
        let conditional = self
            .analysis(clause)
            .and_then(ClauseAnalysis::signal)
            .is_some_and(|signal| matches!(signal.tense, Some(TemporalMode::Conditional(_))));
        conditional
            && self.block(clause).is_some_and(|block| {
                block.cases.iter().any(|p| {
                    p.tile.node.case == Case::Ablative(Some(AblativeDeterminer::Dur(None)))
                })
            })
    }

    fn push(&mut self, clause: ClauseId, iteration: &[u32], first: bool) -> Result<(), PlanError> {
        if !first {
            self.repeated = self.repeated.saturating_add(1);
            if self.repeated > self.limit {
                return Err(PlanError::LoopIterations);
            }
        }
        let targets = self.targets(clause)?;
        self.steps.push(Step {
            clause: GlobalClause {
                period: self.period,
                clause,
            },
            iteration: iteration.into(),
            targets,
        });
        Ok(())
    }

    /// Emits a clause and its subtree. Loop-control subordinates are emitted once,
    /// before the repeated body; the body (clause + other children) is repeated N times.
    fn visit(
        &mut self,
        clause: ClauseId,
        iteration: &mut Vec<u32>,
        first: bool,
    ) -> Result<(), PlanError> {
        let Some(analysis) = self.analysis(clause) else {
            return Ok(());
        };
        let (control, body): (Vec<ClauseId>, Vec<ClauseId>) = analysis
            .children()
            .iter()
            .copied()
            .partition(|c| self.is_loop_control(*c));
        for child in control {
            self.visit(child, iteration, first)?;
        }
        let counts = self.loop_counts(clause);
        // Odometer over the nested loops of the clause (innermost = last written).
        let mut rounds: Vec<u32> = vec![0; counts.len()];
        let outer = iteration.len();
        loop {
            let first_round = first && rounds.iter().all(|round| *round == 0);
            iteration.extend_from_slice(&rounds);
            self.push(clause, iteration, first_round)?;
            for child in &body {
                self.visit(*child, iteration, first_round)?;
            }
            iteration.truncate(outer);
            if !advance(&mut rounds, &counts) {
                break;
            }
        }
        Ok(())
    }

    /// DTI keys of an operand value (variables contribute all their values).
    fn operand_keys(&self, value: SlotValue, clause: ClauseId) -> Result<Vec<DtiKey>, PlanError> {
        match value {
            SlotValue::Glyph(slot) => {
                let Some(GlyphSlot::Glyph(path)) = self.session.glyph_slot(slot) else {
                    return Err(PlanError::UnsupportedOperand { clause });
                };
                let text = self
                    .session
                    .interner()
                    .resolve(path.text)
                    .ok_or(PlanError::UnsupportedOperand { clause })?;
                // A label is not a concept (v4.1).
                if slot_content(self.session, slot, text) != SlotContent::Glyph {
                    return Err(PlanError::UnsupportedOperand { clause });
                }
                match resolve_path(path, self.session.interner(), self.dti)
                    .map_err(|error| PlanError::Resolve { clause, error })?
                {
                    PathResolution::Record(key) => Ok(vec![key]),
                    PathResolution::Segments(atoms) => {
                        self.composite(&atoms, clause).map(|k| vec![k])
                    }
                }
            }
            SlotValue::Variable(binding) => {
                let values = self
                    .session
                    .binding(binding)
                    .map(|b| b.values())
                    .ok_or(PlanError::UnsupportedOperand { clause })?;
                let mut keys = Vec::new();
                for value in values {
                    // Variable values never hold variables of their own beyond one
                    // level here; nested bindings are rejected as unsupported.
                    if matches!(value, SlotValue::Variable(_)) {
                        return Err(PlanError::UnsupportedOperand { clause });
                    }
                    keys.extend(self.operand_keys(*value, clause)?);
                }
                Ok(keys)
            }
            SlotValue::Sema(_) => Err(PlanError::UnsupportedOperand { clause }),
        }
    }

    /// §14: a segmented operand `<K1>§<K2>§…§<Kn>` is the enabled record with
    /// `base:K1` and `param:K2`, folded from the left for longer chains.
    fn composite(&self, atoms: &[IndexedAtom], clause: ClauseId) -> Result<DtiKey, PlanError> {
        let mut keys = atoms.iter().map(|atom| match atom {
            IndexedAtom::Key(key) => Ok(*key),
            IndexedAtom::Numeral(_) => Err(PlanError::UnsupportedOperand { clause }),
        });
        let mut current = keys
            .next()
            .ok_or(PlanError::UnsupportedOperand { clause })??;
        for param in keys {
            let param = param?;
            let found = self.dti.compounds(&current, &param);
            current = match found.as_slice() {
                [single] => *single,
                _ => {
                    return Err(PlanError::Composite {
                        clause,
                        base: current,
                        param,
                        found,
                    })
                }
            };
        }
        Ok(current)
    }

    fn expanded(&self, key: DtiKey) -> BTreeSet<DtiKey> {
        let mut set = self.dti.descendants(&key);
        set.insert(key);
        set
    }

    /// S1/S2/S3: target set of a clause, or `None` when it has no set semantics
    /// (neither a quantifier nor a `cup`/`cua`/`cuo` comparative).
    fn targets(&self, clause: ClauseId) -> Result<Option<TargetSet>, PlanError> {
        let (Some(block), Some(analysis)) = (self.block(clause), self.analysis(clause)) else {
            return Ok(None);
        };
        let Some(signal) = analysis.signal() else {
            return Ok(None);
        };
        let comparative = signal.comparative;
        let set_comparative = matches!(
            comparative,
            Some(Comparative::Cup | Comparative::Cua | Comparative::Cuo)
        );
        if signal.quantifier.is_none() && !set_comparative {
            return Ok(None);
        }
        let quantified = signal.quantifier.is_some();
        let mut result: BTreeSet<DtiKey> = BTreeSet::new();
        let mut intersect: Option<BTreeSet<DtiKey>> = None;
        let mut subtract: BTreeSet<DtiKey> = BTreeSet::new();

        // The base set is formed by the Accusative pairs; without any, by the
        // Nominative pairs (e.g. `/^≈ deepall avvio nm sensore abse sensore§ottico ^/`).
        let base_case = if block
            .cases
            .iter()
            .any(|pair| pair.tile.node.case == Case::Accusative)
        {
            Case::Accusative
        } else {
            Case::Nominative
        };
        for (offset, pair) in block.cases.iter().enumerate() {
            let operation = match pair.tile.node.case {
                case if case == base_case => Operation::Base,
                Case::Ablative(Some(AblativeDeterminer::Si)) => match comparative {
                    Some(Comparative::Cua) => Operation::Intersect,
                    Some(Comparative::Cuo) => Operation::Subtract,
                    _ => Operation::Union,
                },
                Case::Ablative(Some(AblativeDeterminer::Pa)) => Operation::Intersect,
                Case::Ablative(Some(AblativeDeterminer::Se)) => Operation::Subtract,
                _ => continue,
            };
            let position = offset
                .checked_add(1)
                .and_then(|p| u32::try_from(p).ok())
                .ok_or(PlanError::UnsupportedOperand { clause })?;
            let Some(slot) = analysis.slot(SlotIndex::new(position)) else {
                return Err(PlanError::UnsupportedOperand { clause });
            };
            for key in self.operand_keys(slot.value(), clause)? {
                match operation {
                    Operation::Base | Operation::Union => {
                        if quantified {
                            result.extend(self.expanded(key));
                        } else {
                            result.insert(key);
                        }
                    }
                    Operation::Intersect => {
                        intersect
                            .get_or_insert_with(BTreeSet::new)
                            .extend(self.expanded(key));
                    }
                    Operation::Subtract => subtract.extend(self.expanded(key)),
                }
            }
        }
        if let Some(filter) = intersect {
            result.retain(|key| filter.contains(key));
        }
        result.retain(|key| !subtract.contains(key));
        let mode = match signal.quantifier {
            Some(Quantifier::Existential) => QuantifierMode::AtLeastOne,
            Some(Quantifier::Universal) | None => QuantifierMode::All,
        };
        Ok(Some(TargetSet {
            mode,
            keys: result.into_iter().collect(),
        }))
    }
}

/// Advances the odometer `rounds` (one digit per loop, bounded by `counts`);
/// `false` once every combination has been produced.
fn advance(rounds: &mut [u32], counts: &[u32]) -> bool {
    for (round, count) in rounds.iter_mut().zip(counts).rev() {
        *round = round.saturating_add(1);
        if *round < *count {
            return true;
        }
        *round = 0;
    }
    false
}

/// Builds the execution plan of a period: relational clauses in tree pre-order
/// (roots in order, children in order), loops unrolled (L1, L3), targets resolved
/// (B3). `def` and portability blocks produce no step.
pub fn build_execution_plan(
    session: &Session,
    id: PeriodId,
    dti: &Dti,
) -> Result<ExecutionPlan, PlanError> {
    let analysis = session.analysis(id).ok_or(PlanError::PeriodNotFound(id))?;
    let mut planner = Planner {
        session,
        dti,
        period: id,
        steps: Vec::new(),
        repeated: 0,
        limit: u64::from(session.config().max_loop_iterations.get()),
    };
    let mut iteration = Vec::new();
    for root in analysis.roots() {
        planner.visit(*root, &mut iteration, true)?;
    }
    Ok(ExecutionPlan {
        steps: planner.steps,
    })
}

#[cfg(test)]
mod tests;
