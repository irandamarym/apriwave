//! ApriWave v4.1 AST, stage 1: faithful syntactic form, no resolution.
//!
//! Principles:
//! 1. **Illegal states are unrepresentable**: `def` without determiners, `taaltzato`,
//!    `nef`, portability tiles as `head` or after a conjunction have no
//!    representation at all.
//! 2. **Omitted ≠ explicit**: `Option<TemporalMode>` and `Option<ExplicitNumber>`
//!    preserve what was actually written (required for DSP inheritance).
//! 3. **No syntactic recursion**: the hierarchy derives from the number of `:`,
//!    so the AST is flat and free of recursive `Box`es.
//!
//! The `ALL` + `surface()` tables are the **single source of truth** for surface
//! forms, shared by the parser and the printer.

use crate::prelude::*;

use core::num::NonZeroU8;

use crate::span::{Span, Spanned};
use crate::symbol::Symbol;

// ═══════════════════════════ Utilities ═══════════════════════════

/// Non-empty sequence, guaranteed by the type.
#[derive(Debug, PartialEq, Eq)]
pub struct NonEmpty<T> {
    pub first: T,
    pub rest: Box<[T]>,
}

impl<T> NonEmpty<T> {
    /// `None` if the vector is empty.
    pub fn from_vec(items: Vec<T>) -> Option<Self> {
        let mut iter = items.into_iter();
        let first = iter.next()?;
        Some(Self {
            first,
            rest: iter.collect(),
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        core::iter::once(&self.first).chain(self.rest.iter())
    }

    pub fn len(&self) -> usize {
        self.rest.len().saturating_add(1)
    }

    /// Always `false`: present for symmetry with `len`.
    pub fn is_empty(&self) -> bool {
        false
    }
}

// ═══════════════════════════ Period ═══════════════════════════

/// `period = period_open SP block { SP link SP block } SP period_close`
#[derive(Debug, PartialEq, Eq)]
pub struct Period {
    pub open: Spanned<PeriodOpen>,
    /// First block: by construction it has no connector and is never a portability block (Q6).
    pub head: ClauseBlock,
    pub tail: Vec<LinkedClause>,
    pub close: Spanned<PeriodClose>,
}

/// Read-only reference to a block of the period, indexed like `ClauseId`
/// (`0` = `head`, `i + 1` = `tail[i]`).
#[derive(Debug, Clone, Copy)]
pub enum ClauseRef<'a> {
    Relational(&'a RelationalBlock),
    Definition(&'a DefinitionBlock),
    Portability(&'a PortabilityBlock),
}

impl<'a> From<&'a ClauseBlock> for ClauseRef<'a> {
    fn from(block: &'a ClauseBlock) -> Self {
        match block {
            ClauseBlock::Relational(b) => Self::Relational(b),
            ClauseBlock::Definition(b) => Self::Definition(b),
        }
    }
}

impl Period {
    /// Block at index `index` (`0` = `head`).
    pub fn clause(&self, index: u32) -> Option<ClauseRef<'_>> {
        let index = usize::try_from(index).ok()?;
        match index.checked_sub(1) {
            None => Some(ClauseRef::from(&self.head)),
            Some(tail_index) => self.tail.get(tail_index).map(|linked| match linked {
                LinkedClause::Clause { block, .. } => ClauseRef::from(block),
                LinkedClause::Portability { block, .. } => ClauseRef::Portability(block),
            }),
        }
    }

    /// Total number of blocks (`head` included).
    pub fn clause_count(&self) -> usize {
        self.tail.len().saturating_add(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeriodOpen {
    /// `/^`
    Standard,
    /// `/^≈`
    StrictSimultaneity,
    /// `/^≃`
    LogicalProximity,
}

impl PeriodOpen {
    pub const ALL: [Self; 3] = [
        Self::Standard,
        Self::StrictSimultaneity,
        Self::LogicalProximity,
    ];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Standard => "/^",
            Self::StrictSimultaneity => "/^≈",
            Self::LogicalProximity => "/^≃",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeriodClose {
    /// `/^/`
    Soft,
    /// `^/`
    Standard,
    /// `^^/`
    Hard,
}

impl PeriodClose {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Standard, Self::Hard];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Soft => "/^/",
            Self::Standard => "^/",
            Self::Hard => "^^/",
        }
    }
}

// ═══════════════════════════ Connectors ═══════════════════════════

/// Block preceded by its own connector.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkedClause {
    Clause {
        link: Spanned<Link>,
        block: ClauseBlock,
    },
    /// Q6: portability tiles appear **exclusively** after `Link::Subordination`.
    Portability {
        colons: Spanned<ColonRun>,
        block: PortabilityBlock,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// `:`, `::`, `:::`, …
    Subordination(ColonRun),
    /// `y` | `vel` | `aut` | `sed`, optionally followed by `:`+.
    Coordination {
        conj: Spanned<Conjunction>,
        colons: Option<Spanned<ColonRun>>,
    },
}

/// Number of consecutive colons (≥ 1, ≤ `ParserConfig::max_dsp_depth`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ColonRun(NonZeroU8);

impl ColonRun {
    pub const fn new(count: NonZeroU8) -> Self {
        Self(count)
    }

    pub const fn count(self) -> u8 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Conjunction {
    Y,
    Vel,
    Aut,
    Sed,
}

impl Conjunction {
    pub const ALL: [Self; 4] = [Self::Y, Self::Vel, Self::Aut, Self::Sed];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Y => "y",
            Self::Vel => "vel",
            Self::Aut => "aut",
            Self::Sed => "sed",
        }
    }
}

// ═══════════════════════════ Blocks ═══════════════════════════

/// Block that can occupy the `head` position or follow any connector.
#[derive(Debug, PartialEq, Eq)]
pub enum ClauseBlock {
    Relational(RelationalBlock),
    Definition(DefinitionBlock),
}

/// Positional `[Tile] [Glyph]` pair.
///
/// Every even slot has the same status: the grammar assigns no categories to
/// glyphs; only the tile preceding them declares their function in the relation.
#[derive(Debug, PartialEq, Eq)]
pub struct Pair<T> {
    pub tile: Spanned<T>,
    pub glyph: Spanned<GlyphSlot>,
}

/// Relational block: sequence of `[Tile] [Glyph]` pairs in which the first pair
/// carries the signal tile and the following ones carry the case tiles.
#[derive(Debug, PartialEq, Eq)]
pub struct RelationalBlock {
    /// `[signal+determiners] [glyph]` pair (position 0).
    pub signal: Pair<RelationalSignal>,
    /// `[case] [glyph]` pairs (positions 1…n) in written order; the
    /// `n→a→d→g→ab` normalization is the job of stage 2.
    pub cases: Vec<Pair<CaseTile>>,
}

impl RelationalBlock {
    /// Number of pairs in the block (signal pair included).
    pub fn pair_count(&self) -> usize {
        self.cases.len().saturating_add(1)
    }

    /// Glyph of the pair at position `position` (`0` = signal pair).
    pub fn glyph(&self, position: u32) -> Option<&Spanned<GlyphSlot>> {
        let position = usize::try_from(position).ok()?;
        match position.checked_sub(1) {
            None => Some(&self.signal.glyph),
            Some(case_index) => self.cases.get(case_index).map(|pair| &pair.glyph),
        }
    }
}

/// `def` block (Q11). Surface identical to a relational block
/// (`[def…] [glyph] { [case] [glyph] }`), but typed so that the constraints the
/// `def` tile imposes on its own pairs (Q11: a glyph token as name, at least
/// one operation, only `a*`/`abfi`/`absi`/`abse ~`) are guaranteed by the type.
#[derive(Debug, PartialEq, Eq)]
pub struct DefinitionBlock {
    pub signal: Spanned<DefSignal>,
    /// Glyph of the signal pair: the variable name, atomic (without `§`).
    pub name: Spanned<Symbol>,
    pub operations: NonEmpty<DefOperation>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DefOperation {
    /// `a` | `au` | `am` | `anum` — initial assignment.
    Assign {
        tile: Spanned<Option<ExplicitNumber>>,
        value: Spanned<GlyphSlot>,
    },
    /// `abfi` — modification that keeps the variable alive.
    Modify {
        tile: Span,
        value: Spanned<GlyphSlot>,
    },
    /// `absi` — extension/union.
    Union {
        tile: Span,
        value: Spanned<GlyphSlot>,
    },
    /// `abse ~` — deletion (unbind).
    Unbind { tile: Span, tilde: Span },
}

/// Portability tile (Q6). No determiners, no case pairs.
#[derive(Debug, PartialEq, Eq)]
pub enum PortabilityBlock {
    Anchor {
        keyword: Span,
        id: Spanned<GlyphPath>,
    },
    AnchorRel {
        keyword: Span,
        id: Spanned<GlyphPath>,
    },
    Rel {
        keyword: Span,
        id: Spanned<GlyphPath>,
    },
    RelOff {
        keyword: Span,
        id: Spanned<GlyphPath>,
    },
    /// `end ~`
    End { keyword: Span, tilde: Span },
}

impl PortabilityBlock {
    pub const fn keyword(&self) -> PortabilityKeyword {
        match self {
            Self::Anchor { .. } => PortabilityKeyword::Anchor,
            Self::AnchorRel { .. } => PortabilityKeyword::AnchorRel,
            Self::Rel { .. } => PortabilityKeyword::Rel,
            Self::RelOff { .. } => PortabilityKeyword::RelOff,
            Self::End { .. } => PortabilityKeyword::End,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortabilityKeyword {
    Anchor,
    AnchorRel,
    Rel,
    RelOff,
    End,
}

impl PortabilityKeyword {
    pub const ALL: [Self; 5] = [
        Self::Anchor,
        Self::AnchorRel,
        Self::Rel,
        Self::RelOff,
        Self::End,
    ];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::AnchorRel => "anchorrel",
            Self::Rel => "rel",
            Self::RelOff => "reloff",
            Self::End => "end",
        }
    }
}

// ═══════════════════════════ Signal tiles ═══════════════════════════

/// Signal of a relational block (Q3, Q4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationalSignal {
    /// `ta` | `to` | `ka` | `ko` | `ot` | `oh` + determiners.
    Primary {
        signal: PrimarySignal,
        determiners: SignalDeterminers,
    },
    /// `qua|que|quo|quu` + mandatory primary + determiners (e.g. `quatazato`).
    Secondary {
        secondary: SecondarySignal,
        primary: PrimarySignal,
        determiners: SignalDeterminers,
    },
    /// `deep` + determiners, never a primary (e.g. `deepall`, `deepzato`).
    Deep { determiners: SignalDeterminers },
}

impl RelationalSignal {
    pub const fn determiners(&self) -> &SignalDeterminers {
        match self {
            Self::Primary { determiners, .. }
            | Self::Secondary { determiners, .. }
            | Self::Deep { determiners } => determiners,
        }
    }
}

/// `def` | `defu` | `defm` | `defnum`: agglutinates only the number (Q4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefSignal {
    pub number: Option<ExplicitNumber>,
}

impl DefSignal {
    pub const SURFACE: &'static str = "def";
}

/// Lexical marker of `deep`.
pub const DEEP_SURFACE: &str = "deep";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimarySignal {
    Ta,
    To,
    Ka,
    Ko,
    Ot,
    Oh,
}

impl PrimarySignal {
    pub const ALL: [Self; 6] = [Self::Ta, Self::To, Self::Ka, Self::Ko, Self::Ot, Self::Oh];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Ta => "ta",
            Self::To => "to",
            Self::Ka => "ka",
            Self::Ko => "ko",
            Self::Ot => "ot",
            Self::Oh => "oh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecondarySignal {
    Qua,
    Que,
    Quo,
    Quu,
}

impl SecondarySignal {
    pub const ALL: [Self; 4] = [Self::Qua, Self::Que, Self::Quo, Self::Quu];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Qua => "qua",
            Self::Que => "que",
            Self::Quo => "quo",
            Self::Quu => "quu",
        }
    }
}

