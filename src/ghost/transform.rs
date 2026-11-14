//! G5: keystream, token classes, period layout and text rewriting.
//!
//! The input is one canonical period in indexed wire form. Its stage-1 AST is
//! walked in textual order to collect every atom with its **role** (slot atom,
//! `def` name, portability-id atom) and the block layout that drives the
//! binding state of the `walk` module. Atoms are replaced in place, so the
//! output keeps the exact byte layout of the input (keys stay 32 bytes,
//! numerals keep their length).
//!
//! # Token classes and positions
//!
//! Every permuted token belongs to one class, identified by its label, and has
//! a 0-based position **within its class**, counted in textual order over the
//! whole period (each class has its own independent counter):
//!
//! | class                  | label            | domain                              | one position per                  |
//! |------------------------|------------------|-------------------------------------|-----------------------------------|
//! | glyph atom (DTI key)   | `glyph-atom`     | [`domain`]                          | snapshot key atom                 |
//! | ephemeral key          | `ephemeral-key`  | keys outside the snapshot (Feistel) | ephemeral key atom                |
//! | numeral digit          | `numeral-digit`  | `0..B` (snapshot base `B`)          | digit of a numeral atom           |
//! | primary signal (flag)  | `primary-signal` | 5 primaries (`oh` is fixed)         | relational tile with a primary    |
//! | tense slot (flag)      | `signal-tense`   | 7 indicative / 6 conditional        | relational signal tile            |
//! | signal number (flag)   | `signal-number`  | 4 values (omitted = Dual)           | relational signal tile            |
//! | case number (flag)     | `case-number`    | 4 values (omitted = Dual)           | case tile                         |
//!
//! Every atom consumes positions of its class, whatever the way it is mapped
//! (bound, anaphora-forced or free). The class of an atom is the same in the
//! plaintext and in the ciphertext (snapshot keys map to snapshot keys,
//! ephemeral keys to ephemeral keys, numerals to numerals of the same length),
//! so both sides advance the same counters.
//!
//! The grammar-tile classes are partitioned so that permuting them never changes
//! stage-2 validity: `oh` (which forces the DSP placement) is never permuted, and
//! the tense slot is permuted within `{omitted, 6 indicative}` and within the 6
//! conditionals separately (the bare `abdur` loop marker requires a written
//! conditional, L2).

use std::ops::Range;

use crate::ast::{
    CaseTile, ClauseBlock, Conditional, DefOperation, DefinitionBlock, ExplicitNumber,
    GlyphAnaphora, GlyphSlot, LinkedClause, Period, PortabilityBlock, PrimarySignal,
    RelationalSignal, SemaAnaphora, Temporal, TemporalMode, ANAPHORA_MARK, QUALIFIER_SEPARATOR,
    SEMA_ANAPHORA_MARK,
};
use crate::dti::{Dti, DtiKey};
use crate::parser::Parser;
use crate::span::{Span, Spanned};
use crate::symbol::Interner;
use crate::ParserConfig;

use super::permutation::{apply, permutation, Direction};
use super::primitives::{absorb_field, keyed_xof, CryptoSuite, XofReader};
use super::{GhostError, MasterKey, Nonce};

/// Domain-separation label of the keystream.
pub const KEYSTREAM_LABEL: &[u8] = b"ApriWave-Ghost-v2/keystream";
/// Class label of glyph atoms (snapshot keys).
pub const CLASS_GLYPH: &[u8] = b"glyph-atom";
/// Class label of the digits of numeral atoms (domain `0..B`, `B` = base of
/// the snapshot numeral system).
pub const CLASS_NUMERAL_DIGIT: &[u8] = b"numeral-digit";
/// Class label of primary signals (`permute_grammar_tiles`).
pub const CLASS_PRIMARY: &[u8] = b"primary-signal";
/// Class label of case-tile numbers (`permute_grammar_tiles`).
pub const CLASS_NUMBER: &[u8] = b"case-number";
/// Class label of relational signal-tile numbers (rule 1B, `permute_grammar_tiles`).
/// Same value class as [`CLASS_NUMBER`], separate label and position counter.
pub const CLASS_SIGNAL_NUMBER: &[u8] = b"signal-number";
/// Class label of the tense slot of signal determiners (`permute_grammar_tiles`).
pub const CLASS_TENSE: &[u8] = b"signal-tense";

