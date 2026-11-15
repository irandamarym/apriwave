//! Stage 2: contextual analysis.
//!
//! - DSP tree (Q14–Q16), anchors and relative mode (Q6, R1–R3).
//! - Full inheritance of the omitted determiners (v4.1) and Q22.
//! - Glyph and sema anaphora (`~`, `~~`), with identical rules for every
//!   `[Tile] [Glyph]` pair: no pair has privileged status.
//! - Lifecycle of `def` variables (Q11, R11, R13, R14).
//! - Siblings in written order (§9.2) and 3-level Garbage Collector.
//!
//! All references are arena indices (`PeriodId`, `ClauseId`, `BindingId`): no
//! `Rc`, no `RefCell`, no copy of the antecedent sema.
//! The analysis works on a **draft** state (variable overlay, antecedent queue,
//! new bindings) which the session applies only if there are no diagnostics.

use crate::prelude::*;

use alloc::collections::BTreeMap;
use core::num::{NonZeroU32, NonZeroU8};

use crate::parser::ParserConfig;

use crate::ast::{
    CaseTile, ClauseBlock, ClauseRef, ColonRun, Comparative, Conjunction, DefOperation,
    DefinitionBlock, ExplicitNumber, GlyphAnaphora, GlyphPath, GlyphSlot, Link, LinkedClause,
    Period, PeriodClose, PortabilityBlock, PrimarySignal, Quantifier, QuantumNumber,
    RelationalBlock, RelationalSignal, SemaAnaphora, Temporal, TemporalMode,
};
use crate::error::{Diagnostic, DiagnosticKind, Limit};
use crate::span::{Span, Spanned};
use crate::symbol::Symbol;

// ═══════════════════════════ Arena identifiers ═══════════════════════════

/// Index of a period in the session arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeriodId(pub(crate) u32);

impl PeriodId {
    pub const fn index(self) -> u32 {
        self.0
    }

    const fn slot(self) -> usize {
        self.0 as usize
    }
}

/// Index of a block within the period: `0` = `head`, `i + 1` = `tail[i]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClauseId(u32);

impl ClauseId {
    pub const HEAD: Self = Self(0);

    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }

    const fn slot(self) -> usize {
        self.0 as usize
    }
}

/// Block identified at session level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlobalClause {
    pub period: PeriodId,
    pub clause: ClauseId,
}

/// Position of a `[Tile] [Glyph]` pair within the block: `0` = signal pair,
/// `1…n` = subsequent pairs in written order. Identical for every block type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotIndex(u32);

impl SlotIndex {
    /// Pair of the signal tile.
    pub const SIGNAL: Self = Self(0);

    pub const fn new(position: u32) -> Self {
        Self(position)
    }

    pub const fn position(self) -> u32 {
        self.0
    }

    const fn slot(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotRef {
    pub clause: GlobalClause,
    pub slot: SlotIndex,
}

/// Immutable version of a `def` variable (every mutation creates a new binding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindingId(u32);

impl BindingId {
    pub const fn index(self) -> u32 {
        self.0
    }

    const fn slot(self) -> usize {
        self.0 as usize
    }
}

fn to_u32(value: usize) -> Option<u32> {
    u32::try_from(value).ok()
}

/// Syntactic content of pair `slot` of block `clause` in `period`.
/// The name of a `def` is a symbol (Q11) and the `~` of `abse ~` is a marker (R14):
/// neither is a `GlyphSlot`.
pub(crate) fn slot_in_period(
    period: &Period,
    clause: ClauseId,
    slot: SlotIndex,
) -> Option<&GlyphSlot> {
    match period.clause(clause.0)? {
        ClauseRef::Relational(block) => block.glyph(slot.0).map(|g| &g.node),
        ClauseRef::Definition(block) => {
            let operation = slot.slot().checked_sub(1)?;
            match block.operations.iter().nth(operation)? {
                DefOperation::Assign { value, .. }
                | DefOperation::Modify { value, .. }
                | DefOperation::Union { value, .. } => Some(&value.node),
                DefOperation::Unbind { .. } => None,
            }
        }
        ClauseRef::Portability(_) => None,
    }
}

// ═══════════════════════════ Analysis results ═══════════════════════════

/// Final value of a slot after following anaphora chains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotValue {
    /// Literal glyph present in the given slot.
    Glyph(SlotRef),
    /// `def` variable (version in effect at resolution time).
    Variable(BindingId),
    /// Complete sema of the given block.
    Sema(GlobalClause),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    /// Literal glyph.
    Literal,
    /// R13: glyph token equal to the name of a live variable.
    Variable { binding: BindingId },
    /// `~`: `antecedent` is the attached slot, `value` the final value after
    /// following any chain (R9.1).
    GlyphAnaphora {
        antecedent: SlotRef,
        value: SlotValue,
    },
    /// `~~`.
    SemaAnaphora { clause: GlobalClause },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotAnalysis {
    resolution: Resolution,
    value: SlotValue,
    number: QuantumNumber,
}

impl SlotAnalysis {
    pub const fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub const fn value(&self) -> SlotValue {
        self.value
    }

    /// Effective quantum number of the pair (rule 1B, v4.1): the one written in
    /// the tile (signal or case tile). When omitted, the signal pair takes the
    /// number of the parent's signal (full inheritance), Dual at the root; a case
    /// pair takes the number of the antecedent (`~`, Q18) or the variable's own
    /// number (`def`, R9.4), Dual otherwise.
    pub const fn number(&self) -> QuantumNumber {
        self.number
    }

    const fn placeholder(slot: SlotRef) -> Self {
        Self {
            resolution: Resolution::Literal,
            value: SlotValue::Glyph(slot),
            number: QuantumNumber::Dual,
        }
    }
}

/// Role of the block in the DSP stack (Q16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DspRole {
    Normal,
    /// `def`: does not push nodes onto the stack.
    Transparent,
    /// `oh`: without explicit `:` it is placed as a sibling of the last active block.
    ForcedSibling,
    /// `deep`: without explicit `:` it opens a new tree at level 0.
    ForcedRoot,
    /// Portability tile: operates on the state but is not a node (R2).
    Portability,
}

/// Effective determiners after inheritance (v4.1, full inheritance): a
/// subordinate clause with a primary or secondary signal inherits from its parent
/// every determiner it omits — quantifier, `ref`, comparative, tense or
/// conditional, number — unless it writes its own. `deep` inherits nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectiveSignal {
    pub quantifier: Option<Quantifier>,
    pub quantifier_inherited: bool,
    pub reflexive: bool,
    pub reflexive_inherited: bool,
    pub comparative: Option<Comparative>,
    pub comparative_inherited: bool,
    /// `None` = no tense written or inherited (Present by default, M§2.4).
    pub tense: Option<TemporalMode>,
    pub tense_inherited: bool,
    /// Number of the signal pair (`None` = Dual).
    pub number: Option<ExplicitNumber>,
    pub number_inherited: bool,
}

