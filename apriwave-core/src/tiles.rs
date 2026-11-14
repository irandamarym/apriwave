//! Classification of the tokens in **odd slots** (tiles) and of reserved tokens.
//!
//! Public low-level API for tooling crates (the `apriwave` editor).
//!
//! All sets of surface forms are prefix-free (verified by the tests),
//! so the ordered greedy scan of M§7 is deterministic.

use crate::ast::{
    AblativeDeterminer, Case, CaseTile, Comparative, Conditional, Conjunction, DefSignal,
    ExplicitNumber, LoopCount, PeriodClose, PeriodOpen, PortabilityKeyword, PrimarySignal,
    Quantifier, Reflexive, RelationalSignal, SecondarySignal, SignalDeterminers, Temporal,
    TemporalMode, DEEP_SURFACE,
};
use crate::error::{DeterminerClass, DiagnosticKind};

pub fn period_open(text: &str) -> Option<PeriodOpen> {
    PeriodOpen::ALL.into_iter().find(|o| o.surface() == text)
}

pub fn period_close(text: &str) -> Option<PeriodClose> {
    PeriodClose::ALL.into_iter().find(|c| c.surface() == text)
}

/// Number of `:` if the token consists only of colons.
pub fn colon_run(text: &str) -> Option<usize> {
    (!text.is_empty() && text.bytes().all(|b| b == b':')).then_some(text.len())
}

/// Tokens reserved in **any** slot (Q8): period delimiters and `:` sequences.
pub fn is_reserved(text: &str) -> bool {
    period_open(text).is_some() || period_close(text).is_some() || colon_run(text).is_some()
}

pub fn conjunction(text: &str) -> Option<Conjunction> {
    Conjunction::ALL.into_iter().find(|c| c.surface() == text)
}

pub fn portability_keyword(text: &str) -> Option<PortabilityKeyword> {
    PortabilityKeyword::ALL
        .into_iter()
        .find(|k| k.surface() == text)
}

/// Character check for tiles (M§1.4): lowercase, no `.`, `|`, `(`, `)`.
pub fn check_tile_characters(text: &str) -> Result<(), DiagnosticKind> {
    for ch in text.chars() {
        if ch.is_uppercase() {
            return Err(DiagnosticKind::UppercaseCharacter { ch });
        }
        if is_forbidden_separator(ch) {
            return Err(DiagnosticKind::ForbiddenSeparator { ch });
        }
    }
    Ok(())
}

pub(crate) const fn is_forbidden_separator(ch: char) -> bool {
    matches!(ch, '.' | '|' | '(' | ')')
}

// ───────────────────────── Number ─────────────────────────

fn split_number(text: &str) -> (Option<ExplicitNumber>, &str) {
    ExplicitNumber::ALL
        .into_iter()
        .find_map(|n| text.strip_prefix(n.surface()).map(|rest| (Some(n), rest)))
        .unwrap_or((None, text))
}

// ───────────────────────── Case ─────────────────────────

/// `[case][case_determiner][number]`. The reading is unique: after every root,
/// no other longer root can continue with a number (dedicated test).
pub fn parse_case(text: &str) -> Option<CaseTile> {
    Case::ALL.into_iter().find_map(|case| {
        let rest = text.strip_prefix(case.stem())?;
        let (number, tail) = split_number(rest);
        if tail.is_empty() {
            return Some(CaseTile { case, number });
        }
        // B2: only `abdur` accepts the `=<N>` iteration count after the number.
        if !case.is_loop() {
            return None;
        }
        let count = LoopCount::parse(tail.strip_prefix(LoopCount::SEPARATOR)?)?;
        Some(CaseTile {
            case: Case::Ablative(Some(AblativeDeterminer::Dur(Some(count)))),
            number,
        })
    })
}

// ───────────────────────── Signal ─────────────────────────

pub enum SignalParse {
    Relational(RelationalSignal),
    Def(DefSignal),
}

#[derive(Clone, Copy)]
enum Determiner {
    Quantifier(Quantifier),
    Reflexive,
    Comparative(Comparative),
    Temporal(Temporal),
    Conditional(Temporal),
    Number(ExplicitNumber),
}

impl Determiner {
    const fn class(self) -> DeterminerClass {
        match self {
            Self::Quantifier(_) => DeterminerClass::Quantifier,
            Self::Reflexive => DeterminerClass::Reflexive,
            Self::Comparative(_) => DeterminerClass::Comparative,
            Self::Temporal(_) => DeterminerClass::Temporal,
            Self::Conditional(_) => DeterminerClass::Conditional,
            Self::Number(_) => DeterminerClass::Number,
        }
    }
}

