//! Stage 3 indexed wire form (D4, C.2).
//!
//! The indexed form is the **written** canonical form of the period with every
//! glyph replaced by DTI keys; it does not derive from the expanded form, so every
//! period valid at stage 2 is transmissible (`def` blocks, portability blocks and
//! anaphora included):
//!
//! - glyph tokens (slot glyphs, `def` values) → the key of the whole token when
//!   it is a DTI form (`§` included), otherwise one key per `§` segment
//!   (`<K1>§…§<Kn>`);
//! - labels (v4.1) → `LVAAAA<form>` for a slot marked as a label in the session
//!   (see [`super::label`]); a token already in wire-label form
//!   (`LVA<XYZ><form>`, e.g. received) is kept verbatim, subtype included; every
//!   unmarked slot is a glyph;
//! - **bound symbols** — `def` variable names, their references (R13: a glyph
//!   token resolved by stage 2 as a live variable) and portability ids — live in
//!   the session, not in the static DTI: an enabled primary form keeps its DTI
//!   key, any other symbol gets an ephemeral session key
//!   ([`DtiKey::session_symbol`]);
//! - numerals and the marks `~`, `~~` are kept literally.

use crate::prelude::*;

use core::fmt;

use crate::analyze::{ClauseId, GlobalClause, Resolution, SlotIndex, SlotRef};
use crate::ast::GlyphPath;
use crate::dti::{Dti, DtiKey, FormResolution, KeyError};
use crate::printer::{
    period_clause_tokens, render_period, render_period_with, GlyphMap, GlyphSite,
};
use crate::session::{PeriodId, Session};
use crate::symbol::Symbol;

use super::expand::WireMode;
use super::label::{label_wire, slot_content, SlotContent};
use super::resolve::{resolve_atom, resolve_path, ResolveError};

/// Errors of the indexed rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexError {
    PeriodNotFound(PeriodId),
    Resolve(ResolveError),
    /// The ephemeral key of a session symbol could not be built.
    SessionSymbol(KeyError),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodNotFound(id) => write!(f, "period {} not found", id.index()),
            Self::Resolve(err) => write!(f, "{err}"),
            Self::SessionSymbol(err) => write!(f, "session symbol key: {err}"),
        }
    }
}

impl core::error::Error for IndexError {}

impl From<ResolveError> for IndexError {
    fn from(err: ResolveError) -> Self {
        Self::Resolve(err)
    }
}

/// Key of a bound symbol (`def` name, variable reference, portability-id atom):
/// the DTI key when the atom is the primary form of an enabled concept, otherwise
/// an ephemeral session key. A primary form shared by two enabled concepts in the
/// two languages is ambiguous (B3).
pub(crate) fn symbol_key(dti: &Dti, text: &str) -> Result<DtiKey, IndexError> {
    // A symbol received in indexed form is already a key: it is kept (relay).
    if let Ok(key) = DtiKey::parse(text) {
        return Ok(key);
    }
    match dti.resolve_primary(text) {
        FormResolution::Unique(key) if dti.is_enabled(&key) => Ok(key),
        FormResolution::Ambiguous(keys) if keys.iter().any(|k| dti.is_enabled(k)) => {
            Err(IndexError::Resolve(ResolveError::Ambiguous {
                form: text.to_owned(),
                keys,
            }))
        }
        FormResolution::Unique(_) | FormResolution::Ambiguous(_) | FormResolution::Unknown => {
            DtiKey::session_symbol(text).map_err(IndexError::SessionSymbol)
        }
    }
}

/// `true` when every `§` segment of a glyph is already an enabled DTI key or an
/// ASCII numeral (a period received in indexed form and rendered again, e.g. by
/// a relay): the text is kept as it is.
fn already_indexed(dti: &Dti, text: &str) -> bool {
    let mut has_key = false;
    let all = text.split(crate::ast::QUALIFIER_SEPARATOR).all(|segment| {
        if let Ok(key) = DtiKey::parse(segment) {
            has_key = true;
            dti.is_enabled(&key)
        } else {
            !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit())
        }
    });
    all && has_key
}

/// Glyph map writing DTI keys.
struct IndexedMap<'a> {
    session: &'a Session,
    period: PeriodId,
    dti: &'a Dti,
}

