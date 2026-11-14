//! Checks **local** to a single block (independent of the DSP context).
//!
//! - Q21: duplicate cases → `ERR_CASE_AMBIGUITY` (except `absi`).
//! - M§7: `ref` written in the block + Accusative → `ERR_SYNTAX`.
//!   (The `ref` inherited via Q17 is checked by stage 2.)
//! - Q19: comparative written without any pair in the Accusative → `ERR_SEMANTIC`
//!   (the comparative is not inherited, Q17, so the local check is complete).
//! - Q11: constraints of `def` blocks.

use crate::prelude::*;

use alloc::collections::BTreeSet;

use crate::ast::{
    AblativeDeterminer, Case, CaseTile, DefOperation, DefSignal, DefinitionBlock, GlyphAnaphora,
    GlyphSlot, NonEmpty, Pair, RelationalBlock,
};
use crate::error::{Diagnostic, DiagnosticKind};
use crate::span::Spanned;

const COMITATIVE: Case = Case::Ablative(Some(AblativeDeterminer::Si));

fn duplicate_cases<'a>(
    cases: impl Iterator<Item = &'a Spanned<CaseTile>>,
    out: &mut Vec<Diagnostic>,
) {
    let mut seen: BTreeSet<Case> = BTreeSet::new();
    for tile in cases {
        // `abdur=3` and `abdur=5` are the same case (the count belongs to the tile).
        let case = tile.node.case.identity();
        if case == COMITATIVE {
            continue;
        }
        if !seen.insert(case) {
            out.push(Diagnostic::new(DiagnosticKind::DuplicateCase, tile.span));
        }
    }
}

pub(crate) fn relational(block: &RelationalBlock, out: &mut Vec<Diagnostic>) {
    duplicate_cases(block.cases.iter().map(|pair| &pair.tile), out);

    let determiners = block.signal.tile.node.determiners();
    if determiners.reflexive.is_some() {
        for pair in &block.cases {
            if pair.tile.node.case == Case::Accusative {
                out.push(Diagnostic::new(
                    DiagnosticKind::AccusativeInReflexive,
                    pair.tile.span,
                ));
            }
        }
    }
    if determiners.comparative.is_some()
        && !block
            .cases
            .iter()
            .any(|pair| pair.tile.node.case == Case::Accusative)
    {
        out.push(Diagnostic::new(
            DiagnosticKind::ComparativeWithoutAccusative,
            block.signal.tile.span,
        ));
    }
}

/// Builds a `DefinitionBlock` applying Q11; `None` if a diagnostic was emitted.
pub(crate) fn definition(
    signal: Spanned<DefSignal>,
    name_glyph: Spanned<GlyphSlot>,
    cases: Vec<Pair<CaseTile>>,
    out: &mut Vec<Diagnostic>,
) -> Option<DefinitionBlock> {
    let before = out.len();

    let name = match name_glyph.node {
        GlyphSlot::Glyph(path) if path.is_atomic() => {
            Some(Spanned::new(path.root, name_glyph.span))
        }
        _ => {
            out.push(Diagnostic::new(
                DiagnosticKind::DefNameNotAtomic,
                name_glyph.span,
            ));
            None
        }
    };

    if cases.is_empty() {
        out.push(Diagnostic::new(
            DiagnosticKind::DefWithoutOperations,
            signal.span,
        ));
    }

    duplicate_cases(cases.iter().map(|pair| &pair.tile), out);

    let mut operations = Vec::with_capacity(cases.len());
    for Pair { tile: case, glyph } in cases {
        let operation = match (case.node.case, case.node.number) {
            (Case::Accusative, number) => Some(DefOperation::Assign {
                tile: Spanned::new(number, case.span),
                value: glyph,
            }),
            (Case::Ablative(Some(AblativeDeterminer::Fi)), None) => Some(DefOperation::Modify {
                tile: case.span,
                value: glyph,
            }),
            (Case::Ablative(Some(AblativeDeterminer::Si)), None) => Some(DefOperation::Union {
                tile: case.span,
                value: glyph,
            }),
            (Case::Ablative(Some(AblativeDeterminer::Se)), None) => match glyph.node {
                GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact) => Some(DefOperation::Unbind {
                    tile: case.span,
                    tilde: glyph.span,
                }),
                _ => {
                    out.push(Diagnostic::new(
                        DiagnosticKind::UnbindRequiresTilde,
                        glyph.span,
                    ));
                    None
                }
            },
            _ => {
                out.push(Diagnostic::new(
                    DiagnosticKind::DefOperationNotAllowed,
                    case.span,
                ));
                None
            }
        };
        if let Some(op) = operation {
            operations.push(op);
        }
    }

    if out.len() != before {
        return None;
    }
    let name = name?;
    let operations = NonEmpty::from_vec(operations)?;
    Some(DefinitionBlock {
        signal,
        name,
        operations,
    })
}
