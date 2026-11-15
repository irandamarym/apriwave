//! Stage 3 indexed-input validation (C.2) and human views (C.3).

use crate::prelude::*;

use core::fmt;

use crate::analyze::{GlobalClause, Resolution, SlotIndex, SlotRef};
use crate::ast::{
    ClauseRef, DefOperation, GlyphPath, GlyphSlot, Link, LinkedClause, PortabilityBlock,
    ANAPHORA_MARK,
};
use crate::dti::{Dti, DtiKey, KeyError, KeyResolution};
use crate::printer::{self, render_glyph_path, render_glyph_slot};
use crate::session::{PeriodId, Session};
use crate::symbol::{Interner, Symbol};

use super::indexed::{indexed_lines, IndexError};
use super::label::{is_glyph_form, parse_label_wire, LabelSubtype};
use super::resolve::{numeral, resolve_path, IndexedAtom, MorphFlag, PathResolution, ResolveError};
use crate::session::SlotKind;

// ═══════════════════════════ Indexed input (C.2) ═══════════════════════════

/// One key found in a period received in indexed mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedEntry {
    pub slot: SlotRef,
    pub key: DtiKey,
    /// Enabled, or degraded to an enabled ancestor (D3).
    pub resolution: KeyResolution,
}

/// One label (`LVA<XYZ><form>`, v4.1) found in a period received in indexed
/// mode. A label is accepted even when its form is absent from the local DTI
/// and whatever its subtype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedLabel {
    pub slot: SlotRef,
    pub form: String,
    /// `Known` (`AAA` or registered in the session) or `Dynamic` (first seen).
    pub subtype: LabelSubtype,
    /// The form is also a glyph of the local DTI (homonymy to report to the user).
    pub homonym: bool,
}

/// Informative note: a label subtype not known by the receiving session
/// (registered by [`receive_indexed_input`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewLabelSubtype {
    /// First slot where the subtype appears in the period.
    pub slot: SlotRef,
    pub code: [u8; 3],
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct IndexedReport {
    pub entries: Vec<IndexedEntry>,
    pub labels: Vec<IndexedLabel>,
    /// Subtypes seen for the first time (informative notes, never errors).
    pub new_subtypes: Vec<NewLabelSubtype>,
}

/// Problems of a period received in indexed mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexedInputError {
    PeriodNotFound(PeriodId),
    /// A glyph atom that is not a DTI key (atoms starting with a letter).
    NotAKey {
        slot: SlotRef,
        atom: String,
        error: KeyError,
    },
    /// An atom starting with a digit that is not a valid numeral (B4, B9).
    InvalidNumeral {
        slot: SlotRef,
        atom: String,
    },
    UnknownKey {
        slot: SlotRef,
        key: DtiKey,
    },
    RejectedKey {
        slot: SlotRef,
        key: DtiKey,
    },
    /// A token starting with `LVA` that is truncated or corrupted (shorter than
    /// 7 characters, or subtype not `[A-Z]{3}`).
    MalformedLabel {
        slot: SlotRef,
        token: String,
    },
    /// A morphology permission is missing: `+UNION` on a concept joined by `§`,
    /// `+AGGL_NUM` on a digit (B9).
    MorphViolation {
        slot: SlotRef,
        form: String,
        key: DtiKey,
        flag: MorphFlag,
    },
    UnknownSymbol,
}

impl fmt::Display for IndexedInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodNotFound(id) => write!(f, "period {} not found", id.index()),
            Self::NotAKey { atom, error, .. } => write!(f, "'{atom}' is not a DTI key: {error}"),
            Self::InvalidNumeral { atom, .. } => write!(f, "invalid numeral '{atom}'"),
            Self::UnknownKey { key, .. } => write!(f, "unknown key {key}"),
            Self::RejectedKey { key, .. } => write!(f, "key {key} rejected by the local policy"),
            Self::MalformedLabel { token, .. } => write!(f, "malformed label token '{token}'"),
            Self::MorphViolation {
                form, key, flag, ..
            } => write!(f, "'{form}' requires {flag} on concept {key}"),
            Self::UnknownSymbol => f.write_str("symbol not found in the interner"),
        }
    }
}

impl core::error::Error for IndexedInputError {}

fn path_atoms(path: &GlyphPath) -> Vec<Symbol> {
    core::iter::once(path.root)
        .chain(path.qualifiers.iter().copied())
        .collect()
}