impl EffectiveSignal {
    /// Effective tense with the M§2.4 default (`ato`).
    pub fn tense_or_present(&self) -> TemporalMode {
        self.tense
            .unwrap_or(TemporalMode::Indicative(Temporal::Ato))
    }

    fn compute(signal: &RelationalSignal, parent: Option<EffectiveSignal>) -> Self {
        let own = signal.determiners();
        // Q17: only blocks with a primary or secondary signal inherit.
        let parent = match signal {
            RelationalSignal::Deep { .. } => None,
            RelationalSignal::Primary { .. } | RelationalSignal::Secondary { .. } => parent,
        };
        let (quantifier, quantifier_inherited) =
            match (own.quantifier, parent.and_then(|p| p.quantifier)) {
                (Some(q), _) => (Some(q), false),
                (None, Some(q)) => (Some(q), true),
                (None, None) => (None, false),
            };
        let (reflexive, reflexive_inherited) =
            match (own.reflexive.is_some(), parent.is_some_and(|p| p.reflexive)) {
                (true, _) => (true, false),
                (false, true) => (true, true),
                (false, false) => (false, false),
            };
        let (tense, tense_inherited) = inherit(own.tense, parent.and_then(|p| p.tense));
        let (comparative, comparative_inherited) =
            inherit(own.comparative, parent.and_then(|p| p.comparative));
        let (number, number_inherited) = inherit(own.number, parent.and_then(|p| p.number));
        Self {
            quantifier,
            quantifier_inherited,
            reflexive,
            reflexive_inherited,
            comparative,
            comparative_inherited,
            tense,
            tense_inherited,
            number,
            number_inherited,
        }
    }
}

/// Written value if present, otherwise the inherited one (with the flag).
fn inherit<T>(own: Option<T>, parent: Option<T>) -> (Option<T>, bool) {
    match (own, parent) {
        (Some(value), _) => (Some(value), false),
        (None, Some(value)) => (Some(value), true),
        (None, None) => (None, false),
    }
}

/// Analysis of a single block (parallel to the period's blocks).
#[derive(Debug, PartialEq, Eq)]
pub struct ClauseAnalysis {
    role: DspRole,
    depth: u8,
    parent: Option<ClauseId>,
    children: Vec<ClauseId>,
    conjunction: Option<Conjunction>,
    signal: Option<EffectiveSignal>,
    canonical_order: Box<[u32]>,
    slots: Box<[Option<SlotAnalysis>]>,
}

impl ClauseAnalysis {
    pub const fn role(&self) -> DspRole {
        self.role
    }

    /// DSP depth (for `def`: insertion depth; for portability:
    /// effective depth of the `:`).
    pub const fn depth(&self) -> u8 {
        self.depth
    }

    pub const fn parent(&self) -> Option<ClauseId> {
        self.parent
    }

    pub fn children(&self) -> &[ClauseId] {
        &self.children
    }

    /// Conjunction of the incoming connector, if present (§9.2: the siblings keep
    /// the written order; the grammar presumes no boolean grouping).
    pub const fn conjunction(&self) -> Option<Conjunction> {
        self.conjunction
    }

    pub const fn signal(&self) -> Option<&EffectiveSignal> {
        self.signal.as_ref()
    }

    /// Canonical order of the pairs (M§3.1): the signal pair (position 0),
    /// then the case pairs in order `n → a → d → g → ab` (stable sort).
    /// Empty for non-relational blocks.
    pub fn canonical_order(&self) -> &[u32] {
        &self.canonical_order
    }

