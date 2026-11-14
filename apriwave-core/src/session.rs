//! Session: owns the interner, the period arena and the stage 2 state
//! (analyses, `def` variable bindings, antecedents preserved by `/^/`).
//!
//! Cross references use arena indices, never `&`, so there is no
//! self-referential struct. Ingestion is **atomic**: if stage 1 or stage 2
//! emits diagnostics, the session stays identical to how it was before the call.

use crate::prelude::*;

use alloc::collections::BTreeMap;

use crate::analyze::{self, AnalyzedPeriod, Binding, BindingId, Committed, GlobalClause, SlotRef};
use crate::ast::{GlyphSlot, Period, PeriodClose};
use crate::error::{Diagnostic, DiagnosticKind, Limit};
use crate::lexer;
use crate::parser::{Parser, ParserConfig};
use crate::span::Span;
use crate::symbol::{Interner, InternerMark, Symbol};

pub use crate::analyze::PeriodId;

/// Why [`Session::compact`] refused to release the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactError {
    /// The last period was not closed by `^^/`: released state could still be
    /// referenced (anaphora carried by `/^/`, live variables, synchronization).
    NotHardClosed,
}

impl core::fmt::Display for CompactError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotHardClosed => {
                f.write_str("the session can be compacted only after a period closed by `^^/`")
            }
        }
    }
}

impl core::error::Error for CompactError {}

#[derive(Debug, Default)]
pub struct Session {
    interner: Interner,
    periods: Vec<Period>,
    analyses: Vec<AnalyzedPeriod>,
    bindings: Vec<Binding>,
    variables: BTreeMap<Symbol, BindingId>,
    /// Level-0 blocks preserved by `/^/`, in chronological order (R6.3).
    carried: Vec<GlobalClause>,
    /// An antecedent exists for `/^≈` and `/^≃` (Q23, D5, R12).
    sync_antecedent: bool,
    config: ParserConfig,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: ParserConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    // ───────────────────────── Inspection ─────────────────────────

    pub fn interner(&self) -> &Interner {
        &self.interner
    }

    /// Parser and analyzer limits of this session.
    pub const fn config(&self) -> ParserConfig {
        self.config
    }

    pub fn periods(&self) -> &[Period] {
        &self.periods
    }

    pub fn period(&self, id: PeriodId) -> Option<&Period> {
        self.periods.get(id.index() as usize)
    }

    pub fn analysis(&self, id: PeriodId) -> Option<&AnalyzedPeriod> {
        self.analyses.get(id.index() as usize)
    }

    pub fn binding(&self, id: BindingId) -> Option<&Binding> {
        self.bindings.get(id.index() as usize)
    }

    /// Current binding of the variable `name`, if live.
    pub fn live_variable(&self, name: &str) -> Option<(BindingId, &Binding)> {
        let symbol = self.interner.lookup(name)?;
        let id = *self.variables.get(&symbol)?;
        self.binding(id).map(|b| (id, b))
    }

    /// Every live `def` variable, sorted by name (H7: `@live_variables`).
    pub fn live_variables(&self) -> Vec<(&str, BindingId)> {
        let mut live: Vec<(&str, BindingId)> = self
            .variables
            .iter()
            .filter_map(|(symbol, id)| self.interner.resolve(*symbol).map(|name| (name, *id)))
            .collect();
        live.sort_by(|a, b| a.0.cmp(b.0));
        live
    }

