//! Classification of the tokens in **even slots** (glyphs, anaphoras, raw strings).
//!
//! Q7: a glyph atom is a non-empty sequence of characters `c` with
//! `c.is_alphanumeric() && !c.is_uppercase()`. Raw strings `"…"` are a single
//! token without inner `"` (uppercase allowed). Q10: `base~` = Prefix,
//! `base§mod~` = Inverse, `glyph~~` admits qualifiers.
//! D9: raw strings cannot contain control characters (`Cc`: C0, DEL,
//! C1) nor format/bidi characters (defense against terminal escape-sequence injection
//! and the "Trojan Source" attack).

use crate::prelude::*;

use crate::ast::{
    GlyphAnaphora, GlyphPath, GlyphSlot, NonEmpty, SemaAnaphora, ANAPHORA_MARK,
    QUALIFIER_SEPARATOR, RAW_STRING_QUOTE, SEMA_ANAPHORA_MARK,
};
use crate::error::{DiagnosticKind, GlyphFault, Limit};
use crate::symbol::{Interner, Symbol};
use crate::tiles::is_forbidden_separator;

/// D9: `Cc` plus the format characters that alter the direction or visibility
/// of text (ALM, ZWSP/ZWNJ/ZWJ/LRM/RLM, bidi embedding/override, invisible
/// mathematical operators, bidi isolates, BOM).
fn is_forbidden_raw_character(ch: char) -> bool {
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

fn check_atom(atom: &str) -> Result<(), DiagnosticKind> {
    if atom.is_empty() {
        return Err(DiagnosticKind::MalformedGlyph {
            fault: GlyphFault::EmptySegment,
        });
    }
    for ch in atom.chars() {
        if ch.is_uppercase() {
            return Err(DiagnosticKind::UppercaseCharacter { ch });
        }
        if ch.is_alphanumeric() {
            continue;
        }
        if is_forbidden_separator(ch) {
            return Err(DiagnosticKind::ForbiddenSeparator { ch });
        }
        return Err(DiagnosticKind::MalformedGlyph {
            fault: GlyphFault::IllegalCharacter { ch },
        });
    }
    Ok(())
}

/// `atom { § atom }`. All atoms are validated before any of them is interned.
pub(crate) fn parse_path(text: &str, interner: &mut Interner) -> Result<GlyphPath, DiagnosticKind> {
    for atom in text.split(QUALIFIER_SEPARATOR) {
        check_atom(atom)?;
    }
    let mut symbols = Vec::new();
    for atom in text.split(QUALIFIER_SEPARATOR) {
        symbols.push(intern(interner, atom)?);
    }
    let mut iter = symbols.into_iter();
    let root = iter.next().ok_or(DiagnosticKind::MalformedGlyph {
        fault: GlyphFault::EmptySegment,
    })?;
    Ok(GlyphPath {
        root,
        qualifiers: iter.collect(),
    })
}

/// Classifies the content of an even slot (the caller has already excluded reserved tokens).
pub(crate) fn parse_glyph_slot(
    text: &str,
    interner: &mut Interner,
) -> Result<GlyphSlot, DiagnosticKind> {
    if let Some(inner) = text.strip_prefix(RAW_STRING_QUOTE) {
        let content =
            inner
                .strip_suffix(RAW_STRING_QUOTE)
                .ok_or(DiagnosticKind::MalformedGlyph {
                    fault: GlyphFault::UnterminatedRawString,
                })?;
        if content.contains(RAW_STRING_QUOTE) {
            return Err(DiagnosticKind::MalformedGlyph {
                fault: GlyphFault::QuoteInsideRawString,
            });
        }
        if let Some(ch) = content.chars().find(|c| is_forbidden_raw_character(*c)) {
            return Err(DiagnosticKind::MalformedGlyph {
                fault: GlyphFault::ForbiddenRawCharacter { ch },
            });
        }
        return Ok(GlyphSlot::RawString(intern(interner, content)?));
    }

    if text == SEMA_ANAPHORA_MARK {
        return Ok(GlyphSlot::SemaAnaphora(SemaAnaphora::Latest));
    }
    if let Some(stem) = text.strip_suffix(SEMA_ANAPHORA_MARK) {
        let glyph = parse_path(stem, interner)?;
        return Ok(GlyphSlot::SemaAnaphora(SemaAnaphora::ByGlyph { glyph }));
    }

    if text.len() == ANAPHORA_MARK.len_utf8() && text.starts_with(ANAPHORA_MARK) {
        return Ok(GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact));
    }
    if let Some(stem) = text.strip_suffix(ANAPHORA_MARK) {
        let GlyphPath { root, qualifiers } = parse_path(stem, interner)?;
        let anaphora = match NonEmpty::from_vec(qualifiers.into_vec()) {
            None => GlyphAnaphora::Prefix { base: root },
            Some(qualifiers) => GlyphAnaphora::Inverse {
                base: root,
                qualifiers,
            },
        };
        return Ok(GlyphSlot::GlyphAnaphora(anaphora));
    }

    Ok(GlyphSlot::Glyph(parse_path(text, interner)?))
}
