//! Stage 3 expanded rendering (D8, C.1).
//!
//! The expanded form removes `def` blocks and portability blocks, writes explicit
//! colon runs for every clause at depth ≥ 1 and substitutes variables and glyph
//! anaphora with their resolved values:
//!
//! - in a **case pair** a variable with several values (after `absi`) becomes the
//!   original tile (with its effective number) followed by one `absi` pair per
//!   further value;
//! - an omitted number inherited by an anaphora or a variable in a case pair is
//!   written in the rewritten tile (Q18, rule 1B); the signal tile is kept as
//!   written (an omitted number is inherited again from the parent signal by
//!   any re-reading, rule 1B v4.1);
//! - `~~` in any even slot (signal glyph slot included) stays `~~`: it always
//!   denotes the latest complete sema, which removing `def` and portability
//!   blocks never changes (they hold no relational clause).
//!
//! It is an export error
//! (never an invalid or semantically different output) when something cannot be
//! expressed: multi-valued variables in the signal pair, depths that need
//! anchors, sema values held in glyph positions.
//!
//! The indexed wire form does **not** derive from this form: see `indexed.rs`.

use crate::prelude::*;

use core::fmt;

use crate::analyze::{
    BindingId, ClauseAnalysis, ClauseId, DspRole, GlobalClause, Resolution, SlotAnalysis,
    SlotIndex, SlotRef, SlotValue,
};
use crate::ast::{
    AblativeDeterminer, Case, CaseTile, ClauseRef, Conjunction, ExplicitNumber, GlyphPath,
    GlyphSlot, PeriodClose, PeriodOpen, QuantumNumber, RelationalBlock, ANAPHORA_MARK,
    SEMA_ANAPHORA_MARK,
};
use crate::printer::{self, render_glyph_path};
use crate::session::{PeriodId, Session};

/// Bound on variable indirections followed while flattening values.
const MAX_VALUE_STEPS: u32 = 1_000_000;

/// Channel mode (D4): agreed out of band, never signalled inside the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireMode {
    /// Written canonical form with lexical glyphs (human readable).
    Lexical,
    /// Written canonical form with DTI keys (the only M2M exchange form).
    Indexed,
}

/// Why a period cannot be expanded (D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandError {
    PeriodNotFound(PeriodId),
    /// A variable with several values (after `absi`) in the signal pair.
    MultiValuedVariable {
        clause: ClauseId,
    },
    /// The clause depth cannot be written without portability blocks.
    InexpressibleDepth {
        clause: ClauseId,
    },
    /// A value that cannot occupy a glyph position (e.g. a sema held by a variable).
    InexpressibleValue {
        clause: ClauseId,
    },
    /// Only `def`/portability blocks: nothing remains to transmit.
    EmptyExpansion,
    UnknownSymbol,
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodNotFound(id) => write!(f, "period {} not found", id.index()),
            Self::MultiValuedVariable { clause } => write!(
                f,
                "clause {}: multi-valued variable in the signal pair",
                clause.index()
            ),
            Self::InexpressibleDepth { clause } => write!(
                f,
                "clause {}: depth not expressible without portability blocks",
                clause.index()
            ),
            Self::InexpressibleValue { clause } => write!(
                f,
                "clause {}: value cannot occupy a glyph position",
                clause.index()
            ),
            Self::EmptyExpansion => f.write_str("the period contains no relational clause"),
            Self::UnknownSymbol => f.write_str("symbol not found in the interner"),
        }
    }
}

impl core::error::Error for ExpandError {}

// ═══════════════════════════ Expanded model ═══════════════════════════

/// Content of a glyph position after expansion.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ExpandedGlyph<'s> {
    Path(&'s GlyphPath),
    /// `~~`
    SemaLatest,
    /// `~` structural marker after `abdur` (B2).
    Marker,
}