impl IndexedMap<'_> {
    fn text(&self, symbol: Symbol) -> Result<&str, IndexError> {
        self.session
            .interner()
            .resolve(symbol)
            .ok_or(IndexError::Resolve(ResolveError::UnknownSymbol))
    }

    fn symbol(&self, atom: Symbol) -> Result<String, IndexError> {
        Ok(symbol_key(self.dti, self.text(atom)?)?.as_str().to_owned())
    }

    fn slot_ref(&self, clause: u32, pair: u32) -> SlotRef {
        SlotRef {
            clause: GlobalClause {
                period: self.period,
                clause: ClauseId::new(clause),
            },
            slot: SlotIndex::new(pair),
        }
    }

    /// R13: the slot holds a glyph token resolved by stage 2 as a live variable.
    fn is_variable_reference(&self, clause: u32, pair: u32) -> bool {
        self.session
            .analysis(self.period)
            .and_then(|period| period.clause(ClauseId::new(clause)))
            .and_then(|analysis| analysis.slot(SlotIndex::new(pair)))
            .is_some_and(|slot| matches!(slot.resolution(), Resolution::Variable { .. }))
    }
}

impl GlyphMap for IndexedMap<'_> {
    type Error = IndexError;

    fn path(&mut self, path: &GlyphPath, site: GlyphSite) -> Result<String, IndexError> {
        match site {
            GlyphSite::Slot { clause, pair } if self.is_variable_reference(clause, pair) => {
                self.symbol(path.text)
            }
            GlyphSite::PortabilityId => self.symbol(path.text),
            GlyphSite::Slot { clause, pair } => {
                let text = self.text(path.text)?;
                match slot_content(self.session, self.slot_ref(clause, pair), text) {
                    SlotContent::Label => Ok(label_wire(text)),
                    SlotContent::WireLabel => Ok(text.to_owned()),
                    SlotContent::Glyph if already_indexed(self.dti, text) => Ok(text.to_owned()),
                    SlotContent::Glyph => {
                        Ok(resolve_path(path, self.session.interner(), self.dti)?.indexed_text())
                    }
                }
            }
            GlyphSite::DefName | GlyphSite::Standalone => {
                Ok(resolve_path(path, self.session.interner(), self.dti)?.indexed_text())
            }
        }
    }

    fn atom(&mut self, atom: Symbol, site: GlyphSite) -> Result<String, IndexError> {
        match site {
            GlyphSite::DefName | GlyphSite::PortabilityId => self.symbol(atom),
            GlyphSite::Slot { .. } | GlyphSite::Standalone => {
                let text = self.text(atom)?;
                Ok(resolve_atom(self.dti, text)?.to_string())
            }
        }
    }
}

/// Indexed wire form of a period: written canonical form with DTI keys.
pub fn render_indexed_period(
    session: &Session,
    id: PeriodId,
    dti: &Dti,
) -> Result<String, IndexError> {
    let period = session.period(id).ok_or(IndexError::PeriodNotFound(id))?;
    let mut map = IndexedMap {
        session,
        period: id,
        dti,
    };
    render_period_with(period, &mut map)
}

/// Lines `(depth, tokens)` of the indexed form (used by the `DtiIndexed` view).
pub(crate) fn indexed_lines(
    session: &Session,
    id: PeriodId,
    dti: &Dti,
) -> Result<Vec<(u8, Vec<String>)>, IndexError> {
    let period = session.period(id).ok_or(IndexError::PeriodNotFound(id))?;
    let analysis = session.analysis(id).ok_or(IndexError::PeriodNotFound(id))?;
    let mut map = IndexedMap {
        session,
        period: id,
        dti,
    };
    let lines = period_clause_tokens(period, &mut map)?;
    Ok(analysis
        .clauses()
        .iter()
        .map(|clause| clause.depth())
        .zip(lines)
        .collect())
}

/// Renders a period for the given wire mode (both modes keep the written structure).
pub fn render_wire(
    session: &Session,
    id: PeriodId,
    dti: &Dti,
    mode: WireMode,
) -> Result<String, IndexError> {
    match mode {
        WireMode::Lexical => {
            let period = session.period(id).ok_or(IndexError::PeriodNotFound(id))?;
            render_period(period, session.interner())
                .map_err(|_| IndexError::Resolve(ResolveError::UnknownSymbol))
        }
        WireMode::Indexed => render_indexed_period(session, id, dti),
    }
}