/// Glyph atoms of an operand and whether they form a `§` union.
struct OperandAtoms {
    atoms: Vec<Symbol>,
    union: bool,
    /// Bound session symbol (`def` name, R13 variable reference, portability
    /// id): any valid key is accepted, the static DTI is not required.
    bound: bool,
    /// Whole token, checked first as a label (`LVA…`, v4.1).
    token: Option<Symbol>,
}

fn path_operand(path: &GlyphPath) -> OperandAtoms {
    OperandAtoms {
        atoms: path_atoms(path),
        union: !path.is_atomic(),
        bound: false,
        token: Some(path.text),
    }
}

fn bound_operand(atoms: Vec<Symbol>) -> OperandAtoms {
    OperandAtoms {
        atoms,
        union: false,
        bound: true,
        token: None,
    }
}

/// Glyph atoms carried by a slot (the markers `~`/`~~` carry none).
fn slot_atoms(slot: &GlyphSlot) -> OperandAtoms {
    match slot {
        GlyphSlot::Glyph(path) => path_operand(path),
        GlyphSlot::GlyphAnaphora(_) | GlyphSlot::SemaAnaphora(_) => OperandAtoms {
            atoms: Vec::new(),
            union: false,
            bound: false,
            token: None,
        },
    }
}

/// Atoms of every glyph operand of a clause, by pair position: relational slots
/// (`is_variable(pair)`: stage 2 resolved the slot as a live variable, R13), `def`
/// name (pair 0) and values, portability id (pair 0).
fn clause_atoms(
    block: ClauseRef<'_>,
    is_variable: impl Fn(u32) -> bool,
) -> Vec<(u32, OperandAtoms)> {
    match block {
        ClauseRef::Relational(block) => (0..u32::try_from(block.pair_count()).unwrap_or(0))
            .filter_map(|p| {
                block.glyph(p).map(|g| match &g.node {
                    GlyphSlot::Glyph(path) if is_variable(p) => (p, bound_operand(vec![path.text])),
                    other => (p, slot_atoms(other)),
                })
            })
            .collect(),
        ClauseRef::Definition(block) => core::iter::once((0, bound_operand(vec![block.name.node])))
            .chain(
                block
                    .operations
                    .iter()
                    .zip(1_u32..)
                    .filter_map(|(op, p)| match op {
                        DefOperation::Assign { value, .. }
                        | DefOperation::Modify { value, .. }
                        | DefOperation::Union { value, .. } => Some((p, slot_atoms(&value.node))),
                        DefOperation::Unbind { .. } => None,
                    }),
            )
            .collect(),
        ClauseRef::Portability(block) => match block {
            PortabilityBlock::Anchor { id, .. }
            | PortabilityBlock::AnchorRel { id, .. }
            | PortabilityBlock::Rel { id, .. }
            | PortabilityBlock::RelOff { id, .. } => vec![(0, bound_operand(vec![id.node.text]))],
            PortabilityBlock::End { .. } => Vec::new(),
        },
    }
}