#[derive(Debug)]
pub(crate) struct ExpandedPair<'s> {
    /// Rendered tile (signal tile for the signal pair, case tile otherwise).
    pub tile: String,
    pub glyph: ExpandedGlyph<'s>,
}

#[derive(Debug)]
pub(crate) struct ExpandedClause<'s> {
    pub conjunction: Option<Conjunction>,
    pub colons: u8,
    pub pairs: Vec<ExpandedPair<'s>>,
}

#[derive(Debug)]
pub(crate) struct ExpandedPeriod<'s> {
    pub open: PeriodOpen,
    pub close: PeriodClose,
    pub clauses: Vec<ExpandedClause<'s>>,
}

const fn written_number(number: QuantumNumber) -> Option<ExplicitNumber> {
    match number {
        QuantumNumber::Dual => None,
        QuantumNumber::Singular => Some(ExplicitNumber::Singular),
        QuantumNumber::Plural => Some(ExplicitNumber::Plural),
        QuantumNumber::NonDual => Some(ExplicitNumber::NonDual),
    }
}

const COMITATIVE: Case = Case::Ablative(Some(AblativeDeterminer::Si));

fn case_pair(case: CaseTile, glyph: ExpandedGlyph<'_>) -> ExpandedPair<'_> {
    ExpandedPair {
        tile: printer::case(case),
        glyph,
    }
}

struct Expander<'s> {
    session: &'s Session,
}

