//! The period walk: maps every atom in textual order with the binding-consistent
//! bijection of its class and maintains the injective symbol table (G1, option
//! 2) according to the stage-2 structure of the period.
//!
//! # Binding rules
//!
//! The set of bindings used at a position is **every live entry of the symbol
//! table** (see the `table` module), restricted to the class of the atom. The
//! table evolves block by block, identically on both sides, from the stage-2
//! analysis of the period (the sender analyses the plaintext, the receiver the
//! ciphertext: their structure and resolutions coincide):
//!
//! | block                    | before its atoms                          | after its atoms                                                   |
//! |--------------------------|-------------------------------------------|-------------------------------------------------------------------|
//! | relational at depth `d`  | DSP stack truncated to `d` (pruned blocks released) | holds the atoms of the value paths of its pairs; stays visible while on the stack (level 0: until the end of the period) |
//! | `def`                    | —                                         | `a*` creates the variable hold (name + value atoms), `abfi` replaces the values, `absi` adds a value, `abse ~` releases the variable |
//! | `anchor` / `anchorrel`   | DSP stack truncated to the anchor depth   | the anchor holds its id atoms                                     |
//! | `rel`                    | —                                         | anchors opened after the target are closed (R1)                   |
//! | `end ~`                  | —                                         | the last anchor is released, the stack is truncated to its depth   |
//!
//! While a block is being written every mapped atom is held, so two atoms of the
//! same block with the same plaintext get the same cipher atom and atoms with
//! different plaintexts get different cipher atoms. At the end of the period:
//! open anchors and the DSP stack are released; with `/^/` the level-0 blocks
//! become carried antecedents; `^/` releases the level-0 blocks and every carried
//! block; `^^/` also releases every variable.
//!
//! # Why anaphora resolve identically
//!
//! An anaphora base (`base~`, `base§mod~`, `glyph~~`) is compared with the atoms
//! of the value paths of the visible blocks, which are all held. Every live entry
//! is a binding, so `cipher(x) = cipher(y) ⇔ plain(x) = plain(y)` for every atom
//! compared by the R6–R8 search; therefore the ciphertext resolves every
//! anaphora to the same antecedent, the base atoms automatically take the cipher
//! keys of the antecedent atoms, and the receiver decrypts them with the plain
//! inverse bijection. The same holds for R13 references (variable names are
//! held by their variable) and for `rel`/`reloff` ids (held by their anchor).

use crate::analyze::{ClauseId, Resolution, SlotIndex, SlotRef, SlotValue};
use crate::ast::{GlyphSlot, PeriodClose};
use crate::dti::DtiKey;
use crate::session::{PeriodId, Session};

use super::bijection::{forward, inverse, Image};
use super::feistel::Feistel;
use super::permutation::Direction;
use super::table::{Atom, Class, Snapshot, Txn, VariableHold};
use super::transform::{
    is_numeral, map_digits, AtomRole, AtomSite, BlockKind, BlockSite, DefOpKind, Edit, Keystream,
    Layout, SlotForm, CLASS_GLYPH,
};
use super::GhostError;

/// Parses and classifies one atom of the input (pass 1: no stage 2 needed).
pub fn parse_atom(
    text: &str,
    site: &AtomSite,
    snapshot: &Snapshot<'_>,
) -> Result<(Atom, Class), GhostError> {
    let span = site.span;
    let atom_text = text
        .get(site.start..site.end)
        .ok_or(GhostError::InvalidSpan { span })?;
    if is_numeral(atom_text) {
        let base = snapshot
            .dti
            .universal()
            .numeral()
            .map(|system| system.base)
            .ok_or(GhostError::NoNumeralSystem { span })?;
        let mut digits = Vec::with_capacity(atom_text.len());
        for (offset, byte) in (site.start..).zip(atom_text.bytes()) {
            let digit = byte.wrapping_sub(b'0');
            if digit >= base {
                let start = u32::try_from(offset).map_err(|_| GhostError::InvalidSpan { span })?;
                return Err(GhostError::DigitOutOfBase {
                    span: crate::span::Span::new(start, start.saturating_add(1)),
                    digit,
                    base,
                });
            }
            digits.push(digit);
        }
        let atom = Atom::Numeral(digits.into_boxed_slice());
        return Ok((atom, Class::Numeral(atom_text.len())));
    }
    let key = DtiKey::parse(atom_text).map_err(|error| GhostError::InvalidKey { span, error })?;
    let atom = Atom::Key(key);
    let class = snapshot
        .class(&atom)
        .ok_or(GhostError::KeyNotInDomain { span })?;
    Ok((atom, class))
}