/// Validates a period received in indexed mode. A slot token starting with
/// `LVA` is always a label (v4.1): accepted even when absent from the local
/// DTI and whatever its subtype (a subtype other than `AAA` was generated by
/// the sending machine), reported in [`IndexedReport::labels`] with its
/// subtype and its homonymy with a local glyph; only a truncated or corrupted
/// token is an error ([`IndexedInputError::MalformedLabel`]). Every other glyph atom (slot glyphs, `def` values) must be a DTI key,
/// resolved with the local policy (D3), or — when it starts with a digit — a
/// valid numeral (B4). Bound session symbols — `def` names, R13 variable
/// references and portability ids — are local to the session: any valid key is
/// accepted (ephemeral keys included) without requiring the static DTI.
pub fn validate_indexed_input(
    session: &Session,
    id: PeriodId,
    dti: &Dti,
) -> Result<IndexedReport, Vec<IndexedInputError>> {
    let period = session
        .period(id)
        .ok_or_else(|| vec![IndexedInputError::PeriodNotFound(id)])?;
    let interner = session.interner();
    let mut report = IndexedReport::default();
    let mut errors = Vec::new();

    for index in 0..u32::try_from(period.clause_count()).unwrap_or(u32::MAX) {
        let Some(block) = period.clause(index) else {
            break;
        };
        let clause = GlobalClause {
            period: id,
            clause: crate::analyze::ClauseId::new(index),
        };
        let analysis = session
            .analysis(id)
            .and_then(|period| period.clause(clause.clause));
        let is_variable = |pair: u32| {
            analysis
                .and_then(|a| a.slot(SlotIndex::new(pair)))
                .is_some_and(|slot| matches!(slot.resolution(), Resolution::Variable { .. }))
        };
        for (position, operand) in clause_atoms(block, is_variable) {
            let slot_ref = SlotRef {
                clause,
                slot: SlotIndex::new(position),
            };
            let token = operand.token.and_then(|symbol| interner.resolve(symbol));
            if let Some(label) = token.and_then(parse_label_wire) {
                match label {
                    Ok(label) => {
                        let subtype = if session.is_known_label_subtype(label.code) {
                            LabelSubtype::Known(label.code)
                        } else {
                            if !report.new_subtypes.iter().any(|n| n.code == label.code) {
                                report.new_subtypes.push(NewLabelSubtype {
                                    slot: slot_ref,
                                    code: label.code,
                                });
                            }
                            LabelSubtype::Dynamic(label.code)
                        };
                        report.labels.push(IndexedLabel {
                            slot: slot_ref,
                            form: label.form.to_owned(),
                            subtype,
                            homonym: is_glyph_form(dti, label.form),
                        });
                    }
                    Err(_) => errors.push(IndexedInputError::MalformedLabel {
                        slot: slot_ref,
                        token: token.unwrap_or_default().to_owned(),
                    }),
                }
                continue;
            }
            let mut resolved: Vec<(DtiKey, &str)> = Vec::new();
            for symbol in operand.atoms {
                let Some(atom) = interner.resolve(symbol) else {
                    errors.push(IndexedInputError::UnknownSymbol);
                    continue;
                };
                if operand.bound {
                    if let Err(error) = DtiKey::parse(atom) {
                        errors.push(IndexedInputError::NotAKey {
                            slot: slot_ref,
                            atom: atom.to_owned(),
                            error,
                        });
                    }
                    continue;
                }
                // A key always starts with a letter: a leading digit is a numeral.
                if atom.bytes().next().is_some_and(|b| b.is_ascii_digit()) {
                    match numeral(dti, atom) {
                        Some(Ok(_)) => {}
                        Some(Err(ResolveError::MorphViolation { form, key, flag })) => {
                            errors.push(IndexedInputError::MorphViolation {
                                slot: slot_ref,
                                form,
                                key,
                                flag,
                            });
                        }
                        Some(Err(_)) | None => errors.push(IndexedInputError::InvalidNumeral {
                            slot: slot_ref,
                            atom: atom.to_owned(),
                        }),
                    }
                    continue;
                }
                let key = match DtiKey::parse(atom) {
                    Ok(key) => key,
                    Err(error) => {
                        errors.push(IndexedInputError::NotAKey {
                            slot: slot_ref,
                            atom: atom.to_owned(),
                            error,
                        });
                        continue;
                    }
                };
                match dti.resolve_key(&key) {
                    resolution @ (KeyResolution::Enabled(_) | KeyResolution::Degraded { .. }) => {
                        resolved.push((key, atom));
                        report.entries.push(IndexedEntry {
                            slot: slot_ref,
                            key,
                            resolution,
                        });
                    }
                    KeyResolution::Unknown => {
                        errors.push(IndexedInputError::UnknownKey {
                            slot: slot_ref,
                            key,
                        });
                    }
                    KeyResolution::Rejected => {
                        errors.push(IndexedInputError::RejectedKey {
                            slot: slot_ref,
                            key,
                        });
                    }
                }
            }
            // B9: every concept joined by `§` must carry `+UNION`.
            if operand.union {
                for (key, atom) in resolved {
                    if !MorphFlag::Union.allowed_by(dti, &key) {
                        errors.push(IndexedInputError::MorphViolation {
                            slot: slot_ref,
                            form: atom.to_owned(),
                            key,
                            flag: MorphFlag::Union,
                        });
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(report)
    } else {
        Err(errors)
    }
}

/// Receives a period in indexed mode: [`validate_indexed_input`], then
/// registers every new label subtype in the session (informative notes in
/// [`IndexedReport::new_subtypes`], the communication is never interrupted) and
/// marks the label slots as [`SlotKind::Label`]. On error the session is left
/// unchanged.
pub fn receive_indexed_input(
    session: &mut Session,
    id: PeriodId,
    dti: &Dti,
) -> Result<IndexedReport, Vec<IndexedInputError>> {
    let report = validate_indexed_input(session, id, dti)?;
    for note in &report.new_subtypes {
        session.register_label_subtype(note.code);
    }
    for label in &report.labels {
        session.set_slot_kind(label.slot, SlotKind::Label);
    }
    Ok(report)
}

// ═══════════════════════════ Views (C.3) ═══════════════════════════

/// Human consultation modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDisplayMode {
    /// Written structure with human forms (display forms with spaces when declared).
    HumanOriginal,
    /// Indexed wire form (written structure, keys in every glyph position).
    DtiIndexed,
    /// Written structure with primary lexical forms (`verde§smeraldo`).
    CanonicalLexical,
}

/// Layout of a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Single line.
    Linear,
    /// One clause per line, indented by `(depth + 1) × indent` spaces.
    CStyle { indent: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    PeriodNotFound(PeriodId),
    /// `DtiIndexed` needs a DTI.
    MissingDti,
    Index(IndexError),
    UnknownSymbol,
}

impl fmt::Display for ViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeriodNotFound(id) => write!(f, "period {} not found", id.index()),
            Self::MissingDti => f.write_str("the indexed view requires a DTI"),
            Self::Index(err) => write!(f, "{err}"),
            Self::UnknownSymbol => f.write_str("symbol not found in the interner"),
        }
    }
}