/// Tail of a relational signal tile in the fixed order
/// `[quantifier][ref][comparative][temporal|conditional][number]`.
/// `None` = omitted (distinct from an explicit value, e.g. `ato`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SignalDeterminers {
    pub quantifier: Option<Quantifier>,
    pub reflexive: Option<Reflexive>,
    pub comparative: Option<Comparative>,
    /// Q5: temporal and conditional are mutually exclusive (a single slot).
    pub tense: Option<TemporalMode>,
    /// Quantum number of the signal pair (`tau`, `tam`, `tanum`, `deepallu`),
    /// as written (rule 1B, v4.1: when omitted, stage 2 inherits the parent
    /// signal's number in a subordinate clause, otherwise Dual).
    pub number: Option<ExplicitNumber>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantifier {
    /// `all`
    Universal,
    /// `some`
    Existential,
}

impl Quantifier {
    pub const ALL: [Self; 2] = [Self::Universal, Self::Existential];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Universal => "all",
            Self::Existential => "some",
        }
    }
}

/// `ref`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reflexive;

impl Reflexive {
    pub const SURFACE: &'static str = "ref";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Comparative {
    Cha,
    Chu,
    Cla,
    Col,
    Cal,
    Cua,
    Cuo,
    Cup,
}

impl Comparative {
    pub const ALL: [Self; 8] = [
        Self::Cha,
        Self::Chu,
        Self::Cla,
        Self::Col,
        Self::Cal,
        Self::Cua,
        Self::Cuo,
        Self::Cup,
    ];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Cha => "cha",
            Self::Chu => "chu",
            Self::Cla => "cla",
            Self::Col => "col",
            Self::Cal => "cal",
            Self::Cua => "cua",
            Self::Cuo => "cuo",
            Self::Cup => "cup",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Temporal {
    Ato,
    Alt,
    Alv,
    Att,
    Atv,
    Asu,
}

impl Temporal {
    pub const ALL: [Self; 6] = [
        Self::Ato,
        Self::Alt,
        Self::Alv,
        Self::Att,
        Self::Atv,
        Self::Asu,
    ];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Ato => "ato",
            Self::Alt => "alt",
            Self::Alv => "alv",
            Self::Att => "att",
            Self::Atv => "atv",
            Self::Asu => "asu",
        }
    }
}

