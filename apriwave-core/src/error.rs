//! Structured diagnostics.
//!
//! `ErrorCode` contains **only** the 6 official codes of M§7 (Q26). Every variant of
//! `DiagnosticKind` maps to exactly one code through an exhaustive `match`.

use core::fmt;

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    Syntax,
    Order,
    Reference,
    Semantic,
    Scope,
    CaseAmbiguity,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "ERR_SYNTAX",
            Self::Order => "ERR_ORDER",
            Self::Reference => "ERR_REFERENCE",
            Self::Semantic => "ERR_SEMANTIC",
            Self::Scope => "ERR_SCOPE",
            Self::CaseAmbiguity => "ERR_CASE_AMBIGUITY",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Class of a signal determiner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeterminerClass {
    Quantifier,
    Reflexive,
    Comparative,
    Temporal,
    Conditional,
    /// Quantum number slot at the end of a relational signal tile (rule 1B).
    Number,
}

impl DeterminerClass {
    /// Position in the fixed order; temporal and conditional share the same slot (Q5).
    pub const fn slot(self) -> u8 {
        match self {
            Self::Quantifier => 0,
            Self::Reflexive => 1,
            Self::Comparative => 2,
            Self::Temporal | Self::Conditional => 3,
            Self::Number => 4,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Quantifier => "quantifier",
            Self::Reflexive => "reflexive",
            Self::Comparative => "comparative",
            Self::Temporal => "temporal",
            Self::Conditional => "conditional",
            Self::Number => "number",
        }
    }
}

/// What the parser expected at a given position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Expected {
    PeriodOpen,
    SignalTile,
    CaseTileLinkOrClose,
}

impl Expected {
    const fn label(self) -> &'static str {
        match self {
            Self::PeriodOpen => "a period open (/^, /^≈, /^≃)",
            Self::SignalTile => "a signal tile",
            Self::CaseTileLinkOrClose => "a case tile, a connector or a period close",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphFault {
    EmptySegment,
    IllegalCharacter {
        ch: char,
    },
    UnterminatedRawString,
    QuoteInsideRawString,
    /// D9: control (`Cc`) or format/bidi character inside a raw string.
    ForbiddenRawCharacter {
        ch: char,
    },
}

/// Resource limits (all mapped to `ERR_SCOPE`, consistently with Q13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Limit {
    InputSize,
    DspDepth,
    SymbolSpace,
    PeriodCount,
    /// Clause, pair, binding or coordination-node indices beyond `u32::MAX`.
    IndexSpace,
    /// `abdur=<N>` count or loop-generated plan steps beyond `max_loop_iterations` (B2, L5).
    LoopIterations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticKind {
    // ── ERR_SYNTAX ──
    InvalidWhitespace,
    UppercaseCharacter {
        ch: char,
    },
    ForbiddenSeparator {
        ch: char,
    },
    MissingPeriodOpen,
    MissingPeriodClose,
    UnexpectedEnd {
        expected: Expected,
    },
    UnexpectedToken {
        expected: Expected,
    },
    UnknownSignalTile,
    MissingPrimaryAfterSecondary,
    DeterminerOnDef,
    UnknownCaseTile,
    MalformedGlyph {
        fault: GlyphFault,
    },
    SignalTileWithoutGlyph,
    AccusativeInReflexive,
    DefNameNotAtomic,
    DefOperationNotAllowed,
    UnbindRequiresTilde,
    PortabilityOperandInvalid,
    EndRequiresTilde,
    PortabilityWithCasePairs,
    // ── ERR_ORDER ──
    DeterminerOutOfOrder {
        found: DeterminerClass,
        after: DeterminerClass,
    },
    DuplicateDeterminer {
        first: DeterminerClass,
        second: DeterminerClass,
    },
    // ── ERR_REFERENCE (stage 2) ──
    UnresolvedGlyphAnaphora,
    UnresolvedSemaAnaphora,
    UnknownAnchor,
    UndefinedVariable,
    // ── ERR_SEMANTIC ──
    ComparativeWithoutAccusative,
    DefWithoutOperations,
    AssignToLiveVariable,
    VariableNumberMismatch,
    // ── ERR_SCOPE ──
    SynchronizedOpenWithoutAntecedent,
    PortabilityOutsideSubordination,
    LimitExceeded {
        limit: Limit,
    },
    DspLevelSkip {
        max_allowed: u8,
        requested: u8,
    },
    EndWithoutAnchor,
    UnclosedAnchor,
    QuantifierMisuse,
    SecondarySignalAtRoot,
    /// B2: bare `abdur` outside a subordinate clause with a written conditional.
    LoopMarkerOutsideConditional,
    // ── ERR_CASE_AMBIGUITY ──
    CaseTileWithoutGlyph,
    DuplicateCase,
}

impl DiagnosticKind {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidWhitespace
            | Self::UppercaseCharacter { .. }
            | Self::ForbiddenSeparator { .. }
            | Self::MissingPeriodOpen
            | Self::MissingPeriodClose
            | Self::UnexpectedEnd { .. }
            | Self::UnexpectedToken { .. }
            | Self::UnknownSignalTile
            | Self::MissingPrimaryAfterSecondary
            | Self::DeterminerOnDef
            | Self::UnknownCaseTile
            | Self::MalformedGlyph { .. }
            | Self::SignalTileWithoutGlyph
            | Self::AccusativeInReflexive
            | Self::DefNameNotAtomic
            | Self::DefOperationNotAllowed
            | Self::UnbindRequiresTilde
            | Self::PortabilityOperandInvalid
            | Self::EndRequiresTilde
            | Self::PortabilityWithCasePairs => ErrorCode::Syntax,
            Self::DeterminerOutOfOrder { .. } | Self::DuplicateDeterminer { .. } => {
                ErrorCode::Order
            }
            Self::UnresolvedGlyphAnaphora
            | Self::UnresolvedSemaAnaphora
            | Self::UnknownAnchor
            | Self::UndefinedVariable => ErrorCode::Reference,
            Self::ComparativeWithoutAccusative
            | Self::DefWithoutOperations
            | Self::AssignToLiveVariable
            | Self::VariableNumberMismatch => ErrorCode::Semantic,
            Self::SynchronizedOpenWithoutAntecedent
            | Self::PortabilityOutsideSubordination
            | Self::LimitExceeded { .. }
            | Self::DspLevelSkip { .. }
            | Self::EndWithoutAnchor
            | Self::UnclosedAnchor
            | Self::QuantifierMisuse
            | Self::SecondarySignalAtRoot
            | Self::LoopMarkerOutsideConditional => ErrorCode::Scope,
            Self::CaseTileWithoutGlyph | Self::DuplicateCase => ErrorCode::CaseAmbiguity,
        }
    }
}