impl core::error::Error for ViewError {}

/// Lexical form of a resolved path in the requested mode.
fn lexical_form(resolution: &PathResolution, dti: &Dti, human: bool) -> Option<String> {
    let form = |key: &DtiKey| -> Option<String> {
        let display = if human { dti.display_form(key) } else { None };
        display.or_else(|| dti.primary_form(key)).map(str::to_owned)
    };
    match resolution {
        PathResolution::Record(key) => form(key),
        PathResolution::Segments(atoms) => {
            let parts = atoms
                .iter()
                .map(|atom| match atom {
                    IndexedAtom::Key(key) => form(key),
                    IndexedAtom::Numeral(digits) => Some(digits.as_str().to_owned()),
                })
                .collect::<Option<Vec<String>>>()?;
            Some(parts.join("§"))
        }
    }
}

/// Written glyph text, optionally mapped to its lexical form (falls back to the
/// written text when the DTI cannot resolve it).
fn written_path(
    path: &GlyphPath,
    interner: &Interner,
    dti: Option<&Dti>,
    human: bool,
) -> Result<String, ViewError> {
    let written = render_glyph_path(path, interner).map_err(|_| ViewError::UnknownSymbol)?;
    let mapped = dti.and_then(|dti| {
        resolve_path(path, interner, dti)
            .ok()
            .and_then(|resolution| lexical_form(&resolution, dti, human))
    });
    Ok(mapped.unwrap_or(written))
}

fn written_slot(
    slot: &GlyphSlot,
    interner: &Interner,
    dti: Option<&Dti>,
    human: bool,
) -> Result<String, ViewError> {
    match slot {
        GlyphSlot::Glyph(path) => written_path(path, interner, dti, human),
        other => render_glyph_slot(other, interner).map_err(|_| ViewError::UnknownSymbol),
    }
}

