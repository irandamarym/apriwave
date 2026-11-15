//! Suggestion menu (v4.1, SAP / Excel style): the most recent glyphs and semas
//! of the session that start with the typed prefix, plus the Glyph / Label
//! disambiguation of the local DTI.
//!
//! The front end opens the menu under the text while typing
//! (`autocomplete = "auto"`) or on request (`autocomplete = "manual"`, chord
//! `suggest`, `Alt+Down` by default). Selecting an entry inserts
//! [`Suggestion::text`]: a glyph or a label as written, a sema as the canonical
//! text of the whole clause. When the chosen entry is a glyph or a label of a
//! form that the local DTI holds as both, the front end records the choice
//! with [`crate::Session::set_slot_kind`] once the period is ingested.

use crate::analyze::{ClauseId, GlobalClause, SlotIndex, SlotRef};
use crate::ast::{ClauseRef, DefOperation, GlyphSlot, RelationalBlock};
use crate::dti::Dti;
use crate::printer::{self, render_glyph_slot};
use crate::session::{PeriodId, Session, SlotKind};
use crate::stage3::is_glyph_form;

/// Maximum number of entries returned by [`suggestions`].
pub const MAX_SUGGESTIONS: usize = 32;

/// What a suggestion inserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    /// A glyph (highlighted with the glyph colour).
    Glyph,
    /// A label of the local DTI (highlighted with the label colour).
    Label,
    /// A whole sema: the canonical text of a relational clause.
    Sema,
}

/// One entry of the suggestion menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    /// Text inserted when the entry is selected.
    pub text: String,
}

struct Collector<'d> {
    prefix: &'d str,
    dti: Option<&'d Dti>,
    out: Vec<Suggestion>,
}

impl Collector<'_> {
    fn full(&self) -> bool {
        self.out.len() >= MAX_SUGGESTIONS
    }

    fn push(&mut self, kind: SuggestionKind, text: &str) {
        if self.full() || self.out.iter().any(|s| s.kind == kind && s.text == text) {
            return;
        }
        self.out.push(Suggestion {
            kind,
            text: text.to_owned(),
        });
    }

    /// A glyph token: the user's choice when recorded, otherwise every kind the
    /// local DTI allows (both entries when the form is a glyph and a label).
    fn token(&mut self, text: &str, choice: Option<SlotKind>) {
        if !text.starts_with(self.prefix) {
            return;
        }
        match choice {
            Some(SlotKind::Glyph) => self.push(SuggestionKind::Glyph, text),
            Some(SlotKind::Label) => self.push(SuggestionKind::Label, text),
            None => self.form(text),
        }
    }

    /// Every kind of a form according to the local DTI.
    fn form(&mut self, text: &str) {
        match self.dti {
            Some(dti) if dti.label(text).is_some() => {
                if is_glyph_form(dti, text) {
                    self.push(SuggestionKind::Glyph, text);
                }
                self.push(SuggestionKind::Label, text);
            }
            Some(_) | None => self.push(SuggestionKind::Glyph, text),
        }
    }
}

/// Canonical text of a relational clause (signal pair, then the case pairs in
/// written order), without its connector.
fn sema_text(session: &Session, block: &RelationalBlock) -> Option<String> {
    let interner = session.interner();
    let mut tokens = vec![
        printer::signal(&block.signal.tile.node),
        render_glyph_slot(&block.signal.glyph.node, interner).ok()?,
    ];
    for pair in &block.cases {
        tokens.push(printer::case(pair.tile.node));
        tokens.push(render_glyph_slot(&pair.glyph.node, interner).ok()?);
    }
    Some(tokens.join(" "))
}

/// Glyph slots of a clause with their pair position, most recent first.
fn glyph_slots(block: ClauseRef<'_>) -> Vec<(u32, &GlyphSlot)> {
    let mut slots: Vec<(u32, &GlyphSlot)> = match block {
        ClauseRef::Relational(block) => (0_u32..)
            .zip(
                std::iter::once(&block.signal.glyph.node)
                    .chain(block.cases.iter().map(|pair| &pair.glyph.node)),
            )
            .collect(),
        ClauseRef::Definition(block) => (1_u32..)
            .zip(block.operations.iter())
            .filter_map(|(position, op)| match op {
                DefOperation::Assign { value, .. }
                | DefOperation::Modify { value, .. }
                | DefOperation::Union { value, .. } => Some((position, &value.node)),
                DefOperation::Unbind { .. } => None,
            })
            .collect(),
        ClauseRef::Portability(_) => Vec::new(),
    };
    slots.reverse();
    slots
}

/// Suggestions for `prefix`, in this order: the glyphs of the session (most
/// recent first, `def` names included), the labels of the local DTI, then the
/// semas of the session (most recent first) whose canonical text or signal
/// glyph starts with `prefix`. A form that the local DTI holds both as a glyph
/// and as a label yields both entries (disambiguation menu). At most
/// [`MAX_SUGGESTIONS`] entries, without duplicates.
pub fn suggestions(prefix: &str, session: &Session, dti: Option<&Dti>) -> Vec<Suggestion> {
    let mut collector = Collector {
        prefix,
        dti,
        out: Vec::new(),
    };
    let interner = session.interner();
    let mut semas: Vec<String> = Vec::new();
    for period_id in session.period_ids().rev() {
        let Some(period) = session.period(period_id) else {
            continue;
        };
        let count = u32::try_from(period.clause_count()).unwrap_or(u32::MAX);
        for index in (0..count).rev() {
            let Some(block) = period.clause(index) else {
                continue;
            };
            collect_clause(&mut collector, session, period_id, index, block, &mut semas);
            if let ClauseRef::Definition(def) = block {
                if let Some(name) = interner.resolve(def.name.node) {
                    collector.token(name, None);
                }
            }
        }
    }
    if let Some(dti) = dti {
        for (_, form) in dti.overlay().labels().iter() {
            if form.starts_with(prefix) {
                collector.form(form);
            }
        }
    }
    for sema in semas {
        collector.push(SuggestionKind::Sema, &sema);
    }
    collector.out
}

fn collect_clause(
    collector: &mut Collector<'_>,
    session: &Session,
    period: PeriodId,
    index: u32,
    block: ClauseRef<'_>,
    semas: &mut Vec<String>,
) {
    let clause = GlobalClause {
        period,
        clause: ClauseId::new(index),
    };
    for (position, slot) in glyph_slots(block) {
        let GlyphSlot::Glyph(path) = slot else {
            continue;
        };
        let Some(text) = session.interner().resolve(path.text) else {
            continue;
        };
        let choice = session.slot_kind(SlotRef {
            clause,
            slot: SlotIndex::new(position),
        });
        collector.token(text, choice);
    }
    if let ClauseRef::Relational(block) = block {
        let signal_glyph = render_glyph_slot(&block.signal.glyph.node, session.interner());
        let Some(text) = sema_text(session, block) else {
            return;
        };
        let matches = text.starts_with(collector.prefix)
            || signal_glyph.is_ok_and(|glyph| glyph.starts_with(collector.prefix));
        if matches && !semas.contains(&text) {
            semas.push(text);
        }
    }
}