fn next_determiner(text: &str) -> Option<(Determiner, &str)> {
    if let Some(found) = Quantifier::ALL.into_iter().find_map(|q| {
        text.strip_prefix(q.surface())
            .map(|r| (Determiner::Quantifier(q), r))
    }) {
        return Some(found);
    }
    if let Some(rest) = text.strip_prefix(Reflexive::SURFACE) {
        return Some((Determiner::Reflexive, rest));
    }
    if let Some(found) = Comparative::ALL.into_iter().find_map(|c| {
        text.strip_prefix(c.surface())
            .map(|r| (Determiner::Comparative(c), r))
    }) {
        return Some(found);
    }
    if let Some(found) = Temporal::ALL.into_iter().find_map(|t| {
        text.strip_prefix(t.surface())
            .map(|r| (Determiner::Temporal(t), r))
    }) {
        return Some(found);
    }
    let after_z = text.strip_prefix(Conditional::PREFIX)?;
    Temporal::ALL.into_iter().find_map(|t| {
        after_z
            .strip_prefix(t.surface())
            .map(|r| (Determiner::Conditional(t), r))
    })
}

/// Ordered Greedy Matching (M§7): `[quantifier][ref][comparative][temporal|conditional]`.
/// Slot already occupied → `DuplicateDeterminer` (Q5, Q24); earlier slot → `DeterminerOutOfOrder`.
fn parse_determiners(mut text: &str) -> Result<SignalDeterminers, DiagnosticKind> {
    let mut determiners = SignalDeterminers::default();
    let mut previous: Option<DeterminerClass> = None;
    let mut occupied: [Option<DeterminerClass>; 5] = [None; 5];

    while !text.is_empty() {
        // Rule 1B: the optional number closes the tile. No determiner starts with
        // `u`, `m` or `n`, so the reading stays unique.
        let next = next_determiner(text).or_else(|| match split_number(text) {
            (Some(number), rest) => Some((Determiner::Number(number), rest)),
            (None, _) => None,
        });
        let (det, rest) = next.ok_or(DiagnosticKind::UnknownSignalTile)?;
        let class = det.class();
        let slot = occupied
            .get_mut(usize::from(class.slot()))
            .ok_or(DiagnosticKind::UnknownSignalTile)?;
        if let Some(first) = *slot {
            return Err(DiagnosticKind::DuplicateDeterminer {
                first,
                second: class,
            });
        }
        if let Some(after) = previous {
            if class.slot() < after.slot() {
                return Err(DiagnosticKind::DeterminerOutOfOrder {
                    found: class,
                    after,
                });
            }
        }
        *slot = Some(class);
        previous = Some(class);
        match det {
            Determiner::Quantifier(q) => determiners.quantifier = Some(q),
            Determiner::Reflexive => determiners.reflexive = Some(Reflexive),
            Determiner::Comparative(c) => determiners.comparative = Some(c),
            Determiner::Temporal(t) => determiners.tense = Some(TemporalMode::Indicative(t)),
            Determiner::Conditional(t) => {
                determiners.tense = Some(TemporalMode::Conditional(Conditional(t)));
            }
            Determiner::Number(n) => determiners.number = Some(n),
        }
        text = rest;
    }
    Ok(determiners)
}

fn split_primary(text: &str) -> Option<(PrimarySignal, &str)> {
    PrimarySignal::ALL
        .into_iter()
        .find_map(|p| text.strip_prefix(p.surface()).map(|rest| (p, rest)))
}

/// Parses a signal tile (already checked by `check_tile_characters`).
pub fn parse_signal(text: &str) -> Result<SignalParse, DiagnosticKind> {
    if let Some(rest) = text.strip_prefix(DEEP_SURFACE) {
        let determiners = parse_determiners(rest)?;
        return Ok(SignalParse::Relational(RelationalSignal::Deep {
            determiners,
        }));
    }
    if let Some(rest) = text.strip_prefix(DefSignal::SURFACE) {
        let (number, tail) = split_number(rest);
        if tail.is_empty() {
            return Ok(SignalParse::Def(DefSignal { number }));
        }
        return Err(if next_determiner(tail).is_some() {
            DiagnosticKind::DeterminerOnDef
        } else {
            DiagnosticKind::UnknownSignalTile
        });
    }
    if let Some((secondary, rest)) = SecondarySignal::ALL
        .into_iter()
        .find_map(|s| text.strip_prefix(s.surface()).map(|r| (s, r)))
    {
        let (primary, tail) =
            split_primary(rest).ok_or(DiagnosticKind::MissingPrimaryAfterSecondary)?;
        let determiners = parse_determiners(tail)?;
        return Ok(SignalParse::Relational(RelationalSignal::Secondary {
            secondary,
            primary,
            determiners,
        }));
    }
    let (signal, tail) = split_primary(text).ok_or(DiagnosticKind::UnknownSignalTile)?;
    let determiners = parse_determiners(tail)?;
    Ok(SignalParse::Relational(RelationalSignal::Primary {
        signal,
        determiners,
    }))
}

#[cfg(test)]
pub fn determiner_surfaces() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    out.extend(Quantifier::ALL.iter().map(|q| q.surface().to_owned()));
    out.push(Reflexive::SURFACE.to_owned());
    out.extend(Comparative::ALL.iter().map(|c| c.surface().to_owned()));
    out.extend(Temporal::ALL.iter().map(|t| t.surface().to_owned()));
    out.extend(
        Temporal::ALL
            .iter()
            .map(|t| format!("{}{}", Conditional::PREFIX, t.surface())),
    );
    out
}