    /// Antecedents preserved by `/^/`, most recent first.
    pub fn carried_antecedents(&self) -> impl Iterator<Item = GlobalClause> + '_ {
        self.carried.iter().rev().copied()
    }

    /// `true` if the next period can open with `/^≈` or `/^≃`.
    pub const fn has_sync_antecedent(&self) -> bool {
        self.sync_antecedent
    }

    /// Syntactic content of a slot.
    pub fn glyph_slot(&self, slot: SlotRef) -> Option<&GlyphSlot> {
        let period = self.period(slot.clause.period)?;
        analyze::slot_in_period(period, slot.clause.clause, slot.slot)
    }

    // ───────────────────────── Memory release ─────────────────────────

    /// Releases every period, analysis, binding and interned text, keeping the
    /// configuration, for long-running devices with little RAM.
    ///
    /// Allowed only when the session is empty or its last period was closed by
    /// `^^/` (Hard Close): such a close already drops the anaphora, the `def`
    /// variables and the synchronization antecedent (§3), so nothing released
    /// can influence a later period and future ingestion behaves exactly as on
    /// the uncompacted session. Identifiers obtained before the call
    /// (`PeriodId`, `BindingId`, `Symbol`) become invalid and may be issued again.
    pub fn compact(&mut self) -> Result<(), CompactError> {
        let closed = self
            .periods
            .last()
            .is_none_or(|period| period.close.node == PeriodClose::Hard);
        if !closed {
            return Err(CompactError::NotHardClosed);
        }
        *self = Self::with_config(self.config);
        Ok(())
    }

    // ───────────────────────── Ingestion ─────────────────────────

    /// Analyzes `src` (one or more periods separated by a single space).
    ///
    /// `Ok` with the identifiers of the added periods; `Err` with **all** the
    /// collected diagnostics, sorted by position (Q25). Stage 2 runs
    /// only if stage 1 produced no diagnostics.
    pub fn ingest(&mut self, src: &str) -> Result<Vec<PeriodId>, Vec<Diagnostic>> {
        self.ingest_with_undo(src).map(|(ids, _undo)| ids)
    }

    /// Like [`Session::ingest`], then runs `check` on the committed session; if
    /// `check` fails, every change made by this call is undone (B5: the Legacy
    /// Shield must leave its session untouched when it blocks an input).
    pub fn ingest_checked<E>(
        &mut self,
        src: &str,
        check: impl FnOnce(&Session, &[PeriodId]) -> Result<(), Vec<E>>,
    ) -> Result<Vec<PeriodId>, CheckedIngestError<E>> {
        let (ids, undo) = self
            .ingest_with_undo(src)
            .map_err(CheckedIngestError::Grammar)?;
        match check(self, &ids) {
            Ok(()) => Ok(ids),
            Err(errors) => {
                self.rollback(undo);
                Err(CheckedIngestError::Check(errors))
            }
        }
    }

    fn rollback(&mut self, undo: Undo) {
        self.periods.truncate(undo.periods);
        self.analyses.truncate(undo.analyses);
        self.bindings.truncate(undo.bindings);
        match undo.variables {
            VariablesUndo::Replaced(previous) => self.variables = previous,
            VariablesUndo::Changes(changes) => {
                for (name, previous) in changes.into_iter().rev() {
                    match previous {
                        Some(binding) => {
                            self.variables.insert(name, binding);
                        }
                        None => {
                            self.variables.remove(&name);
                        }
                    }
                }
            }
        }
        match undo.carried {
            CarriedUndo::Replaced(previous) => self.carried = previous,
            CarriedUndo::Truncate(length) => self.carried.truncate(length),
        }
        self.sync_antecedent = undo.sync_antecedent;
        self.interner.rollback(undo.interner);
    }

    fn ingest_with_undo(&mut self, src: &str) -> Result<(Vec<PeriodId>, Undo), Vec<Diagnostic>> {
        let lexed = lexer::lex(src).map_err(|d| vec![d])?;
        let mark = self.interner.mark();

        let mut parser = Parser::new(&lexed.tokens, &mut self.interner, self.config, lexed.end);
        let periods = parser.parse_session(self.sync_antecedent);
        let mut diagnostics = lexed.diagnostics;
        diagnostics.extend(parser.into_diagnostics());

        // Identifiers computed before any mutation (atomicity).
        let base = self.periods.len();
        let ids: Option<Vec<PeriodId>> = (0..periods.len())
            .map(|offset| {
                base.checked_add(offset)
                    .and_then(|n| u32::try_from(n).ok())
                    .map(PeriodId)
            })
            .collect();
        let ids = match ids {
            Some(ids) => ids,
            None => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::PeriodCount,
                    },
                    Span::new(0, 0),
                ));
                Vec::new()
            }
        };

        let mut outcome = None;
        if diagnostics.is_empty() {
            let Some(first) = ids.first().copied() else {
                // Stage 1 guarantees at least one period when there are no diagnostics.
                return Ok((Vec::new(), self.undo_point(mark)));
            };
            let committed = Committed {
                periods: &self.periods,
                analyses: &self.analyses,
                bindings: &self.bindings,
                variables: &self.variables,
                carried: &self.carried,
            };
            match analyze::analyze(committed, &periods, first, self.config) {
                Ok(result) => outcome = Some(result),
                Err(stage2) => diagnostics.extend(stage2),
            }
        }

        let Some(outcome) = outcome else {
            self.interner.rollback(mark);
            diagnostics.sort_by_key(|d| (d.span.start, d.span.end));
            return Err(diagnostics);
        };

        // ── Atomic commit (with an undo record) ──
        let mut undo = self.undo_point(mark);
        if let Some(last) = periods.last() {
            self.sync_antecedent = last.close.node != PeriodClose::Hard;
        }
        self.periods.extend(periods);
        self.analyses.extend(outcome.analyses);
        self.bindings.extend(outcome.bindings);
        let mut changes = Vec::with_capacity(outcome.overlay.len());
        if outcome.variables_cleared {
            undo.variables = VariablesUndo::Replaced(core::mem::take(&mut self.variables));
        }
        for (name, entry) in outcome.overlay {
            let previous = match entry {
                Some(binding) => self.variables.insert(name, binding),
                None => self.variables.remove(&name),
            };
            changes.push((name, previous));
        }
        if !outcome.variables_cleared {
            undo.variables = VariablesUndo::Changes(changes);
        }
        if !outcome.keep_base_carried {
            undo.carried = CarriedUndo::Replaced(core::mem::take(&mut self.carried));
        }
        self.carried.extend(outcome.carried_tail);
        Ok((ids, undo))
    }

    fn undo_point(&self, interner: InternerMark) -> Undo {
        Undo {
            interner,
            periods: self.periods.len(),
            analyses: self.analyses.len(),
            bindings: self.bindings.len(),
            variables: VariablesUndo::Changes(Vec::new()),
            carried: CarriedUndo::Truncate(self.carried.len()),
            sync_antecedent: self.sync_antecedent,
        }
    }
}

/// Failure of [`Session::ingest_checked`].
#[derive(Debug, PartialEq, Eq)]
pub enum CheckedIngestError<E> {
    /// Stage 1 / stage 2 diagnostics (nothing was committed).
    Grammar(Vec<Diagnostic>),
    /// The additional check failed (everything was undone).
    Check(Vec<E>),
}

/// State needed to undo one committed ingestion (no clones: previous containers
/// are moved out, or only lengths and previous map entries are recorded).
struct Undo {
    interner: InternerMark,
    periods: usize,
    analyses: usize,
    bindings: usize,
    variables: VariablesUndo,
    carried: CarriedUndo,
    sync_antecedent: bool,
}

enum VariablesUndo {
    /// Previous value of every name touched by the overlay.
    Changes(Vec<(Symbol, Option<BindingId>)>),
    /// The whole previous table (after a Hard close).
    Replaced(BTreeMap<Symbol, BindingId>),
}

enum CarriedUndo {
    Truncate(usize),
    Replaced(Vec<GlobalClause>),
}