impl<'s> Expander<'s> {
    fn path_of(&self, slot: SlotRef) -> Option<&'s GlyphPath> {
        match self.session.glyph_slot(slot)? {
            GlyphSlot::Glyph(path) => Some(path),
            _ => None,
        }
    }

    /// Final values of a value, following variable bindings in chronological order.
    fn flatten(&self, value: SlotValue, clause: ClauseId) -> Result<Vec<SlotValue>, ExpandError> {
        let mut out = Vec::new();
        let mut pending = vec![value];
        let mut steps: u32 = 0;
        while let Some(current) = pending.pop() {
            steps = steps.saturating_add(1);
            if steps > MAX_VALUE_STEPS {
                return Err(ExpandError::InexpressibleValue { clause });
            }
            match current {
                SlotValue::Variable(binding) => {
                    let values = self.binding_values(binding, clause)?;
                    // Reverse push keeps the chronological order on pop.
                    pending.extend(values.iter().rev().copied());
                }
                other => out.push(other),
            }
        }
        Ok(out)
    }

    fn binding_values(
        &self,
        binding: BindingId,
        clause: ClauseId,
    ) -> Result<&'s [SlotValue], ExpandError> {
        Ok(self
            .session
            .binding(binding)
            .ok_or(ExpandError::InexpressibleValue { clause })?
            .values())
    }

    /// Glyph-position content of a final (non-variable) value.
    fn final_glyph(
        &self,
        value: SlotValue,
        clause: ClauseId,
    ) -> Result<ExpandedGlyph<'s>, ExpandError> {
        match value {
            SlotValue::Glyph(slot) => self
                .path_of(slot)
                .map(ExpandedGlyph::Path)
                .ok_or(ExpandError::InexpressibleValue { clause }),
            SlotValue::Sema(_) | SlotValue::Variable(_) => {
                Err(ExpandError::InexpressibleValue { clause })
            }
        }
    }

    /// Final values of a pair (several only for multi-valued variables).
    fn pair_values(
        &self,
        written: &'s GlyphSlot,
        slot: &SlotAnalysis,
        clause: ClauseId,
    ) -> Result<Vec<ExpandedGlyph<'s>>, ExpandError> {
        match slot.resolution() {
            Resolution::Literal => match written {
                GlyphSlot::Glyph(path) => Ok(vec![ExpandedGlyph::Path(path)]),
                GlyphSlot::GlyphAnaphora(_) | GlyphSlot::SemaAnaphora(_) => {
                    Err(ExpandError::InexpressibleValue { clause })
                }
            },
            Resolution::Variable { binding } => self
                .flatten(SlotValue::Variable(binding), clause)?
                .into_iter()
                .map(|value| self.final_glyph(value, clause))
                .collect(),
            Resolution::GlyphAnaphora { value, .. } => self
                .flatten(value, clause)?
                .into_iter()
                .map(|value| self.final_glyph(value, clause))
                .collect(),
            Resolution::SemaAnaphora { .. } => Ok(vec![ExpandedGlyph::SemaLatest]),
        }
    }

    /// Signal pair followed by the case pairs of a relational clause. Every even
    /// slot follows the same rules (a sema anaphora stays `~~` in the signal
    /// glyph slot exactly as in a case slot).
    fn clause(
        &self,
        block: &'s RelationalBlock,
        analysis: &ClauseAnalysis,
        at: GlobalClause,
    ) -> Result<Vec<ExpandedPair<'s>>, ExpandError> {
        let clause = at.clause;
        let mut pairs = Vec::with_capacity(block.pair_count());
        let signal_slot = analysis
            .slot(SlotIndex::SIGNAL)
            .ok_or(ExpandError::InexpressibleValue { clause })?;
        let glyph = match self
            .pair_values(&block.signal.glyph.node, signal_slot, clause)?
            .as_slice()
        {
            [single] => *single,
            _ => return Err(ExpandError::MultiValuedVariable { clause }),
        };
        pairs.push(ExpandedPair {
            tile: printer::signal(&block.signal.tile.node),
            glyph,
        });
        self.case_pairs(block, analysis, clause, &mut pairs)?;
        Ok(pairs)
    }

    /// Expanded case pairs of a relational block, appended to `pairs`.
    fn case_pairs(
        &self,
        block: &'s RelationalBlock,
        analysis: &ClauseAnalysis,
        clause: ClauseId,
        pairs: &mut Vec<ExpandedPair<'s>>,
    ) -> Result<(), ExpandError> {
        for (offset, pair) in block.cases.iter().enumerate() {
            let position = offset
                .checked_add(1)
                .and_then(|p| u32::try_from(p).ok())
                .ok_or(ExpandError::InexpressibleValue { clause })?;
            let Some(slot) = analysis.slot(SlotIndex::new(position)) else {
                // Only the `~` marker after `abdur` has no slot analysis (B2).
                if pair.tile.node.case.is_loop() {
                    pairs.push(case_pair(pair.tile.node, ExpandedGlyph::Marker));
                    continue;
                }
                return Err(ExpandError::InexpressibleValue { clause });
            };
            // Q18 / rule 1B: an anaphora or a variable carries its effective number
            // into the rewritten tile.
            let tile = match slot.resolution() {
                Resolution::GlyphAnaphora { .. } | Resolution::Variable { .. } => CaseTile {
                    case: pair.tile.node.case,
                    number: written_number(slot.number()),
                },
                Resolution::Literal | Resolution::SemaAnaphora { .. } => pair.tile.node,
            };
            let mut values = self
                .pair_values(&pair.glyph.node, slot, clause)?
                .into_iter();
            let first = values
                .next()
                .ok_or(ExpandError::InexpressibleValue { clause })?;
            pairs.push(case_pair(tile, first));
            for value in values {
                pairs.push(case_pair(
                    CaseTile {
                        case: COMITATIVE,
                        number: None,
                    },
                    value,
                ));
            }
        }
        Ok(())
    }
}