/// Lines `(depth, tokens)` of the written structure.
fn written_lines(
    session: &Session,
    id: PeriodId,
    dti: Option<&Dti>,
    human: bool,
) -> Result<Vec<(u8, Vec<String>)>, ViewError> {
    let period = session.period(id).ok_or(ViewError::PeriodNotFound(id))?;
    let analysis = session.analysis(id).ok_or(ViewError::PeriodNotFound(id))?;
    let interner = session.interner();
    let mut lines = Vec::with_capacity(period.clause_count());
    for (offset, clause_analysis) in analysis.clauses().iter().enumerate() {
        let index = u32::try_from(offset).map_err(|_| ViewError::PeriodNotFound(id))?;
        let block = period.clause(index).ok_or(ViewError::PeriodNotFound(id))?;
        let mut tokens = Vec::new();
        let link = offset.checked_sub(1).and_then(|t| period.tail.get(t));
        match link {
            Some(LinkedClause::Clause { link, .. }) => match link.node {
                Link::Subordination(run) => tokens.push(":".repeat(usize::from(run.count()))),
                Link::Coordination { conj, colons } => {
                    tokens.push(conj.node.surface().to_owned());
                    if let Some(run) = colons {
                        tokens.push(":".repeat(usize::from(run.node.count())));
                    }
                }
            },
            Some(LinkedClause::Portability { colons, .. }) => {
                tokens.push(":".repeat(usize::from(colons.node.count())));
            }
            None => {}
        }
        match block {
            ClauseRef::Relational(block) => {
                tokens.push(printer::signal(&block.signal.tile.node));
                tokens.push(written_slot(
                    &block.signal.glyph.node,
                    interner,
                    dti,
                    human,
                )?);
                for pair in &block.cases {
                    tokens.push(printer::case(pair.tile.node));
                    tokens.push(written_slot(&pair.glyph.node, interner, dti, human)?);
                }
            }
            ClauseRef::Definition(block) => {
                tokens.push(printer::def_signal(block.signal.node));
                tokens.push(
                    interner
                        .resolve(block.name.node)
                        .ok_or(ViewError::UnknownSymbol)?
                        .to_owned(),
                );
                for op in block.operations.iter() {
                    let (tile, value) = match op {
                        DefOperation::Assign { tile, value } => {
                            let mut surface = String::from("a");
                            if let Some(n) = tile.node {
                                surface.push_str(n.surface());
                            }
                            (surface, Some(&value.node))
                        }
                        DefOperation::Modify { value, .. } => {
                            ("abfi".to_owned(), Some(&value.node))
                        }
                        DefOperation::Union { value, .. } => ("absi".to_owned(), Some(&value.node)),
                        DefOperation::Unbind { .. } => ("abse".to_owned(), None),
                    };
                    tokens.push(tile);
                    tokens.push(match value {
                        Some(value) => written_slot(value, interner, dti, human)?,
                        None => ANAPHORA_MARK.to_string(),
                    });
                }
            }
            ClauseRef::Portability(block) => {
                tokens.push(block.keyword().surface().to_owned());
                tokens.push(match block {
                    PortabilityBlock::Anchor { id, .. }
                    | PortabilityBlock::AnchorRel { id, .. }
                    | PortabilityBlock::Rel { id, .. }
                    | PortabilityBlock::RelOff { id, .. } => render_glyph_path(&id.node, interner)
                        .map_err(|_| ViewError::UnknownSymbol)?,
                    PortabilityBlock::End { .. } => ANAPHORA_MARK.to_string(),
                });
            }
        }
        lines.push((clause_analysis.depth(), tokens));
    }
    Ok(lines)
}

fn layout_text(open: &str, close: &str, lines: Vec<(u8, Vec<String>)>, layout: Layout) -> String {
    match layout {
        Layout::Linear => {
            let mut tokens = vec![open.to_owned()];
            tokens.extend(lines.into_iter().flat_map(|(_, tokens)| tokens));
            tokens.push(close.to_owned());
            tokens.join(" ")
        }
        Layout::CStyle { indent } => {
            let mut out = String::from(open);
            for (depth, tokens) in lines {
                out.push('\n');
                let width = (usize::from(depth) + 1).saturating_mul(usize::from(indent));
                out.push_str(&" ".repeat(width));
                out.push_str(&tokens.join(" "));
            }
            out.push('\n');
            out.push_str(close);
            out
        }
    }
}

/// Human consultation view of a period.
pub fn render_view(
    session: &Session,
    id: PeriodId,
    dti: Option<&Dti>,
    mode: ViewDisplayMode,
    layout: Layout,
) -> Result<String, ViewError> {
    let period = session.period(id).ok_or(ViewError::PeriodNotFound(id))?;
    let (open, close) = (period.open.node.surface(), period.close.node.surface());
    let lines = match mode {
        ViewDisplayMode::HumanOriginal => written_lines(session, id, dti, true)?,
        ViewDisplayMode::CanonicalLexical => written_lines(session, id, dti, false)?,
        ViewDisplayMode::DtiIndexed => {
            let dti = dti.ok_or(ViewError::MissingDti)?;
            indexed_lines(session, id, dti).map_err(ViewError::Index)?
        }
    };
    Ok(layout_text(open, close, lines, layout))
}