/// M§2.5: "z + [temporal] = Conditional".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Conditional(pub Temporal);

impl Conditional {
    pub const PREFIX: &'static str = "z";
}

/// Q5: single slot for temporal or conditional.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemporalMode {
    Indicative(Temporal),
    Conditional(Conditional),
}

// ═══════════════════════════ Case tiles ═══════════════════════════

/// `[case][case_determiner][number]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaseTile {
    pub case: Case,
    /// `None` = omitted number (Dual by default, unless inherited from an anaphora — Q18).
    pub number: Option<ExplicitNumber>,
}

/// Each case carries only the determiners allowed by M§2.6-C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Case {
    Nominative,
    Accusative,
    Genitive(Option<GenitiveDeterminer>),
    Dative(Option<DativeDeterminer>),
    Ablative(Option<AblativeDeterminer>),
}

impl Case {
    pub const ALL: [Self; 17] = [
        Self::Nominative,
        Self::Accusative,
        Self::Genitive(None),
        Self::Genitive(Some(GenitiveDeterminer::Pa)),
        Self::Dative(None),
        Self::Dative(Some(DativeDeterminer::Da)),
        Self::Ablative(None),
        Self::Ablative(Some(AblativeDeterminer::Ef)),
        Self::Ablative(Some(AblativeDeterminer::En)),
        Self::Ablative(Some(AblativeDeterminer::El)),
        Self::Ablative(Some(AblativeDeterminer::Se)),
        Self::Ablative(Some(AblativeDeterminer::Fi)),
        Self::Ablative(Some(AblativeDeterminer::Cu)),
        Self::Ablative(Some(AblativeDeterminer::Si)),
        Self::Ablative(Some(AblativeDeterminer::Du)),
        Self::Ablative(Some(AblativeDeterminer::Pa)),
        Self::Ablative(Some(AblativeDeterminer::Dur(None))),
    ];

