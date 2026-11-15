//! Checks **local** to a single block (independent of the DSP context).
//!
//! - v4.1: repeated cases are allowed (several pairs with the same case keep the
//!   written order); `ref` is compatible with the Accusative and with every
//!   comparative.
//! - Q19: a comparative **written** on the clause requires at least one pair in
//!   the Accusative → `ERR_SEMANTIC` (an inherited comparative does not).
//! - Q11: constraints of `def` blocks.

use crate::prelude::*;

use crate::ast::{
    AblativeDeterminer, Case, CaseTile, DefOperation, DefSignal, DefinitionBlock, GlyphAnaphora,
    GlyphSlot, NonEmpty, Pair, RelationalBlock,
};
use crate::error::{Diagnostic, DiagnosticKind};
use crate::span::Spanned;

pub(crate) fn relational(block: &RelationalBlock, out: &mut Vec<Diagnostic>) {
    let determiners = block.signal.tile.node.determiners();
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

    // The name is the whole glyph token (any visible characters, `§` included).
    let name = match name_glyph.node {
        GlyphSlot::Glyph(path) => Some(Spanned::new(path.text, name_glyph.span)),
        GlyphSlot::GlyphAnaphora(_) | GlyphSlot::SemaAnaphora(_) => {
            out.push(Diagnostic::new(
                DiagnosticKind::DefNameNotAGlyph,
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