    /// Analysis of the pair at position `slot` (`None` if the pair does not contain a
    /// resolvable glyph: `def` name, `abse ~`, portability operand).
    pub fn slot(&self, slot: SlotIndex) -> Option<&SlotAnalysis> {
        self.slots.get(slot.slot()).and_then(Option::as_ref)
    }

    /// Analysis of all pairs, indexed by position.
    pub fn slots(&self) -> &[Option<SlotAnalysis>] {
        &self.slots
    }

    fn non_relational(
        role: DspRole,
        depth: u8,
        conjunction: Option<Conjunction>,
        slots: Vec<Option<SlotAnalysis>>,
    ) -> Self {
        Self {
            role,
            depth,
            parent: None,
            children: Vec::new(),
            conjunction,
            signal: None,
            canonical_order: Box::new([]),
            slots: slots.into_boxed_slice(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnalyzedPeriod {
    clauses: Box<[ClauseAnalysis]>,
    roots: Box<[ClauseId]>,
    warnings: Box<[Diagnostic]>,
}

impl AnalyzedPeriod {
    pub fn clause(&self, id: ClauseId) -> Option<&ClauseAnalysis> {
        self.clauses.get(id.slot())
    }

    pub fn clauses(&self) -> &[ClauseAnalysis] {
        &self.clauses
    }

    /// Level-0 relational blocks, in order.
    pub fn roots(&self) -> &[ClauseId] {
        &self.roots
    }

    /// Diagnostics of this period downgraded to warnings by the configuration
    /// (§15): the period was accepted despite them.
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }

    pub(crate) fn set_warnings(&mut self, warnings: Vec<Diagnostic>) {
        self.warnings = warnings.into_boxed_slice();
    }
}

/// Version of a `def` variable.
#[derive(Debug, PartialEq, Eq)]
pub struct Binding {
    name: Symbol,
    number: Option<ExplicitNumber>,
    values: Box<[SlotValue]>,
    defined_at: GlobalClause,
}

impl Binding {
    pub const fn name(&self) -> Symbol {
        self.name
    }

    /// Number written in the `def` signal (`None` = `def`, Dual).
    pub const fn written_number(&self) -> Option<ExplicitNumber> {
        self.number
    }

    pub fn quantum_number(&self) -> QuantumNumber {
        QuantumNumber::from(self.number)
    }

    /// Values in chronological order (multiple values after `absi`).
    pub fn values(&self) -> &[SlotValue] {
        &self.values
    }

    pub const fn defined_at(&self) -> GlobalClause {
        self.defined_at
    }
}

// ═══════════════════════════ Engine input / output ═══════════════════════════

/// Already committed session state (read-only during analysis).
pub(crate) struct Committed<'a> {
    pub analyses: &'a [AnalyzedPeriod],
    pub bindings: &'a [Binding],
    pub variables: &'a BTreeMap<Symbol, BindingId>,
    /// Level-0 blocks retained by `/^/`, in chronological order.
    pub carried: &'a [GlobalClause],
}

/// Changes to apply atomically to the session.
pub(crate) struct Outcome {
    pub analyses: Vec<AnalyzedPeriod>,
    pub bindings: Vec<Binding>,
    pub overlay: BTreeMap<Symbol, Option<BindingId>>,
    pub variables_cleared: bool,
    pub carried_tail: Vec<GlobalClause>,
    pub keep_base_carried: bool,
    /// Diagnostics downgraded to warnings by the configuration (§15).
    pub warnings: Vec<Diagnostic>,
}

pub(crate) fn analyze<'a>(
    committed: Committed<'a>,
    new_periods: &'a [Period],
    first: PeriodId,
    config: ParserConfig,
) -> Result<Outcome, Vec<Diagnostic>> {
    let mut analyzer = Analyzer {
        committed,
        first,
        max_depth: config.max_dsp_depth,
        max_loop_iterations: config.max_loop_iterations,
        analyses: Vec::with_capacity(new_periods.len()),
        bindings: Vec::new(),
        overlay: BTreeMap::new(),
        variables_cleared: false,
        carried_tail: Vec::new(),
        keep_base_carried: true,
        diagnostics: Vec::new(),
    };

    for (offset, period) in new_periods.iter().enumerate() {
        let Some(id) = to_u32(offset)
            .and_then(|k| first.0.checked_add(k))
            .map(PeriodId)
        else {
            analyzer.report(
                DiagnosticKind::LimitExceeded {
                    limit: Limit::PeriodCount,
                },
                period.open.span,
            );
            break;
        };
        let analysis = analyzer.period(id, period);
        analyzer.analyses.push(analysis);
    }

    // §15: the input is rejected only by blocking diagnostics; the others are
    // kept as warnings.
    let graded: Vec<Diagnostic> = analyzer
        .diagnostics
        .iter()
        .map(|d| d.graded(config.warnings))
        .collect();
    if graded.iter().any(|d| !d.is_warning()) {
        return Err(graded);
    }
    Ok(Outcome {
        warnings: graded,
        analyses: analyzer.analyses,
        bindings: analyzer.bindings,
        overlay: analyzer.overlay,
        variables_cleared: analyzer.variables_cleared,
        carried_tail: analyzer.carried_tail,
        keep_base_carried: analyzer.keep_base_carried,
    })
}

// ═══════════════════════════ Internal state ═══════════════════════════

#[derive(Clone, Copy)]
enum LinkInfo {
    Head,
    Colons(u8),
    Conj(Conjunction),
    ConjColons(Conjunction, u8),
}

impl LinkInfo {
    fn from_link(link: Link) -> Self {
        match link {
            Link::Subordination(run) => Self::Colons(run.count()),
            Link::Coordination { conj, colons: None } => Self::Conj(conj.node),
            Link::Coordination {
                conj,
                colons: Some(run),
            } => Self::ConjColons(conj.node, run.node.count()),
        }
    }