/// Parses an atom of an already accepted period (previous periods).
fn parse_known(text: &str) -> Result<Atom, GhostError> {
    if is_numeral(text) {
        let digits: Vec<u8> = text.bytes().map(|b| b.wrapping_sub(b'0')).collect();
        return Ok(Atom::Numeral(digits.into_boxed_slice()));
    }
    DtiKey::parse(text)
        .map(Atom::Key)
        .map_err(|_| GhostError::Internal {
            context: "accepted period holds an invalid atom",
        })
}

/// Rule 5: an ephemeral key may only be a bound symbol: `def` name,
/// portability-id atom, or R13 reference (atomic glyph of a relational pair
/// resolved by stage 2 as a live variable).
pub fn check_ephemeral(view: &PeriodView<'_>, snapshot: &Snapshot<'_>) -> Result<(), GhostError> {
    for (index, (site, role)) in view
        .layout
        .atoms
        .iter()
        .zip(view.layout.roles.iter())
        .enumerate()
    {
        let (_, class) = parse_atom(view.text, site, snapshot)?;
        if class != Class::Ephemeral {
            continue;
        }
        let allowed = match role {
            AtomRole::DefName | AtomRole::PortId => true,
            AtomRole::Slot(slot) => view.layout.slots.get(*slot).is_some_and(|slot| {
                slot.relational
                    && slot.form == SlotForm::Glyph
                    && slot.count == 1
                    && slot.first == index
                    && view
                        .slot_analysis(slot.clause, slot.slot)
                        .is_some_and(|resolution| matches!(resolution, Resolution::Variable { .. }))
            }),
        };
        if !allowed {
            return Err(GhostError::UnboundEphemeralKey { span: site.span });
        }
    }
    Ok(())
}

/// The period being processed and its stage-2 context.
pub struct PeriodView<'a> {
    /// Input text (plaintext when encrypting, ciphertext when decrypting).
    pub text: &'a str,
    pub layout: &'a Layout,
    /// Session holding the stage-2 analysis of the input period (`period`).
    pub analysis: &'a Session,
    pub period: PeriodId,
    /// Plaintext session (atoms of previous periods).
    pub plain: &'a Session,
    pub close: PeriodClose,
}

