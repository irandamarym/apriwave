//! Classification of the tokens in **even slots** (v4.1).
//!
//! Slot Locking (`1 Tile -> 1 Glyph`), one rule for every token whatever its
//! characters: the role of an even-slot token is fixed by the annotation of
//! its slot. A slot annotated as a glyph or a label is parsed with
//! [`parse_path`] (a glyph path, `~` / `~~` included); an unannotated slot is
//! classified by [`parse_glyph_slot`], which gives the anaphora role to the
//! exact tokens `~` / `~~` and the glyph role to every other token. A glyph may contain any
//! visible character (any alphabet, uppercase, punctuation, symbols, `§`, `"`,
//! period delimiters, `:`…); the grammar never judges its content.
//! D9: only invisible characters are refused: control characters (`Cc`) and the
//! format/bidi characters (defense against terminal escape-sequence injection and
//! the "Trojan Source" attack).

use crate::prelude::*;

use crate::ast::{
    GlyphAnaphora, GlyphPath, GlyphSlot, SemaAnaphora, ANAPHORA_MARK, QUALIFIER_SEPARATOR,
    SEMA_ANAPHORA_MARK,
};
use crate::error::{DiagnosticKind, GlyphFault, Limit};
use crate::symbol::{Interner, Symbol};

/// D9: `Cc` plus the format characters that alter the direction or visibility
/// of text (ALM, ZWSP/ZWNJ/ZWJ/LRM/RLM, bidi embedding/override, invisible
/// mathematical operators, bidi isolates, BOM).
pub(crate) fn is_invisible_character(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

fn intern(interner: &mut Interner, text: &str) -> Result<Symbol, DiagnosticKind> {
    interner
        .intern(text)
        .map_err(|_| DiagnosticKind::LimitExceeded {
            limit: Limit::SymbolSpace,
        })
}

/// D9 check of a whole glyph token.
pub(crate) fn check_glyph_text(text: &str) -> Result<(), DiagnosticKind> {
    match text.chars().find(|c| is_invisible_character(*c)) {
        Some(ch) => Err(DiagnosticKind::MalformedGlyph {
            fault: GlyphFault::InvisibleCharacter { ch },
        }),
        None => Ok(()),
    }
}

/// A glyph token with its `§` segments (possibly empty). The whole token is
/// validated before anything is interned.
pub(crate) fn parse_path(text: &str, interner: &mut Interner) -> Result<GlyphPath, DiagnosticKind> {
    check_glyph_text(text)?;
    let whole = intern(interner, text)?;
    let mut segments = Vec::new();
    for segment in text.split(QUALIFIER_SEPARATOR) {
        segments.push(intern(interner, segment)?);
    }
    let mut iter = segments.into_iter();
    let root = iter.next().unwrap_or(whole);
    Ok(GlyphPath {
        text: whole,
        root,
        qualifiers: iter.collect(),
    })
}

/// Classifies the content of an **unannotated** even slot (its stage-1 role):
/// anaphora for the exact tokens `~` / `~~`, glyph for every other token.
/// Annotated slots never come here: the parser reads them with [`parse_path`].
pub(crate) fn parse_glyph_slot(
    text: &str,
    interner: &mut Interner,
) -> Result<GlyphSlot, DiagnosticKind> {
    if text == SEMA_ANAPHORA_MARK {
        return Ok(GlyphSlot::SemaAnaphora(SemaAnaphora::Latest));
    }
    if text.len() == ANAPHORA_MARK.len_utf8() && text.starts_with(ANAPHORA_MARK) {
        return Ok(GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact));
    }
    Ok(GlyphSlot::Glyph(parse_path(text, interner)?))
}