    const fn conjunction(self) -> Option<Conjunction> {
        match self {
            Self::Conj(c) | Self::ConjColons(c, _) => Some(c),
            Self::Head | Self::Colons(_) => None,
        }
    }
}

/// Slot resolution request.
#[derive(Clone, Copy)]
struct SlotQuery {
    /// Block depth: the ancestors are `stack[..depth]`.
    depth: usize,
    slot: SlotRef,
    /// Number written in the pair's tile (`None` if omitted).
    written_number: Option<ExplicitNumber>,
    /// Rule 1B / Q18: whether an omitted number is inherited from the anaphora
    /// antecedent or from the variable (`false` for the signal pair: Dual).
    inherit_number: bool,
    /// R13: variable name resolution (only in relational blocks).
    variables: bool,
}

#[derive(Clone, Copy)]
struct Candidate {
    slot: SlotRef,
    value: SlotValue,
    number: QuantumNumber,
}

struct AnchorFrame<'a> {
    id: &'a GlyphPath,
    depth: u8,
    span: Span,
}

struct PeriodState<'a> {
    id: PeriodId,
    clauses: Vec<ClauseAnalysis>,
    /// `stack[d]` = active relational node at depth `d`.
    stack: Vec<ClauseId>,
    anchors: Vec<AnchorFrame<'a>>,
    rel_mode: bool,
    /// Level-0 relational blocks, in order (R6.2).
    roots: Vec<ClauseId>,
}