impl PeriodView<'_> {
    fn slot_analysis(&self, clause: u32, slot: u32) -> Option<Resolution> {
        self.analysis
            .analysis(self.period)?
            .clause(ClauseId::new(clause))?
            .slot(SlotIndex::new(slot))
            .map(|analysis| analysis.resolution())
    }

    fn slot_value(&self, clause: u32, slot: u32) -> Option<SlotValue> {
        self.analysis
            .analysis(self.period)?
            .clause(ClauseId::new(clause))?
            .slot(SlotIndex::new(slot))
            .map(|analysis| analysis.value())
    }

    fn depth(&self, clause: u32) -> Result<usize, GhostError> {
        self.analysis
            .analysis(self.period)
            .and_then(|period| period.clause(ClauseId::new(clause)))
            .map(|analysis| usize::from(analysis.depth()))
            .ok_or(GhostError::Internal {
                context: "clause without stage-2 analysis",
            })
    }

    /// Literal glyph slots of the value paths of a value (R7: variable values
    /// in any order; raw strings and semas have no path).
    fn literal_slots(&self, value: SlotValue) -> Vec<SlotRef> {
        let is_glyph =
            |slot: &SlotRef| matches!(self.analysis.glyph_slot(*slot), Some(GlyphSlot::Glyph(_)));
        match value {
            SlotValue::Glyph(slot) if is_glyph(&slot) => vec![slot],
            SlotValue::Variable(binding) => self
                .analysis
                .binding(binding)
                .map(|binding| {
                    binding
                        .values()
                        .iter()
                        .filter_map(|value| match value {
                            SlotValue::Glyph(slot) if is_glyph(slot) => Some(*slot),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            SlotValue::Glyph(_) | SlotValue::Raw(_) | SlotValue::Sema(_) => Vec::new(),
        }
    }
}

/// Per-class position counters.
#[derive(Default)]
struct Positions {
    glyph: u64,
    digit: u64,
    ephemeral: u64,
}

struct Frame {
    depth: usize,
    ids: Vec<usize>,
}

/// Mutable state of the walk.
struct Walker<'a, 'k, 't, 'x> {
    view: &'a PeriodView<'a>,
    snapshot: &'a Snapshot<'a>,
    keystream: &'a Keystream<'k>,
    direction: Direction,
    txn: &'x mut Txn<'t>,
    positions: Positions,
    /// Entry of every processed atom.
    atom_ids: Vec<usize>,
    edits: Vec<Edit>,
    /// DSP stack: holds of the block at each depth (`None` for level 0, whose
    /// holds live in `roots`).
    stack: Vec<Option<Vec<usize>>>,
    roots: Vec<Vec<usize>>,
    frames: Vec<Frame>,
}

/// Runs the walk; returns the atom edits (the grammar tiles are handled by
/// the caller).
pub fn run(
    view: &PeriodView<'_>,
    snapshot: &Snapshot<'_>,
    keystream: &Keystream<'_>,
    txn: &mut Txn<'_>,
    direction: Direction,
) -> Result<Vec<Edit>, GhostError> {
    let mut walker = Walker {
        view,
        snapshot,
        keystream,
        direction,
        txn,
        positions: Positions::default(),
        atom_ids: Vec::with_capacity(view.layout.atoms.len()),
        edits: Vec::with_capacity(view.layout.atoms.len()),
        stack: Vec::new(),
        roots: Vec::new(),
        frames: Vec::new(),
    };
    for block in &view.layout.blocks {
        walker.block(block)?;
    }
    walker.finish()?;
    Ok(walker.edits)
}

const fn internal(context: &'static str) -> GhostError {
    GhostError::Internal { context }
}

impl Walker<'_, '_, '_, '_> {
    fn block(&mut self, block: &BlockSite) -> Result<(), GhostError> {
        match block.kind {
            BlockKind::Relational => {
                let depth = self.view.depth(block.clause)?;
                self.truncate(depth)?;
                let current = self.atoms(block)?;
                // Every pair, with or without atoms (`~`, `~~`), has value paths.
                let mut holds = Vec::new();
                for pair in 0..self.pair_count(block.clause) {
                    holds.extend(self.slot_holds(block.clause, pair)?);
                }
                self.txn.acquire_all(&holds)?;
                self.txn.release_all(&current)?;
                self.stack.resize_with(depth, || None);
                if depth == 0 {
                    self.stack.push(None);
                    self.roots.push(holds);
                } else {
                    self.stack.push(Some(holds));
                }
            }
            BlockKind::Definition { first_op, op_count } => {
                let current = self.atoms(block)?;
                let name = *current
                    .first()
                    .ok_or_else(|| internal("def without name"))?;
                let ops: Vec<DefOpKind> = self
                    .view
                    .layout
                    .def_ops
                    .get(first_op..first_op.saturating_add(op_count))
                    .ok_or_else(|| internal("def operations out of range"))?
                    .to_vec();
                for (index, op) in ops.into_iter().enumerate() {
                    let pair = u32::try_from(index.saturating_add(1))
                        .map_err(|_| internal("def operation index beyond u32"))?;
                    self.def_op(block.clause, name, pair, op)?;
                }
                self.txn.release_all(&current)?;
            }
            BlockKind::Anchor => {
                let depth = self.view.depth(block.clause)?;
                self.truncate(depth)?;
                // The holds taken while writing the id are kept by the anchor.
                let current = self.atoms(block)?;
                self.frames.push(Frame {
                    depth,
                    ids: current,
                });
            }
            BlockKind::Rel => {
                let current = self.atoms(block)?;
                if let Some(position) = self.frames.iter().rposition(|f| f.ids == current) {
                    while self.frames.len() > position.saturating_add(1) {
                        if let Some(frame) = self.frames.pop() {
                            self.txn.release_all(&frame.ids)?;
                        }
                    }
                }
                self.txn.release_all(&current)?;
            }
            BlockKind::RelOff => {
                let current = self.atoms(block)?;
                self.txn.release_all(&current)?;
            }
            BlockKind::End => {
                if let Some(frame) = self.frames.pop() {
                    self.truncate(frame.depth)?;
                    self.txn.release_all(&frame.ids)?;
                }
            }
        }
        Ok(())
    }

    fn pair_count(&self, clause: u32) -> u32 {
        self.view
            .analysis
            .analysis(self.view.period)
            .and_then(|period| period.clause(ClauseId::new(clause)))
            .and_then(|analysis| u32::try_from(analysis.slots().len()).ok())
            .unwrap_or(0)
    }

    /// Applies one `def` operation to the variable holds.
    fn def_op(
        &mut self,
        clause: u32,
        name: usize,
        pair: u32,
        op: DefOpKind,
    ) -> Result<(), GhostError> {
        let existing = self.txn.variables.iter().position(|v| v.name == name);
        match op {
            DefOpKind::Assign => {
                if let Some(index) = existing {
                    self.drop_variable(index)?;
                }
                let values = self.slot_holds(clause, pair)?;
                self.txn.acquire(name)?;
                self.txn.acquire_all(&values)?;
                self.txn.variables.push(VariableHold { name, values });
            }
            DefOpKind::Modify | DefOpKind::Union => {
                let index = existing.ok_or_else(|| internal("def operation on a dead variable"))?;
                let values = self.slot_holds(clause, pair)?;
                self.txn.acquire_all(&values)?;
                let variable = self
                    .txn
                    .variables
                    .get_mut(index)
                    .ok_or_else(|| internal("variable index out of range"))?;
                let previous = if op == DefOpKind::Modify {
                    std::mem::replace(&mut variable.values, values)
                } else {
                    variable.values.extend(values);
                    Vec::new()
                };
                self.txn.release_all(&previous)?;
            }
            DefOpKind::Unbind => {
                let index = existing.ok_or_else(|| internal("unbinding a dead variable"))?;
                self.drop_variable(index)?;
            }
        }
        Ok(())
    }

    fn drop_variable(&mut self, index: usize) -> Result<(), GhostError> {
        if index < self.txn.variables.len() {
            let variable = self.txn.variables.swap_remove(index);
            self.txn.release(variable.name)?;
            self.txn.release_all(&variable.values)?;
        }
        Ok(())
    }

    /// Truncates the DSP stack to `depth`, releasing pruned blocks.
    fn truncate(&mut self, depth: usize) -> Result<(), GhostError> {
        while self.stack.len() > depth {
            if let Some(Some(holds)) = self.stack.pop() {
                self.txn.release_all(&holds)?;
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), GhostError> {
        while let Some(frame) = self.frames.pop() {
            self.txn.release_all(&frame.ids)?;
        }
        self.truncate(0)?;
        let roots = std::mem::take(&mut self.roots);
        match self.view.close {
            PeriodClose::Soft => self.txn.carried.extend(roots),
            PeriodClose::Standard | PeriodClose::Hard => {
                for holds in roots {
                    self.txn.release_all(&holds)?;
                }
                for holds in std::mem::take(&mut self.txn.carried) {
                    self.txn.release_all(&holds)?;
                }
                if self.view.close == PeriodClose::Hard {
                    while !self.txn.variables.is_empty() {
                        self.drop_variable(0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Maps every atom of the block; returns their entries (each held once by
    /// the block being written).
    fn atoms(&mut self, block: &BlockSite) -> Result<Vec<usize>, GhostError> {
        let mut ids = Vec::with_capacity(block.count);
        for index in block.atoms() {
            if index != self.atom_ids.len() {
                return Err(internal("atoms processed out of order"));
            }
            let site = self
                .view
                .layout
                .atoms
                .get(index)
                .ok_or_else(|| internal("atom index out of range"))?;
            let id = self.atom(site)?;
            self.txn.acquire(id)?;
            self.atom_ids.push(id);
            ids.push(id);
        }
        Ok(ids)
    }

    /// Entries of the atoms of the value paths of pair `slot` of `clause`.
    fn slot_holds(&self, clause: u32, slot: u32) -> Result<Vec<usize>, GhostError> {
        let Some(value) = self.view.slot_value(clause, slot) else {
            return Ok(Vec::new());
        };
        let mut ids = Vec::new();
        for literal in self.view.literal_slots(value) {
            if literal.clause.period == self.view.period {
                let site = self
                    .view
                    .layout
                    .slot_site(literal.clause.clause.index(), literal.slot.position())
                    .ok_or_else(|| internal("literal slot without atoms"))?;
                for index in site.atoms() {
                    let id = self
                        .atom_ids
                        .get(index)
                        .copied()
                        .ok_or_else(|| internal("value path after its reference"))?;
                    ids.push(id);
                }
            } else {
                let Some(GlyphSlot::Glyph(path)) = self.view.plain.glyph_slot(literal) else {
                    return Err(internal("previous literal slot is not a glyph"));
                };
                for symbol in std::iter::once(&path.root).chain(path.qualifiers.iter()) {
                    let text = self
                        .view
                        .plain
                        .interner()
                        .resolve(*symbol)
                        .ok_or_else(|| internal("unknown symbol"))?;
                    let atom = parse_known(text)?;
                    let id = self
                        .txn
                        .find_plain(&atom)
                        .ok_or_else(|| internal("visible atom missing from the symbol table"))?;
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    /// Maps one atom and records its entry.
    fn atom(&mut self, site: &AtomSite) -> Result<usize, GhostError> {
        let (input, class) = parse_atom(self.view.text, site, self.snapshot)?;
        let output = match class {
            Class::Domain(index) => self.map_domain(index)?,
            Class::Numeral(_) => self.map_numeral(&input)?,
            Class::Ephemeral => self.map_ephemeral(&input)?,
        };
        let (start, end) = (site.start, site.end);
        self.edits.push(Edit {
            start,
            end,
            replacement: output.to_string(),
        });
        let (plain, cipher) = match self.direction {
            Direction::Forward => (input, output),
            Direction::Inverse => (output, input),
        };
        self.txn.bind(plain, cipher)
    }

    /// Live bindings of `(plain, cipher)` atoms filtered by `select`.
    fn bindings<T>(&self, select: impl Fn(&Atom) -> Option<T>) -> Vec<(T, T)> {
        self.txn
            .live_entries()
            .filter_map(|(_, entry)| Some((select(&entry.plain)?, select(&entry.cipher)?)))
            .collect()
    }

    fn other_side<T: Copy>(&self, pairs: &[(T, T)], index: usize) -> Result<T, GhostError> {
        let (plain, cipher) = pairs
            .get(index)
            .copied()
            .ok_or_else(|| internal("binding index out of range"))?;
        Ok(match self.direction {
            Direction::Forward => cipher,
            Direction::Inverse => plain,
        })
    }

    fn map_domain(&mut self, index: usize) -> Result<Atom, GhostError> {
        let domain = &self.snapshot.domain;
        let images = self
            .keystream
            .permutation(CLASS_GLYPH, self.positions.glyph, domain.len())?;
        self.positions.glyph = self.positions.glyph.saturating_add(1);
        let mut preimages = vec![0_usize; images.len()];
        for (source, image) in images.iter().enumerate() {
            if let Some(slot) = preimages.get_mut(*image) {
                *slot = source;
            }
        }
        let pairs = self.bindings(|atom| match self.snapshot.class(atom) {
            Some(Class::Domain(index)) => Some(index),
            _ => None,
        });
        let refs: Vec<(&usize, &usize)> = pairs.iter().map(|(p, c)| (p, c)).collect();
        let lookup = |table: &[usize], x: &usize| {
            table
                .get(*x)
                .copied()
                .ok_or_else(|| internal("index outside the domain"))
        };
        let image = match self.direction {
            Direction::Forward => forward(&index, &refs, |x| lookup(&images, x))?,
            Direction::Inverse => inverse(&index, &refs, |x| lookup(&preimages, x))?,
        };
        let mapped = match image {
            Image::Bound(j) => self.other_side(&pairs, j)?,
            Image::Free(mapped) => mapped,
        };
        domain
            .get(mapped)
            .map(|key| Atom::Key(*key))
            .ok_or_else(|| internal("mapped key outside the domain"))
    }

    fn map_numeral(&mut self, input: &Atom) -> Result<Atom, GhostError> {
        let Atom::Numeral(digits) = input else {
            return Err(internal("numeral class without digits"));
        };
        let base = self
            .snapshot
            .dti
            .universal()
            .numeral()
            .map(|system| system.base)
            .ok_or_else(|| internal("numeral without numeral system"))?;
        let position = self.positions.digit;
        let length = digits.len();
        self.positions.digit = position.saturating_add(u64::try_from(length).unwrap_or(u64::MAX));
        let refs: Vec<(&[u8], &[u8])> = self
            .txn
            .live_entries()
            .filter_map(|(_, entry)| match (&entry.plain, &entry.cipher) {
                (Atom::Numeral(p), Atom::Numeral(c)) if p.len() == length => {
                    Some((p.as_ref(), c.as_ref()))
                }
                _ => None,
            })
            .collect();
        let keystream = self.keystream;
        let image = match self.direction {
            Direction::Forward => forward(digits.as_ref(), &refs, |x: &[u8]| {
                map_digits(keystream, x, base, position, Direction::Forward)
            })?,
            Direction::Inverse => inverse(digits.as_ref(), &refs, |x: &[u8]| {
                map_digits(keystream, x, base, position, Direction::Inverse)
            })?,
        };
        let mapped: Box<[u8]> = match image {
            Image::Bound(j) => {
                let (plain, cipher) = refs
                    .get(j)
                    .copied()
                    .ok_or_else(|| internal("binding index out of range"))?;
                match self.direction {
                    Direction::Forward => Box::from(cipher),
                    Direction::Inverse => Box::from(plain),
                }
            }
            Image::Free(mapped) => mapped.into_boxed_slice(),
        };
        Ok(Atom::Numeral(mapped))
    }

    fn map_ephemeral(&mut self, input: &Atom) -> Result<Atom, GhostError> {
        let Atom::Key(key) = input else {
            return Err(internal("ephemeral class without key"));
        };
        let feistel = Feistel::new(self.keystream, self.positions.ephemeral);
        self.positions.ephemeral = self.positions.ephemeral.saturating_add(1);
        let pairs = self.bindings(|atom| match atom {
            Atom::Key(key) if self.snapshot.class(atom) == Some(Class::Ephemeral) => Some(*key),
            _ => None,
        });
        let refs: Vec<(&DtiKey, &DtiKey)> = pairs.iter().map(|(p, c)| (p, c)).collect();
        let snapshot = self.snapshot;
        let in_snapshot = |key: &DtiKey| snapshot.contains(key);
        let image = match self.direction {
            Direction::Forward => forward(key, &refs, |x| {
                feistel.permute(x, Direction::Forward, &in_snapshot)
            })?,
            Direction::Inverse => inverse(key, &refs, |x| {
                feistel.permute(x, Direction::Inverse, &in_snapshot)
            })?,
        };
        let mapped = match image {
            Image::Bound(j) => self.other_side(&pairs, j)?,
            Image::Free(mapped) => mapped,
        };
        Ok(Atom::Key(mapped))
    }
}