impl fmt::Display for DiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWhitespace => f.write_str(
                "non-canonical separator: only a single ASCII space is allowed between tokens",
            ),
            Self::UppercaseCharacter { ch } => write!(f, "uppercase character not allowed: '{ch}'"),
            Self::ForbiddenSeparator { ch } => write!(f, "forbidden separator: '{ch}'"),
            Self::MissingPeriodOpen => f.write_str("missing period open (/^, /^≈, /^≃)"),
            Self::MissingPeriodClose => f.write_str("missing period close (/^/, ^/, ^^/)"),
            Self::UnexpectedEnd { expected } => {
                write!(f, "unexpected end: expected {}", expected.label())
            }
            Self::UnexpectedToken { expected } => {
                write!(f, "unexpected token: expected {}", expected.label())
            }
            Self::UnknownSignalTile => f.write_str("unknown signal tile"),
            Self::MissingPrimaryAfterSecondary => {
                f.write_str("a secondary signal requires a primary signal immediately after it")
            }
            Self::DeterminerOnDef => {
                f.write_str("def agglutinates only the number: determiners are not allowed")
            }
            Self::UnknownCaseTile => f.write_str("unknown case tile"),
            Self::MalformedGlyph { fault } => match fault {
                GlyphFault::EmptySegment => {
                    f.write_str("malformed glyph: empty segment around '§'")
                }
                GlyphFault::IllegalCharacter { ch } => {
                    write!(f, "malformed glyph: character not allowed '{ch}'")
                }
                GlyphFault::UnterminatedRawString => f.write_str("unterminated raw string"),
                GlyphFault::QuoteInsideRawString => {
                    f.write_str("raw string with disallowed inner quotes")
                }
                GlyphFault::ForbiddenRawCharacter { ch } => write!(
                    f,
                    "raw string with disallowed control or format character U+{:04X}",
                    u32::from(*ch)
                ),
            },
            Self::SignalTileWithoutGlyph => f.write_str("signal tile not followed by a glyph"),
            Self::AccusativeInReflexive => {
                f.write_str("Accusative case not allowed in a ref block")
            }
            Self::DefNameNotAtomic => {
                f.write_str("the name of a def variable must be an atomic glyph without '§'")
            }
            Self::DefOperationNotAllowed => {
                f.write_str("only a/au/am/anum, abfi, absi, abse ~ are allowed in a def block")
            }
            Self::UnbindRequiresTilde => f.write_str("abse in a def block must be followed by '~'"),
            Self::PortabilityOperandInvalid => {
                f.write_str("the operand of anchor/anchorrel/rel/reloff must be a glyph")
            }
            Self::EndRequiresTilde => f.write_str("end must be followed by '~'"),
            Self::PortabilityWithCasePairs => {
                f.write_str("portability tiles do not accept [case] [glyph] pairs")
            }
            Self::DeterminerOutOfOrder { found, after } => write!(
                f,
                "{} determiner out of sequence after {}",
                found.label(),
                after.label()
            ),
            Self::DuplicateDeterminer { first, second } => write!(
                f,
                "determiner slot already occupied: {} after {}",
                second.label(),
                first.label()
            ),
            Self::ComparativeWithoutAccusative => f.write_str(
                "the comparative determiner requires at least one pair in the Accusative case",
            ),
            Self::DefWithoutOperations => {
                f.write_str("a def block requires at least one [case] [glyph] pair")
            }
            Self::UnresolvedGlyphAnaphora => {
                f.write_str("glyph anaphora without an antecedent in the active DSP stack")
            }
            Self::UnresolvedSemaAnaphora => {
                f.write_str("sema anaphora without an antecedent in the active DSP stack")
            }
            Self::UnknownAnchor => f.write_str("anchor not present in the stack of active anchors"),
            Self::UndefinedVariable => f.write_str("def variable not defined or already removed"),
            Self::AssignToLiveVariable => {
                f.write_str("def variable already live: use abfi to modify it")
            }
            Self::VariableNumberMismatch => {
                f.write_str("quantum number conflicts with that of the def variable")
            }
            Self::DspLevelSkip {
                max_allowed,
                requested,
            } => write!(
                f,
                "DSP level skip: requested {requested}, maximum allowed {max_allowed}"
            ),
            Self::EndWithoutAnchor => f.write_str("end ~ without any open anchor"),
            Self::UnclosedAnchor => {
                f.write_str("anchor not closed by end ~ before the end of the period")
            }
            Self::QuantifierMisuse => {
                f.write_str("quantifier in a block that has no case pairs and is not reflexive")
            }
            Self::SecondarySignalAtRoot => {
                f.write_str("secondary signal not allowed at DSP level 0")
            }
            Self::LoopMarkerOutsideConditional => f.write_str(
                "loop marker `abdur` without `=<N>` is allowed only in a subordinate clause with a written conditional",
            ),
            Self::SynchronizedOpenWithoutAntecedent => {
                f.write_str("/^≈ and /^≃ require an antecedent period in the session")
            }
            Self::PortabilityOutsideSubordination => {
                f.write_str("portability tiles are allowed only after a ':' connector")
            }
            Self::LimitExceeded { limit } => match limit {
                Limit::InputSize => f.write_str("input exceeds u32::MAX bytes"),
                Limit::DspDepth => f.write_str("DSP depth exceeds the configured limit"),
                Limit::SymbolSpace => f.write_str("symbol space exhausted"),
                Limit::PeriodCount => f.write_str("number of periods exceeds u32::MAX"),
                Limit::IndexSpace => f.write_str("u32 index space exhausted"),
                Limit::LoopIterations => f.write_str("loop iterations beyond the configured limit"),
            },
            Self::CaseTileWithoutGlyph => f.write_str("case tile not followed by a glyph"),
            Self::DuplicateCase => f.write_str("duplicate case in the same block"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub span: Span,
}

impl Diagnostic {
    pub const fn new(kind: DiagnosticKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub const fn code(&self) -> ErrorCode {
        self.kind.code()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] byte {}..{}: {}",
            self.code(),
            self.span.start,
            self.span.end,
            self.kind
        )
    }
}

impl core::error::Error for Diagnostic {}
