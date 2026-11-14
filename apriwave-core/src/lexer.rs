//! Lexer: **segmentation only** (§5.1 of the design).
//!
//! Token classification depends on slot parity (`a` is Accusative in an
//! odd position and a glyph in an even position), so it is the parser's job.
//! Q12: the only canonical separator is the single ASCII space; multiple spaces,
//! leading/trailing spaces, `\t`, `\n` and any other Unicode whitespace produce
//! `InvalidWhitespace` (the lexer keeps going anyway to collect diagnostics).

use crate::prelude::*;

use crate::error::{Diagnostic, DiagnosticKind, Limit};
use crate::span::Span;

/// Raw token: text borrowed from the source, plus its position.
/// `Copy` is justified: two words + `Span`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawToken<'src> {
    pub text: &'src str,
    pub span: Span,
}

#[derive(Debug)]
pub struct Lexed<'src> {
    pub tokens: Vec<RawToken<'src>>,
    pub diagnostics: Vec<Diagnostic>,
    /// Empty span at the end of the input (for unexpected-end diagnostics).
    pub end: Span,
}

fn limit_error() -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::LimitExceeded {
            limit: Limit::InputSize,
        },
        Span::new(0, 0),
    )
}

fn offset(value: usize) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| limit_error())
}

/// Segments `src`. Blocking error only if the input exceeds `u32::MAX` bytes.
pub fn lex(src: &str) -> Result<Lexed<'_>, Diagnostic> {
    let end_offset = offset(src.len())?;
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    // ' ' separator that closed a non-empty token: if it is the last character, it is trailing.
    let mut pending_trailing: Option<Span> = None;
    let mut cursor: usize = 0;

    for piece in src.split_inclusive(char::is_whitespace) {
        let piece_start = cursor;
        cursor = cursor.checked_add(piece.len()).ok_or_else(limit_error)?;

        let (text, separator) = match piece.chars().next_back() {
            Some(last) if last.is_whitespace() => match piece.strip_suffix(last) {
                Some(stripped) => (stripped, Some(last)),
                None => (piece, None),
            },
            _ => (piece, None),
        };

        let text_end = piece_start
            .checked_add(text.len())
            .ok_or_else(limit_error)?;
        if !text.is_empty() {
            tokens.push(RawToken {
                text,
                span: Span::new(offset(piece_start)?, offset(text_end)?),
            });
        }

        pending_trailing = None;
        if let Some(sep) = separator {
            let sep_span = Span::new(offset(text_end)?, offset(cursor)?);
            if sep != ' ' {
                diagnostics.push(Diagnostic::new(DiagnosticKind::InvalidWhitespace, sep_span));
            } else if text.is_empty() {
                // Leading or double space.
                diagnostics.push(Diagnostic::new(DiagnosticKind::InvalidWhitespace, sep_span));
            } else {
                pending_trailing = Some(sep_span);
            }
        }
    }

    if let Some(span) = pending_trailing {
        diagnostics.push(Diagnostic::new(DiagnosticKind::InvalidWhitespace, span));
    }

    Ok(Lexed {
        tokens,
        diagnostics,
        end: Span::new(end_offset, end_offset),
    })
}