/// Finite class of tile numbers (omitted = Dual), shared by case tiles and
/// relational signal tiles.
pub(super) const NUMBERS: [Option<ExplicitNumber>; 4] = [
    None,
    Some(ExplicitNumber::Singular),
    Some(ExplicitNumber::Plural),
    Some(ExplicitNumber::NonDual),
];

/// Primaries permuted by `permute_grammar_tiles` (`oh` is fixed: it forces the
/// DSP placement, H4).
pub(super) const PRIMARIES: [PrimarySignal; 5] = [
    PrimarySignal::Ta,
    PrimarySignal::To,
    PrimarySignal::Ka,
    PrimarySignal::Ko,
    PrimarySignal::Ot,
];

/// Tense partition 1: omitted and the 6 indicative temporals.
pub(super) const INDICATIVE_TENSES: [Option<TemporalMode>; 7] = [
    None,
    Some(TemporalMode::Indicative(Temporal::Ato)),
    Some(TemporalMode::Indicative(Temporal::Alt)),
    Some(TemporalMode::Indicative(Temporal::Alv)),
    Some(TemporalMode::Indicative(Temporal::Att)),
    Some(TemporalMode::Indicative(Temporal::Atv)),
    Some(TemporalMode::Indicative(Temporal::Asu)),
];

/// Tense partition 2: the 6 conditionals.
pub(super) const CONDITIONAL_TENSES: [Option<TemporalMode>; 6] = [
    Some(TemporalMode::Conditional(Conditional(Temporal::Ato))),
    Some(TemporalMode::Conditional(Conditional(Temporal::Alt))),
    Some(TemporalMode::Conditional(Conditional(Temporal::Alv))),
    Some(TemporalMode::Conditional(Conditional(Temporal::Att))),
    Some(TemporalMode::Conditional(Conditional(Temporal::Atv))),
    Some(TemporalMode::Conditional(Conditional(Temporal::Asu))),
];

/// Every value of the tense slot (union of the two partitions).
#[cfg(test)]
pub(super) fn all_tenses() -> impl Iterator<Item = Option<TemporalMode>> {
    INDICATIVE_TENSES.into_iter().chain(CONDITIONAL_TENSES)
}

/// Per-message keystream parameters.
pub struct Keystream<'k> {
    pub suite: CryptoSuite,
    pub key: &'k MasterKey,
    pub nonce: Nonce,
    pub snapshot_version: u64,
}

impl Keystream<'_> {
    fn absorb_prefix(
        &self,
        class: &[u8],
        position: u64,
    ) -> Result<Box<dyn super::primitives::Xof>, GhostError> {
        let mut xof = keyed_xof(self.suite, KEYSTREAM_LABEL, self.key)?;
        xof.update(&self.nonce.0);
        xof.update(&self.snapshot_version.to_le_bytes());
        absorb_field(xof.as_mut(), class);
        xof.update(&position.to_le_bytes());
        Ok(xof)
    }

    /// `XOF(field(label) || key || nonce || version (u64 LE) || field(class) || position (u64 LE))`
    /// (BLAKE3: keyed mode with the master key instead of absorbing it).
    pub fn reader(&self, class: &[u8], position: u64) -> Result<Box<dyn XofReader>, GhostError> {
        Ok(self.absorb_prefix(class, position)?.finalize_xof())
    }

    /// Keyed PRF of a token position with extra input:
    /// `reader(class, position)` prefix followed by `field(input)`.
    pub fn prf(
        &self,
        class: &[u8],
        position: u64,
        input: &[u8],
    ) -> Result<Box<dyn XofReader>, GhostError> {
        let mut xof = self.absorb_prefix(class, position)?;
        absorb_field(xof.as_mut(), input);
        Ok(xof.finalize_xof())
    }

    /// Permutation `π_position` of a class of `size` elements.
    pub fn permutation(
        &self,
        class: &[u8],
        position: u64,
        size: usize,
    ) -> Result<Vec<usize>, GhostError> {
        let mut reader = self.reader(class, position)?;
        permutation(reader.as_mut(), size)
    }

    fn map(
        &self,
        class: &[u8],
        position: u64,
        size: usize,
        index: usize,
        direction: Direction,
    ) -> Result<usize, GhostError> {
        let images = self.permutation(class, position, size)?;
        apply(&images, index, direction).ok_or(GhostError::Internal {
            context: "index outside the permuted class",
        })
    }

    /// Maps `value` within the finite class `values`.
    fn map_value<T: Copy + PartialEq>(
        &self,
        class: &[u8],
        position: u64,
        values: &[T],
        value: T,
        direction: Direction,
    ) -> Result<T, GhostError> {
        let index = values
            .iter()
            .position(|candidate| *candidate == value)
            .ok_or(GhostError::Internal {
                context: "value outside its grammar class",
            })?;
        let mapped = self.map(class, position, values.len(), index, direction)?;
        values.get(mapped).copied().ok_or(GhostError::Internal {
            context: "mapped index outside its grammar class",
        })
    }
}