    /// Full `[case][determiner]` stem (full code from M§2.6-C).
    ///
    /// Q2: for Dative + `da` the **only** canonical form is `dad`; to change it,
    /// editing this line is enough.
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Nominative => "n",
            Self::Accusative => "a",
            Self::Genitive(None) => "g",
            Self::Genitive(Some(GenitiveDeterminer::Pa)) => "gpa",
            Self::Dative(None) => "d",
            Self::Dative(Some(DativeDeterminer::Da)) => "dad",
            Self::Ablative(None) => "ab",
            Self::Ablative(Some(AblativeDeterminer::Ef)) => "abef",
            Self::Ablative(Some(AblativeDeterminer::En)) => "aben",
            Self::Ablative(Some(AblativeDeterminer::El)) => "abel",
            Self::Ablative(Some(AblativeDeterminer::Se)) => "abse",
            Self::Ablative(Some(AblativeDeterminer::Fi)) => "abfi",
            Self::Ablative(Some(AblativeDeterminer::Cu)) => "abcu",
            Self::Ablative(Some(AblativeDeterminer::Si)) => "absi",
            Self::Ablative(Some(AblativeDeterminer::Du)) => "abdu",
            Self::Ablative(Some(AblativeDeterminer::Pa)) => "abpa",
            // The `=<N>` count, when present, follows the number suffix.
            Self::Ablative(Some(AblativeDeterminer::Dur(_))) => "abdur",
        }
    }

    /// `abdur` count, if this is a loop tile with `=<N>`.
    pub const fn loop_count(self) -> Option<LoopCount> {
        match self {
            Self::Ablative(Some(AblativeDeterminer::Dur(count))) => count,
            _ => None,
        }
    }

    /// `true` for `abdur` with or without count.
    pub const fn is_loop(self) -> bool {
        matches!(self, Self::Ablative(Some(AblativeDeterminer::Dur(_))))
    }

    /// Rank in the M§3.1 canonical order `n → a → d → g → ab` (stable sort: equal
    /// ranks keep the written order, v4.1).
    pub const fn canonical_rank(self) -> u8 {
        match self {
            Self::Nominative => 0,
            Self::Accusative => 1,
            Self::Dative(_) => 2,
            Self::Genitive(_) => 3,
            Self::Ablative(_) => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GenitiveDeterminer {
    Pa,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DativeDeterminer {
    Da,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AblativeDeterminer {
    Ef,
    En,
    El,
    Se,
    Fi,
    Cu,
    Si,
    /// `abdu`: continuous temporal complement ("while / during"); never a loop.
    Du,
    Pa,
    /// `abdur` / `abdur=<N>` (B2): grammatical loop operator. The iteration count
    /// is part of the tile (`=<N>`); the glyph that follows is never read for it.
    /// `None` = loop marker, allowed only in a subordinate clause with a written
    /// conditional determiner.
    Dur(Option<LoopCount>),
}

/// Iteration count of `abdur=<N>`: `[1-9][0-9]*`, fitting in `u32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LoopCount(core::num::NonZeroU32);

impl LoopCount {
    /// Separator between the tile and the iteration count.
    pub const SEPARATOR: char = '=';

    pub const fn new(count: core::num::NonZeroU32) -> Self {
        Self(count)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Parses `[1-9][0-9]*` (no sign, no leading zero, within `u32`).
    pub fn parse(text: &str) -> Option<Self> {
        let first = text.bytes().next()?;
        if !(b'1'..=b'9').contains(&first) || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        text.parse::<u32>()
            .ok()
            .and_then(core::num::NonZeroU32::new)
            .map(Self)
    }
}

/// Writable numbers. The Dual has no marker: it coincides with omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExplicitNumber {
    /// `u`
    Singular,
    /// `m`
    Plural,
    /// `num`
    NonDual,
}

impl ExplicitNumber {
    pub const ALL: [Self; 3] = [Self::NonDual, Self::Singular, Self::Plural];

    pub const fn surface(self) -> &'static str {
        match self {
            Self::Singular => "u",
            Self::Plural => "m",
            Self::NonDual => "num",
        }
    }
}

/// Effective quantum state (M§1.3). Used by stage 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuantumNumber {
    Dual,
    Singular,
    Plural,
    NonDual,
}

impl From<ExplicitNumber> for QuantumNumber {
    fn from(written: ExplicitNumber) -> Self {
        match written {
            ExplicitNumber::Singular => Self::Singular,
            ExplicitNumber::Plural => Self::Plural,
            ExplicitNumber::NonDual => Self::NonDual,
        }
    }
}

impl From<Option<ExplicitNumber>> for QuantumNumber {
    /// `None` (omitted number) = Dual (M§1.3).
    fn from(written: Option<ExplicitNumber>) -> Self {
        written.map_or(Self::Dual, Self::from)
    }
}

// ═══════════════════════════ Glyph slot ═══════════════════════════

/// Content of an even slot (v4.1, Slot Locking: `1 Tile -> 1 Glyph`). Every
/// token in an even slot is a glyph, except the two exact anaphora operators
/// `~` and `~~`.
#[derive(Debug, PartialEq, Eq)]
pub enum GlyphSlot {
    Glyph(GlyphPath),
    GlyphAnaphora(GlyphAnaphora),
    SemaAnaphora(SemaAnaphora),
}

/// A glyph token. `text` is the whole token (any visible characters, `§`
/// included): the DTI is searched for it first and it is the name of a `def`
/// variable. `root` and `qualifiers` are its `§` segments (possibly empty), used
/// when the whole token is not a DTI form (`glyph1§glyph2`).
#[derive(Debug, PartialEq, Eq)]
pub struct GlyphPath {
    pub text: Symbol,
    pub root: Symbol,
    pub qualifiers: Box<[Symbol]>,
}

impl GlyphPath {
    /// `true` when the token contains no `§`.
    pub fn is_atomic(&self) -> bool {
        self.qualifiers.is_empty()
    }
}

/// `~`: the last glyph, in canonical order, of the antecedent block.
#[derive(Debug, PartialEq, Eq)]
pub enum GlyphAnaphora {
    /// `~`
    Exact,
}

/// `~~`: the whole antecedent sema (block).
#[derive(Debug, PartialEq, Eq)]
pub enum SemaAnaphora {
    /// `~~`
    Latest,
}

/// Lexical markers for anaphora and qualifiers.
pub const QUALIFIER_SEPARATOR: char = '§';
pub const ANAPHORA_MARK: char = '~';
pub const SEMA_ANAPHORA_MARK: &str = "~~";