struct Analyzer<'a> {
    committed: Committed<'a>,
    first: PeriodId,
    max_depth: NonZeroU8,
    max_loop_iterations: NonZeroU32,
    analyses: Vec<AnalyzedPeriod>,
    bindings: Vec<Binding>,
    overlay: BTreeMap<Symbol, Option<BindingId>>,
    variables_cleared: bool,
    carried_tail: Vec<GlobalClause>,
    keep_base_carried: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Analyzer<'a> {
    fn report(&mut self, kind: DiagnosticKind, span: Span) {
        self.diagnostics.push(Diagnostic::new(kind, span));
    }

    // ───────────────────────── Arena access ─────────────────────────

    fn analysis_of<'s>(
        &'s self,
        st: &'s PeriodState<'a>,
        clause: GlobalClause,
    ) -> Option<&'s ClauseAnalysis> {
        if clause.period == st.id {
            return st.clauses.get(clause.clause.slot());
        }
        let period = match clause.period.0.checked_sub(self.first.0) {
            Some(offset) => self.analyses.get(offset as usize),
            None => self.committed.analyses.get(clause.period.slot()),
        }?;
        period.clause(clause.clause)
    }

    fn binding(&self, id: BindingId) -> Option<&Binding> {
        match id.slot().checked_sub(self.committed.bindings.len()) {
            Some(offset) => self.bindings.get(offset),
            None => self.committed.bindings.get(id.slot()),
        }
    }

    fn live(&self, name: Symbol) -> Option<BindingId> {
        if let Some(entry) = self.overlay.get(&name) {
            return *entry;
        }
        if self.variables_cleared {
            return None;
        }
        self.committed.variables.get(&name).copied()
    }

    // ───────────────────────── Antecedents (R6) ─────────────────────────

    /// Search order: active ancestors (from parent to root), then the period's
    /// preceding level-0 blocks (most recent first), then those retained by `/^/`.
    fn antecedents<'s>(
        &'s self,
        st: &'s PeriodState<'a>,
        depth: usize,
    ) -> impl Iterator<Item = GlobalClause> + 's {
        let id = st.id;
        let root = if depth > 0 {
            st.stack.first().copied()
        } else {
            None
        };
        let keep_base = self.keep_base_carried;
        let ancestors = st
            .stack
            .iter()
            .take(depth)
            .rev()
            .map(move |c| GlobalClause {
                period: id,
                clause: *c,
            });
        let prior_roots = st
            .roots
            .iter()
            .rev()
            .filter(move |c| Some(**c) != root)
            .map(move |c| GlobalClause {
                period: id,
                clause: *c,
            });
        let carried_tail = self.carried_tail.iter().rev().copied();
        let carried_base = self
            .committed
            .carried
            .iter()
            .rev()
            .copied()
            .filter(move |_| keep_base);
        ancestors
            .chain(prior_roots)
            .chain(carried_tail)
            .chain(carried_base)
    }

    /// Candidates of an antecedent block for a glyph anaphora: **all** pairs,
    /// in reverse canonical order (the last canonical pair first). The rule is
    /// the same regardless of the pair in which the anaphora appears.
    fn candidates(&self, st: &PeriodState<'a>, clause: GlobalClause) -> Vec<Candidate> {
        let Some(analysis) = self.analysis_of(st, clause) else {
            return Vec::new();
        };
        analysis
            .canonical_order
            .iter()
            .rev()
            .filter_map(|position| {
                let slot = analysis.slots.get(*position as usize)?.as_ref()?;
                // Effective number of the antecedent pair (R9.4 and rule 1B are
                // already applied when the pair was resolved).
                let number = slot.number;
                Some(Candidate {
                    slot: SlotRef {
                        clause,
                        slot: SlotIndex(*position),
                    },
                    value: slot.value,
                    number,
                })
            })
            .collect()
    }

    /// `~`: the last pair, in canonical order, of the nearest antecedent block
    /// that holds a value other than a sema (R9.2: sema anaphora are skipped).
    fn search_glyph(&self, st: &PeriodState<'a>, depth: usize) -> Option<Candidate> {
        self.antecedents(st, depth).find_map(|clause| {
            self.candidates(st, clause)
                .into_iter()
                .find(|c| !matches!(c.value, SlotValue::Sema(_)))
        })
    }

    /// `~~`: the first antecedent block of the search order.
    fn search_sema(&self, st: &PeriodState<'a>, depth: usize) -> Option<GlobalClause> {
        self.antecedents(st, depth).next()
    }

    fn resolve(
        &self,
        st: &PeriodState<'a>,
        query: SlotQuery,
        glyph: &GlyphSlot,
    ) -> Result<SlotAnalysis, DiagnosticKind> {
        let written = query.written_number.map(QuantumNumber::from);
        let default_number = written.unwrap_or(QuantumNumber::Dual);
        match glyph {
            GlyphSlot::Glyph(path) => {
                // R13: the whole token is compared with the variable names.
                if query.variables {
                    if let Some(binding) = self.live(path.text) {
                        // R9.4 + rule 1B: in a case pair an omitted number is the
                        // variable's own; in the signal pair it stays Dual.
                        let inherited = self
                            .binding(binding)
                            .filter(|_| query.inherit_number)
                            .map(Binding::quantum_number);
                        return Ok(SlotAnalysis {
                            resolution: Resolution::Variable { binding },
                            value: SlotValue::Variable(binding),
                            number: written.or(inherited).unwrap_or(QuantumNumber::Dual),
                        });
                    }
                }
                Ok(SlotAnalysis {
                    resolution: Resolution::Literal,
                    value: SlotValue::Glyph(query.slot),
                    number: default_number,
                })
            }
            GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact) => {
                let found = self
                    .search_glyph(st, query.depth)
                    .ok_or(DiagnosticKind::UnresolvedGlyphAnaphora)?;
                // Q18: the number written in the tile takes precedence, otherwise it is
                // inherited (the signal pair takes its number from the signal tile).
                let number = written.unwrap_or(if query.inherit_number {
                    found.number
                } else {
                    QuantumNumber::Dual
                });
                Ok(SlotAnalysis {
                    resolution: Resolution::GlyphAnaphora {
                        antecedent: found.slot,
                        value: found.value,
                    },
                    value: found.value,
                    number,
                })
            }
            GlyphSlot::SemaAnaphora(SemaAnaphora::Latest) => {
                let clause = self
                    .search_sema(st, query.depth)
                    .ok_or(DiagnosticKind::UnresolvedSemaAnaphora)?;
                Ok(SlotAnalysis {
                    resolution: Resolution::SemaAnaphora { clause },
                    value: SlotValue::Sema(clause),
                    number: default_number,
                })
            }
        }
    }

    fn resolve_reporting(
        &mut self,
        st: &PeriodState<'a>,
        query: SlotQuery,
        glyph: &Spanned<GlyphSlot>,
    ) -> SlotAnalysis {
        match self.resolve(st, query, &glyph.node) {
            Ok(analysis) => analysis,
            Err(kind) => {
                self.report(kind, glyph.span);
                SlotAnalysis::placeholder(query.slot)
            }
        }
    }

    // ───────────────────────── DSP depth ─────────────────────────

    fn current_depth(&mut self, st: &PeriodState<'a>, span: Span) -> u8 {
        match u8::try_from(st.stack.len().saturating_sub(1)) {
            Ok(depth) => depth,
            Err(_) => {
                self.report(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::DspDepth,
                    },
                    span,
                );
                u8::MAX
            }
        }
    }

    /// Depth indicated by `count` colons, with relative offset (Q6.6, R1).
    fn colon_depth(&mut self, st: &PeriodState<'a>, count: u8, span: Span) -> Option<u8> {
        let effective = match st.anchors.last() {
            Some(anchor) if st.rel_mode => anchor.depth.checked_add(count.saturating_sub(1)),
            _ => Some(count),
        };
        match effective.filter(|d| *d <= self.max_depth.get()) {
            Some(depth) => Some(depth),
            None => {
                self.report(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::DspDepth,
                    },
                    span,
                );
                None
            }
        }
    }

    /// Q14: at most one level beyond the current depth. On error the
    /// depth is clamped so that diagnostic collection can continue.
    fn check_skip(&mut self, st: &PeriodState<'a>, requested: u8, span: Span) -> u8 {
        let max_allowed = st.stack.len();
        if usize::from(requested) <= max_allowed {
            return requested;
        }
        let clamped = u8::try_from(max_allowed).unwrap_or(requested);
        self.report(
            DiagnosticKind::DspLevelSkip {
                max_allowed: clamped,
                requested,
            },
            span,
        );
        clamped
    }

    /// Depth of a relational block (Q15, Q16).
    fn place(&mut self, st: &PeriodState<'a>, link: LinkInfo, role: DspRole, span: Span) -> u8 {
        let current = self.current_depth(st, span);
        let requested = match (link, role) {
            (LinkInfo::Head, _)
            | (LinkInfo::Conj(_), DspRole::ForcedRoot)
            | (LinkInfo::Conj(Conjunction::Sed), DspRole::Normal) => Some(0),
            (LinkInfo::Conj(_), _) => Some(current),
            (LinkInfo::Colons(count) | LinkInfo::ConjColons(_, count), _) => {
                self.colon_depth(st, count, span)
            }
        };
        match requested {
            Some(depth) => self.check_skip(st, depth, span),
            None => current,
        }
    }

    // ───────────────────────── Period ─────────────────────────

    fn period(&mut self, id: PeriodId, period: &'a Period) -> AnalyzedPeriod {
        let mut st = PeriodState {
            id,
            clauses: Vec::with_capacity(period.clause_count()),
            stack: Vec::new(),
            anchors: Vec::new(),
            rel_mode: false,
            roots: Vec::new(),
        };

        self.clause(&mut st, ClauseId::HEAD, LinkInfo::Head, &period.head);
        for (offset, linked) in period.tail.iter().enumerate() {
            let Some(clause) = offset.checked_add(1).and_then(to_u32).map(ClauseId) else {
                self.report(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::IndexSpace,
                    },
                    period.open.span,
                );
                break;
            };
            match linked {
                LinkedClause::Clause { link, block } => {
                    self.clause(&mut st, clause, LinkInfo::from_link(link.node), block);
                }
                LinkedClause::Portability { colons, block } => {
                    self.portability(&mut st, *colons, block);
                }
            }
        }

        // R3: the anchor stack is local to the period.
        for anchor in &st.anchors {
            self.diagnostics
                .push(Diagnostic::new(DiagnosticKind::UnclosedAnchor, anchor.span));
        }

        // 3-level Garbage Collector (Q, R6.3, R12).
        match period.close.node {
            PeriodClose::Soft => {
                self.carried_tail
                    .extend(st.roots.iter().map(|c| GlobalClause {
                        period: id,
                        clause: *c,
                    }));
            }
            PeriodClose::Standard => self.forget_anaphora(),
            PeriodClose::Hard => {
                self.forget_anaphora();
                self.overlay.clear();
                self.variables_cleared = true;
            }
        }

        self.finish_period(st, period.open.span)
    }

    fn forget_anaphora(&mut self) {
        self.carried_tail.clear();
        self.keep_base_carried = false;
    }

    fn clause(
        &mut self,
        st: &mut PeriodState<'a>,
        clause: ClauseId,
        link: LinkInfo,
        block: &'a ClauseBlock,
    ) {
        match block {
            ClauseBlock::Relational(b) => self.relational(st, clause, link, b),
            ClauseBlock::Definition(b) => self.definition(st, clause, link, b),
        }
    }

    fn relational(
        &mut self,
        st: &mut PeriodState<'a>,
        clause: ClauseId,
        link: LinkInfo,
        block: &'a RelationalBlock,
    ) {
        let signal = &block.signal.tile.node;
        let signal_span = block.signal.tile.span;
        let role = match signal {
            RelationalSignal::Deep { .. } => DspRole::ForcedRoot,
            RelationalSignal::Primary {
                signal: PrimarySignal::Oh,
                ..
            }
            | RelationalSignal::Secondary {
                primary: PrimarySignal::Oh,
                ..
            } => DspRole::ForcedSibling,
            RelationalSignal::Primary { .. } | RelationalSignal::Secondary { .. } => {
                DspRole::Normal
            }
        };

        let depth = self.place(st, link, role, signal_span);
        st.stack.truncate(usize::from(depth));
        let parent = if depth == 0 {
            None
        } else {
            st.stack.last().copied()
        };
        let parent_signal = parent
            .and_then(|p| st.clauses.get(p.slot()))
            .and_then(|a| a.signal);
        let effective = EffectiveSignal::compute(signal, parent_signal);

        // Q22
        if depth == 0 && matches!(signal, RelationalSignal::Secondary { .. }) {
            self.report(DiagnosticKind::SecondarySignalAtRoot, signal_span);
        }

        let canonical_order = self.canonical_order(block, signal_span);
        let global = GlobalClause {
            period: st.id,
            clause,
        };
        let depth_index = usize::from(depth);
        // Same resolution for every pair. Rule 1B (v4.1): the signal pair takes the
        // number of the signal tile, written or inherited from the parent's signal;
        // never the number of an anaphora antecedent.
        let signal_number = effective.number;
        let pairs = core::iter::once((&block.signal.glyph, signal_number, None)).chain(
            block
                .cases
                .iter()
                .map(|pair| (&pair.glyph, pair.tile.node.number, Some(&pair.tile))),
        );
        let mut slots = Vec::with_capacity(block.pair_count());
        for (position, (glyph, written_number, case_tile)) in pairs.enumerate() {
            let Some(position) = to_u32(position) else {
                self.report(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::IndexSpace,
                    },
                    glyph.span,
                );
                break;
            };
            if let Some(tile) = case_tile.filter(|tile| tile.node.case.is_loop()) {
                self.check_loop_tile(tile, depth, &effective);
                // B2: `~` after `abdur` is a structural marker (like `abse ~`), not an anaphora.
                if matches!(glyph.node, GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact)) {
                    slots.push(None);
                    continue;
                }
            }
            let analysis = self.resolve_reporting(
                st,
                SlotQuery {
                    depth: depth_index,
                    slot: SlotRef {
                        clause: global,
                        slot: SlotIndex(position),
                    },
                    written_number,
                    inherit_number: case_tile.is_some(),
                    variables: true,
                },
                glyph,
            );
            slots.push(Some(analysis));
        }

        st.clauses.push(ClauseAnalysis {
            role,
            depth,
            parent,
            children: Vec::new(),
            conjunction: link.conjunction(),
            signal: Some(effective),
            canonical_order,
            slots: slots.into_boxed_slice(),
        });
        match parent {
            Some(p) => {
                if let Some(parent_analysis) = st.clauses.get_mut(p.slot()) {
                    parent_analysis.children.push(clause);
                }
            }
            None => st.roots.push(clause),
        }
        st.stack.push(clause);
    }

    /// B2: `abdur=<N>` must not exceed `max_loop_iterations` (L5); the bare loop
    /// marker `abdur` is allowed only in a subordinate clause whose effective
    /// signal carries a conditional, written or inherited from the parent (v4.1).
    fn check_loop_tile(
        &mut self,
        tile: &Spanned<CaseTile>,
        depth: u8,
        effective: &EffectiveSignal,
    ) {
        match tile.node.case.loop_count() {
            Some(count) => {
                if count.get() > self.max_loop_iterations.get() {
                    self.report(
                        DiagnosticKind::LimitExceeded {
                            limit: Limit::LoopIterations,
                        },
                        tile.span,
                    );
                }
            }
            None => {
                let conditional = matches!(effective.tense, Some(TemporalMode::Conditional(_)));
                if depth == 0 || !conditional {
                    self.report(DiagnosticKind::LoopMarkerOutsideConditional, tile.span);
                }
            }
        }
    }

    /// M§3.1: signal pair (position 0), then the case pairs
    /// stably reordered by case rank.
    fn canonical_order(&mut self, block: &RelationalBlock, span: Span) -> Box<[u32]> {
        let mut cases: Vec<(u8, u32)> = Vec::with_capacity(block.cases.len());
        for (index, pair) in block.cases.iter().enumerate() {
            let Some(position) = index.checked_add(1).and_then(to_u32) else {
                self.report(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::IndexSpace,
                    },
                    span,
                );
                break;
            };
            cases.push((pair.tile.node.case.canonical_rank(), position));
        }
        cases.sort_by_key(|(rank, _)| *rank);
        core::iter::once(SlotIndex::SIGNAL.0)
            .chain(cases.into_iter().map(|(_, position)| position))
            .collect()
    }

    // ───────────────────────── def (Q11, R11, R14) ─────────────────────────

    fn definition(
        &mut self,
        st: &mut PeriodState<'a>,
        clause: ClauseId,
        link: LinkInfo,
        block: &'a DefinitionBlock,
    ) {
        let signal_span = block.signal.span;
        let current = self.current_depth(st, signal_span);
        // Q16: `def` is transparent; explicit `:` are only checked.
        let depth = match link {
            LinkInfo::Head | LinkInfo::Conj(Conjunction::Sed) => 0,
            LinkInfo::Conj(_) => current,
            LinkInfo::Colons(count) | LinkInfo::ConjColons(_, count) => {
                match self.colon_depth(st, count, signal_span) {
                    Some(requested) => self.check_skip(st, requested, signal_span),
                    None => current,
                }
            }
        };

        let global = GlobalClause {
            period: st.id,
            clause,
        };
        let name = block.name.node;
        let number = block.signal.node.number;
        // Pair 0: variable name (Q11), not resolved.
        let mut slots: Vec<Option<SlotAnalysis>> =
            Vec::with_capacity(block.operations.len().saturating_add(1));
        slots.push(None);

        for (index, operation) in block.operations.iter().enumerate() {
            let Some(position) = index.checked_add(1).and_then(to_u32) else {
                self.report(
                    DiagnosticKind::LimitExceeded {
                        limit: Limit::IndexSpace,
                    },
                    signal_span,
                );
                break;
            };
            let query = SlotQuery {
                depth: usize::from(depth),
                slot: SlotRef {
                    clause: global,
                    slot: SlotIndex(position),
                },
                written_number: number,
                inherit_number: true,
                variables: false,
            };
            match operation {
                DefOperation::Assign { tile, value } => {
                    let analysis = self.resolve_reporting(st, query, value);
                    if self.live(name).is_some() {
                        self.report(DiagnosticKind::AssignToLiveVariable, tile.span);
                    } else if tile.node.is_some() && tile.node != number {
                        self.report(DiagnosticKind::VariableNumberMismatch, tile.span);
                    } else {
                        self.bind(name, number, vec![analysis.value], global, tile.span);
                    }
                    slots.push(Some(analysis));
                }
                DefOperation::Modify { tile, value } => {
                    let analysis = self.resolve_reporting(st, query, value);
                    if self
                        .checked_live(name, number, *tile, signal_span)
                        .is_some()
                    {
                        self.bind(name, number, vec![analysis.value], global, *tile);
                    }
                    slots.push(Some(analysis));
                }
                DefOperation::Union { tile, value } => {
                    let analysis = self.resolve_reporting(st, query, value);
                    if let Some(current_binding) =
                        self.checked_live(name, number, *tile, signal_span)
                    {
                        // Copy of `Copy` values (indices): justified, the previous
                        // binding stays unchanged for resolutions already performed.
                        let mut merged: Vec<SlotValue> = self
                            .binding(current_binding)
                            .map(|b| b.values.to_vec())
                            .unwrap_or_default();
                        merged.push(analysis.value);
                        self.bind(name, number, merged, global, *tile);
                    }
                    slots.push(Some(analysis));
                }
                DefOperation::Unbind { tile, .. } => {
                    // R14: the tilde of `abse ~` is not an anaphora.
                    if self
                        .checked_live(name, number, *tile, signal_span)
                        .is_some()
                    {
                        self.overlay.insert(name, None);
                    }
                    slots.push(None);
                }
            }
        }

        st.clauses.push(ClauseAnalysis::non_relational(
            DspRole::Transparent,
            depth,
            link.conjunction(),
            slots,
        ));
    }

    /// R11: the variable must be live and have the same number as the `def` signal.
    fn checked_live(
        &mut self,
        name: Symbol,
        number: Option<ExplicitNumber>,
        tile_span: Span,
        signal_span: Span,
    ) -> Option<BindingId> {
        let Some(binding) = self.live(name) else {
            self.report(DiagnosticKind::UndefinedVariable, tile_span);
            return None;
        };
        if self.binding(binding).map(|b| b.number) != Some(number) {
            self.report(DiagnosticKind::VariableNumberMismatch, signal_span);
            return None;
        }
        Some(binding)
    }

    fn bind(
        &mut self,
        name: Symbol,
        number: Option<ExplicitNumber>,
        values: Vec<SlotValue>,
        defined_at: GlobalClause,
        span: Span,
    ) {
        let Some(index) = self
            .committed
            .bindings
            .len()
            .checked_add(self.bindings.len())
            .and_then(to_u32)
        else {
            self.report(
                DiagnosticKind::LimitExceeded {
                    limit: Limit::IndexSpace,
                },
                span,
            );
            return;
        };
        self.bindings.push(Binding {
            name,
            number,
            values: values.into_boxed_slice(),
            defined_at,
        });
        self.overlay.insert(name, Some(BindingId(index)));
    }

    // ───────────────────────── Portability (Q6, R1–R3) ─────────────────────────

    fn portability(
        &mut self,
        st: &mut PeriodState<'a>,
        colons: Spanned<ColonRun>,
        block: &'a PortabilityBlock,
    ) {
        let current = self.current_depth(st, colons.span);
        let depth = match self.colon_depth(st, colons.node.count(), colons.span) {
            Some(requested) => self.check_skip(st, requested, colons.span),
            None => current,
        };

        match block {
            PortabilityBlock::Anchor { keyword, id } => {
                self.open_anchor(st, depth, &id.node, *keyword, false);
            }
            PortabilityBlock::AnchorRel { keyword, id } => {
                self.open_anchor(st, depth, &id.node, *keyword, true);
            }
            PortabilityBlock::Rel { id, .. } => {
                match st.anchors.iter().rposition(|a| *a.id == id.node) {
                    Some(position) => {
                        st.anchors.truncate(position.saturating_add(1));
                        st.rel_mode = true;
                    }
                    None => self.report(DiagnosticKind::UnknownAnchor, id.span),
                }
            }
            PortabilityBlock::RelOff { id, .. } => {
                if st.anchors.iter().any(|a| *a.id == id.node) {
                    st.rel_mode = false;
                } else {
                    self.report(DiagnosticKind::UnknownAnchor, id.span);
                }
            }
            PortabilityBlock::End { keyword, .. } => match st.anchors.pop() {
                Some(anchor) => {
                    st.stack.truncate(usize::from(anchor.depth));
                    st.rel_mode = !st.anchors.is_empty();
                }
                None => self.report(DiagnosticKind::EndWithoutAnchor, *keyword),
            },
        }

        // The `[keyword] [operand]` pair contains no glyph to resolve.
        st.clauses.push(ClauseAnalysis::non_relational(
            DspRole::Portability,
            depth,
            None,
            vec![None],
        ));
    }

    fn open_anchor(
        &mut self,
        st: &mut PeriodState<'a>,
        depth: u8,
        id: &'a GlyphPath,
        span: Span,
        relative: bool,
    ) {
        st.stack.truncate(usize::from(depth));
        st.anchors.push(AnchorFrame { id, depth, span });
        st.rel_mode = relative;
    }

    // ───────────────────────── Period end ─────────────────────────

    /// §9.2 (v4.1): siblings keep the written order, each with its own
    /// conjunction; no coordination tree or boolean grouping is presumed.
    fn finish_period(&mut self, st: PeriodState<'a>, _span: Span) -> AnalyzedPeriod {
        let PeriodState { clauses, roots, .. } = st;
        AnalyzedPeriod {
            clauses: clauses.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
            warnings: Box::new([]),
        }
    }
}