/// `true` for ASCII numerals (atoms made only of digits).
pub fn is_numeral(atom: &str) -> bool {
    !atom.is_empty() && atom.bytes().all(|byte| byte.is_ascii_digit())
}

/// G4: non-deprecated universal keys, sorted by key bytes. DTI keys always
/// start with a letter, so no key can collide with a numeral.
pub fn domain(dti: &Dti) -> Vec<DtiKey> {
    let mut keys: Vec<DtiKey> = dti
        .universal()
        .concepts
        .iter()
        .filter(|(_, record)| record.superseded_by.is_none())
        .map(|(key, _)| *key)
        .collect();
    keys.sort_unstable();
    keys
}

/// Parses exactly one stage-1 valid period (no stage 2).
pub fn parse_single(text: &str) -> Result<Period, GhostError> {
    let lexed = crate::lexer::lex(text).map_err(|diagnostic| GhostError::Stage1 {
        diagnostics: vec![diagnostic],
    })?;
    let mut interner = Interner::new();
    let mut parser = Parser::new(
        &lexed.tokens,
        &mut interner,
        ParserConfig::default(),
        lexed.end,
    );
    let mut periods = parser.parse_session(true);
    let mut diagnostics = parser.into_diagnostics();
    diagnostics.extend(lexed.diagnostics.iter().copied());
    if !diagnostics.is_empty() {
        diagnostics.sort_by_key(|d| d.span);
        return Err(GhostError::Stage1 { diagnostics });
    }
    if periods.len() != 1 {
        return Err(GhostError::PeriodCount {
            found: periods.len(),
        });
    }
    periods.pop().ok_or(GhostError::PeriodCount { found: 0 })
}

// ═══════════════════════════ Layout ═══════════════════════════

/// Byte range of an atom inside the source text.
#[derive(Debug, Clone, Copy)]
pub struct AtomSite {
    pub start: usize,
    pub end: usize,
    pub span: Span,
}

/// Syntactic form of a slot holding atoms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotForm {
    /// Glyph path (`K`, `K1§K2`).
    Glyph,
    /// `base~`
    Prefix,
    /// `base§mod~`
    Inverse,
    /// `glyph~~`
    SemaByGlyph,
}

/// A `[Tile] [Glyph]` slot of the period that holds atoms.
#[derive(Debug, Clone, Copy)]
pub struct SlotSite {
    /// Block index (`0` = head, `i + 1` = `tail[i]`).
    pub clause: u32,
    /// Pair position (relational: `0` = signal; `def`: operation index + 1).
    pub slot: u32,
    pub form: SlotForm,
    /// The slot belongs to a relational block (R13 applies).
    pub relational: bool,
    /// Index of the first atom and number of atoms.
    pub first: usize,
    pub count: usize,
}

impl SlotSite {
    pub fn atoms(&self) -> Range<usize> {
        self.first..self.first.saturating_add(self.count)
    }
}

/// Operation of a `def` block (pair position = index in the block + 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefOpKind {
    Assign,
    Modify,
    Union,
    Unbind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Relational,
    /// The name is the first atom of the block; the operations are
    /// `Layout::def_ops[first_op..first_op + op_count]`.
    Definition {
        first_op: usize,
        op_count: usize,
    },
    /// `anchor` / `anchorrel`.
    Anchor,
    Rel,
    RelOff,
    End,
}

/// A block and the range of its atoms.
#[derive(Debug, Clone, Copy)]
pub struct BlockSite {
    /// Block index (`0` = head, `i + 1` = `tail[i]`).
    pub clause: u32,
    pub kind: BlockKind,
    pub first: usize,
    pub count: usize,
}

impl BlockSite {
    pub fn atoms(&self) -> Range<usize> {
        self.first..self.first.saturating_add(self.count)
    }
}

/// Role of an atom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomRole {
    /// Atom of the slot with this index in [`Layout::slots`].
    Slot(usize),
    /// `def` variable name.
    DefName,
    /// Portability-id atom.
    PortId,
}

#[derive(Debug, Clone, Copy)]
pub struct SignalSite {
    span: Span,
    signal: RelationalSignal,
}