/// Builds the expanded model of a period (shared by the lexical renderer and the
/// expanded consultation).
pub(crate) fn expand(session: &Session, id: PeriodId) -> Result<ExpandedPeriod<'_>, ExpandError> {
    let period = session.period(id).ok_or(ExpandError::PeriodNotFound(id))?;
    let analysis = session
        .analysis(id)
        .ok_or(ExpandError::PeriodNotFound(id))?;
    let expander = Expander { session };
    let mut clauses: Vec<ExpandedClause<'_>> = Vec::new();
    let mut previous_depth: Option<u8> = None;

    for (offset, clause_analysis) in analysis.clauses().iter().enumerate() {
        let index = u32::try_from(offset).map_err(|_| ExpandError::EmptyExpansion)?;
        let clause = ClauseId::new(index);
        let Some(ClauseRef::Relational(block)) = period.clause(index) else {
            continue;
        };
        let depth = clause_analysis.depth();
        let conjunction = clause_analysis.conjunction();
        let colons = match previous_depth {
            // The first emitted clause becomes the head: no connector.
            None => {
                if depth != 0 {
                    return Err(ExpandError::InexpressibleDepth { clause });
                }
                0
            }
            Some(previous) => {
                if depth == 0 {
                    // Depth a re-parse would compute for a connector without colons (Q15, Q16).
                    let reparsed = match (conjunction, clause_analysis.role()) {
                        (None, _) => None,
                        (Some(_), DspRole::ForcedRoot) => Some(0),
                        (Some(_), DspRole::ForcedSibling) => Some(previous),
                        (Some(Conjunction::Sed), _) => Some(0),
                        (Some(_), _) => Some(previous),
                    };
                    if reparsed != Some(0) {
                        return Err(ExpandError::InexpressibleDepth { clause });
                    }
                } else if u16::from(depth) > u16::from(previous).saturating_add(1) {
                    return Err(ExpandError::InexpressibleDepth { clause });
                }
                depth
            }
        };
        let at = GlobalClause { period: id, clause };
        let pairs = expander.clause(block, clause_analysis, at)?;
        clauses.push(ExpandedClause {
            conjunction: if previous_depth.is_some() {
                conjunction
            } else {
                None
            },
            colons,
            pairs,
        });
        previous_depth = Some(depth);
    }

    if clauses.is_empty() {
        return Err(ExpandError::EmptyExpansion);
    }
    Ok(ExpandedPeriod {
        open: period.open.node,
        close: period.close.node,
        clauses,
    })
}

/// Renders an expanded period, mapping every glyph path through `glyph`.
pub(crate) fn render_expanded<E>(
    expanded: &ExpandedPeriod<'_>,
    glyph: &mut dyn FnMut(&GlyphPath) -> Result<String, E>,
) -> Result<Vec<Vec<String>>, E> {
    let mut lines = Vec::with_capacity(expanded.clauses.len());
    for clause in &expanded.clauses {
        let mut tokens = Vec::new();
        if let Some(conj) = clause.conjunction {
            tokens.push(conj.surface().to_owned());
        }
        if clause.colons > 0 {
            tokens.push(":".repeat(usize::from(clause.colons)));
        }
        for pair in &clause.pairs {
            tokens.push(pair.tile.as_str().to_owned());
            let text = match pair.glyph {
                ExpandedGlyph::Path(path) => glyph(path)?,
                ExpandedGlyph::SemaLatest => SEMA_ANAPHORA_MARK.to_owned(),
                ExpandedGlyph::Marker => ANAPHORA_MARK.to_string(),
            };
            tokens.push(text);
        }
        lines.push(tokens);
    }
    Ok(lines)
}

fn join_period(open: PeriodOpen, close: PeriodClose, lines: Vec<Vec<String>>) -> String {
    let mut tokens = vec![open.surface().to_owned()];
    tokens.extend(lines.into_iter().flatten());
    tokens.push(close.surface().to_owned());
    tokens.join(" ")
}

/// Expanded canonical form with lexical glyphs (C.1).
pub fn render_expanded_period(session: &Session, id: PeriodId) -> Result<String, ExpandError> {
    let expanded = expand(session, id)?;
    let interner = session.interner();
    let mut lexical = |path: &GlyphPath| {
        render_glyph_path(path, interner).map_err(|_| ExpandError::UnknownSymbol)
    };
    let lines = render_expanded(&expanded, &mut lexical)?;
    Ok(join_period(expanded.open, expanded.close, lines))
}