#[derive(Debug, Clone, Copy)]
pub struct CaseSite {
    span: Span,
    tile: CaseTile,
}

/// Every atom of the period (textual order) with its role, plus the slots,
/// the blocks and the grammar tiles.
#[derive(Debug, Default)]
pub struct Layout {
    pub atoms: Vec<AtomSite>,
    pub roles: Vec<AtomRole>,
    pub slots: Vec<SlotSite>,
    pub blocks: Vec<BlockSite>,
    pub def_ops: Vec<DefOpKind>,
    signals: Vec<SignalSite>,
    cases: Vec<CaseSite>,
}

pub(super) fn range_of(span: Span) -> Result<(usize, usize), GhostError> {
    let start = usize::try_from(span.start).map_err(|_| GhostError::InvalidSpan { span })?;
    let end = usize::try_from(span.end).map_err(|_| GhostError::InvalidSpan { span })?;
    Ok((start, end))
}

fn span_of(start: usize, end: usize, within: Span) -> Result<Span, GhostError> {
    let start = u32::try_from(start).map_err(|_| GhostError::InvalidSpan { span: within })?;
    let end = u32::try_from(end).map_err(|_| GhostError::InvalidSpan { span: within })?;
    Ok(Span::new(start, end))
}

fn to_u32(value: usize) -> Result<u32, GhostError> {
    u32::try_from(value).map_err(|_| GhostError::Internal {
        context: "index beyond u32",
    })
}

impl Layout {
    pub fn period(period: &Period, text: &str) -> Result<Self, GhostError> {
        let mut layout = Self::default();
        layout.clause(0, &period.head, text)?;
        for (offset, linked) in period.tail.iter().enumerate() {
            let clause = to_u32(offset.saturating_add(1))?;
            match linked {
                LinkedClause::Clause { block, .. } => layout.clause(clause, block, text)?,
                LinkedClause::Portability { block, .. } => {
                    layout.portability(clause, block, text)?;
                }
            }
        }
        // The AST is walked in textual order: atoms must be strictly increasing.
        let ordered = layout
            .atoms
            .iter()
            .zip(layout.atoms.iter().skip(1))
            .all(|(a, b)| a.end < b.start);
        if !ordered {
            return Err(GhostError::Internal {
                context: "atoms out of textual order",
            });
        }
        Ok(layout)
    }

    /// Slot site of `(clause, slot)`.
    pub fn slot_site(&self, clause: u32, slot: u32) -> Option<&SlotSite> {
        self.slots
            .iter()
            .find(|site| site.clause == clause && site.slot == slot)
    }

    fn clause(&mut self, clause: u32, block: &ClauseBlock, text: &str) -> Result<(), GhostError> {
        let first = self.atoms.len();
        let kind = match block {
            ClauseBlock::Definition(definition) => self.definition(clause, definition, text)?,
            ClauseBlock::Relational(relational) => {
                self.signals.push(SignalSite {
                    span: relational.signal.tile.span,
                    signal: relational.signal.tile.node,
                });
                self.slot(clause, 0, true, &relational.signal.glyph, text)?;
                for (index, pair) in relational.cases.iter().enumerate() {
                    self.cases.push(CaseSite {
                        span: pair.tile.span,
                        tile: pair.tile.node,
                    });
                    self.slot(
                        clause,
                        to_u32(index.saturating_add(1))?,
                        true,
                        &pair.glyph,
                        text,
                    )?;
                }
                BlockKind::Relational
            }
        };
        self.push_block(clause, kind, first);
        Ok(())
    }

    fn push_block(&mut self, clause: u32, kind: BlockKind, first: usize) {
        self.blocks.push(BlockSite {
            clause,
            kind,
            first,
            count: self.atoms.len().saturating_sub(first),
        });
    }

    /// `def` blocks: the name (one atom by construction) and every operation
    /// value; the `def` signal tile and the operation tiles are never touched.
    fn definition(
        &mut self,
        clause: u32,
        definition: &DefinitionBlock,
        text: &str,
    ) -> Result<BlockKind, GhostError> {
        self.path(definition.name.span, 0, text, AtomRole::DefName)?;
        let first_op = self.def_ops.len();
        for (index, operation) in definition.operations.iter().enumerate() {
            let slot = to_u32(index.saturating_add(1))?;
            let (kind, value) = match operation {
                DefOperation::Assign { value, .. } => (DefOpKind::Assign, Some(value)),
                DefOperation::Modify { value, .. } => (DefOpKind::Modify, Some(value)),
                DefOperation::Union { value, .. } => (DefOpKind::Union, Some(value)),
                DefOperation::Unbind { .. } => (DefOpKind::Unbind, None),
            };
            if let Some(value) = value {
                self.slot(clause, slot, false, value, text)?;
            }
            self.def_ops.push(kind);
        }
        Ok(BlockKind::Definition {
            first_op,
            op_count: self.def_ops.len().saturating_sub(first_op),
        })
    }

    /// Portability blocks: the id path atom by atom; `end ~` has no atom.
    fn portability(
        &mut self,
        clause: u32,
        block: &PortabilityBlock,
        text: &str,
    ) -> Result<(), GhostError> {
        let first = self.atoms.len();
        let kind = match block {
            PortabilityBlock::Anchor { id, .. } | PortabilityBlock::AnchorRel { id, .. } => {
                self.path(id.span, 0, text, AtomRole::PortId)?;
                BlockKind::Anchor
            }
            PortabilityBlock::Rel { id, .. } => {
                self.path(id.span, 0, text, AtomRole::PortId)?;
                BlockKind::Rel
            }
            PortabilityBlock::RelOff { id, .. } => {
                self.path(id.span, 0, text, AtomRole::PortId)?;
                BlockKind::RelOff
            }
            PortabilityBlock::End { .. } => BlockKind::End,
        };
        self.push_block(clause, kind, first);
        Ok(())
    }

    /// Strips a trailing `~`/`~~` from the slot text and collects its atoms.
    fn slot(
        &mut self,
        clause: u32,
        slot: u32,
        relational: bool,
        glyph: &Spanned<GlyphSlot>,
        text: &str,
    ) -> Result<(), GhostError> {
        let span = glyph.span;
        let (suffix, form) = match &glyph.node {
            GlyphSlot::RawString(_) => return Err(GhostError::RawString { span }),
            GlyphSlot::GlyphAnaphora(GlyphAnaphora::Exact)
            | GlyphSlot::SemaAnaphora(SemaAnaphora::Latest) => return Ok(()),
            GlyphSlot::Glyph(_) => (0, SlotForm::Glyph),
            GlyphSlot::GlyphAnaphora(GlyphAnaphora::Prefix { .. }) => {
                (ANAPHORA_MARK.len_utf8(), SlotForm::Prefix)
            }
            GlyphSlot::GlyphAnaphora(GlyphAnaphora::Inverse { .. }) => {
                (ANAPHORA_MARK.len_utf8(), SlotForm::Inverse)
            }
            GlyphSlot::SemaAnaphora(SemaAnaphora::ByGlyph { .. }) => {
                (SEMA_ANAPHORA_MARK.len(), SlotForm::SemaByGlyph)
            }
        };
        let index = self.slots.len();
        let (first, count) = self.path(span, suffix, text, AtomRole::Slot(index))?;
        self.slots.push(SlotSite {
            clause,
            slot,
            form,
            relational,
            first,
            count,
        });
        Ok(())
    }

    /// Splits the text of `span` (minus `suffix` trailing bytes) on `§`.
    fn path(
        &mut self,
        span: Span,
        suffix: usize,
        text: &str,
        role: AtomRole,
    ) -> Result<(usize, usize), GhostError> {
        let (start, end) = range_of(span)?;
        let body_end = end
            .checked_sub(suffix)
            .ok_or(GhostError::InvalidSpan { span })?;
        let body = text
            .get(start..body_end)
            .ok_or(GhostError::InvalidSpan { span })?;
        let first = self.atoms.len();
        let mut offset = start;
        for atom in body.split(QUALIFIER_SEPARATOR) {
            let atom_end = offset
                .checked_add(atom.len())
                .ok_or(GhostError::InvalidSpan { span })?;
            self.atoms.push(AtomSite {
                start: offset,
                end: atom_end,
                span: span_of(offset, atom_end, span)?,
            });
            self.roles.push(role);
            offset = atom_end
                .checked_add(QUALIFIER_SEPARATOR.len_utf8())
                .ok_or(GhostError::InvalidSpan { span })?;
        }
        Ok((first, self.atoms.len().saturating_sub(first)))
    }

    /// Edits of the grammar tiles (`permute_grammar_tiles`).
    pub fn grammar_edits(
        &self,
        keystream: &Keystream<'_>,
        direction: Direction,
        edits: &mut Vec<Edit>,
    ) -> Result<(), GhostError> {
        signal_edits(&self.signals, keystream, direction, edits)?;
        case_edits(&self.cases, keystream, direction, edits)
    }
}

// ═══════════════════════════ Rewriting ═══════════════════════════

/// Replacement of a byte range.
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

pub fn apply_edits(text: &str, mut edits: Vec<Edit>) -> Result<String, GhostError> {
    edits.sort_by_key(|edit| edit.start);
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0_usize;
    for edit in &edits {
        let unchanged = text.get(cursor..edit.start).ok_or(GhostError::Internal {
            context: "overlapping edits",
        })?;
        out.push_str(unchanged);
        out.push_str(&edit.replacement);
        cursor = edit.end;
    }
    let rest = text.get(cursor..).ok_or(GhostError::Internal {
        context: "edit beyond the end of the text",
    })?;
    out.push_str(rest);
    Ok(out)
}

/// Encrypts (or decrypts) the digits of a numeral: digit `k` is permuted with
/// `π_{position + k}` of class `numeral-digit` over `0..base`.
pub fn map_digits(
    keystream: &Keystream<'_>,
    digits: &[u8],
    base: u8,
    position: u64,
    direction: Direction,
) -> Result<Vec<u8>, GhostError> {
    let size = usize::from(base);
    let mut out = Vec::with_capacity(digits.len());
    for (offset, digit) in (0_u64..).zip(digits.iter()) {
        let mapped = keystream.map(
            CLASS_NUMERAL_DIGIT,
            position.saturating_add(offset),
            size,
            usize::from(*digit),
            direction,
        )?;
        let mapped = u8::try_from(mapped)
            .ok()
            .filter(|value| *value < base)
            .ok_or(GhostError::Internal {
                context: "mapped digit outside the base",
            })?;
        out.push(mapped);
    }
    Ok(out)
}

/// Tense partition containing `tense`.
fn tense_class(tense: Option<TemporalMode>) -> &'static [Option<TemporalMode>] {
    match tense {
        Some(TemporalMode::Conditional(_)) => &CONDITIONAL_TENSES,
        None | Some(TemporalMode::Indicative(_)) => &INDICATIVE_TENSES,
    }
}

/// Relational signal tiles: primary (when present and not `oh`), tense slot
/// (within its partition) and number, each with its own label and position
/// counter. `def` tiles are not collected, so they are never permuted.
fn signal_edits(
    signals: &[SignalSite],
    keystream: &Keystream<'_>,
    direction: Direction,
    edits: &mut Vec<Edit>,
) -> Result<(), GhostError> {
    let mut primary_position = 0_u64;
    for (position, site) in (0_u64..).zip(signals.iter()) {
        let mut signal = site.signal;
        let determiners = match &mut signal {
            RelationalSignal::Primary {
                signal: primary,
                determiners,
            }
            | RelationalSignal::Secondary {
                primary,
                determiners,
                ..
            } => {
                if *primary != PrimarySignal::Oh {
                    *primary = keystream.map_value(
                        CLASS_PRIMARY,
                        primary_position,
                        &PRIMARIES,
                        *primary,
                        direction,
                    )?;
                }
                primary_position = primary_position.saturating_add(1);
                determiners
            }
            RelationalSignal::Deep { determiners } => determiners,
        };
        // Tense slot and number exist on every relational signal tile, so
        // their counters coincide with the tile index.
        determiners.tense = keystream.map_value(
            CLASS_TENSE,
            position,
            tense_class(determiners.tense),
            determiners.tense,
            direction,
        )?;
        determiners.number = keystream.map_value(
            CLASS_SIGNAL_NUMBER,
            position,
            &NUMBERS,
            determiners.number,
            direction,
        )?;
        let (start, end) = range_of(site.span)?;
        edits.push(Edit {
            start,
            end,
            replacement: crate::printer::signal(&signal),
        });
    }
    Ok(())
}

fn case_edits(
    cases: &[CaseSite],
    keystream: &Keystream<'_>,
    direction: Direction,
    edits: &mut Vec<Edit>,
) -> Result<(), GhostError> {
    for (position, site) in (0_u64..).zip(cases.iter()) {
        let mut tile = site.tile;
        tile.number =
            keystream.map_value(CLASS_NUMBER, position, &NUMBERS, tile.number, direction)?;
        let (start, end) = range_of(site.span)?;
        edits.push(Edit {
            start,
            end,
            replacement: crate::printer::case(tile),
        });
    }
    Ok(())
}
