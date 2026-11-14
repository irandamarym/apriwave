//! G1 (option 2): the injective symbol table of the visible antecedents.
//!
//! The table pairs plaintext atoms with ciphertext atoms (`plain ↔ cipher`,
//! one-to-one: a plaintext atom has at most one live cipher atom and vice
//! versa). Every live entry is a binding of the chain walk (`bijection`
//! module): while an entry is live, the plaintext atom always maps to its cipher
//! atom and no other plaintext atom can map to that cipher atom. Entries are
//! reference counted by their **holders**:
//!
//! - the block being written (every atom joins the table when it is mapped);
//! - every block visible as an antecedent (R6 search scope: the active DSP
//!   stack, the level-0 blocks of the current period, the level-0 blocks carried
//!   by `/^/`), holding the atoms of the value paths of its pairs (literal paths,
//!   variable values and anaphora targets);
//! - every live `def` variable, holding its name and the atoms of its values
//!   (R13 can bring them back into visibility at any time);
//! - every open anchor, holding its id atoms (rules 1 and 2).
//!
//! An entry whose holders are all released (DSP pruning, `end ~`, `^/`, `^^/`,
//! `abse ~`, period end for anchors) disappears and its atoms become free again.

use std::fmt;

use crate::dti::{Dti, DtiKey};

use super::GhostError;

/// Value of an atom of the indexed form.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Atom {
    /// A valid DTI key (snapshot key or ephemeral key).
    Key(DtiKey),
    /// Digit values of a numeral (all below the snapshot base).
    Numeral(Box<[u8]>),
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => f.write_str(key.as_str()),
            Self::Numeral(digits) => digits
                .iter()
                .try_for_each(|digit| write!(f, "{}", char::from(b'0'.wrapping_add(*digit)))),
        }
    }
}

/// Permutation class of an atom (identical for a plaintext atom and its image).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Snapshot key: index in the sorted domain.
    Domain(usize),
    /// Key outside the universal snapshot (ephemeral or local key).
    Ephemeral,
    /// Numeral with this number of digits.
    Numeral(usize),
}

/// Snapshot context of the classification.
pub struct Snapshot<'d> {
    pub dti: &'d Dti,
    /// Sorted domain (non-deprecated universal keys).
    pub domain: Vec<DtiKey>,
}

impl Snapshot<'_> {
    /// `true` if the key belongs to the universal snapshot (deprecated included).
    pub fn contains(&self, key: &DtiKey) -> bool {
        self.dti.universal().concept(key).is_some()
    }

    pub fn class(&self, atom: &Atom) -> Option<Class> {
        match atom {
            Atom::Numeral(digits) => Some(Class::Numeral(digits.len())),
            Atom::Key(key) => match self.domain.binary_search(key) {
                Ok(index) => Some(Class::Domain(index)),
                Err(_) if self.contains(key) => None,
                Err(_) => Some(Class::Ephemeral),
            },
        }
    }
}

/// One `plain ↔ cipher` pair.
#[derive(Debug)]
pub struct Entry {
    pub plain: Atom,
    pub cipher: Atom,
}

/// A live `def` variable: entry of its name and entries of its values.
#[derive(Debug, Default)]
pub struct VariableHold {
    pub name: usize,
    pub values: Vec<usize>,
}

/// Persistent part of the table (state of a `GhostSession` between periods).
#[derive(Debug, Default)]
pub struct SymbolTable {
    /// Entry slab (ids are stable; `None` = released).
    entries: Vec<Option<Entry>>,
    /// Holder count of each slab slot.
    holds: Vec<u32>,
    pub(super) variables: Vec<VariableHold>,
    /// Holds of the level-0 blocks carried by `/^/` (one list per block).
    pub(super) carried: Vec<Vec<usize>>,
}

impl SymbolTable {
    /// Number of live entries.
    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Live pairs `(plain, cipher)`.
    pub fn pairs(&self) -> impl Iterator<Item = (&Atom, &Atom)> {
        self.entries
            .iter()
            .flatten()
            .map(|entry| (&entry.plain, &entry.cipher))
    }

    /// Cipher atom bound to `plain`, if any.
    pub fn cipher_of(&self, plain: &Atom) -> Option<&Atom> {
        self.pairs()
            .find(|(candidate, _)| *candidate == plain)
            .map(|(_, cipher)| cipher)
    }

    /// Number of live `def` variables.
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Number of carried antecedent blocks.
    pub fn carried_count(&self) -> usize {
        self.carried.len()
    }

    /// Applies the changes of a successful period.
    pub(super) fn commit(&mut self, changes: Changes) -> Result<(), GhostError> {
        let Changes {
            holds,
            new_entries,
            variables,
            carried,
        } = changes;
        self.entries.extend(new_entries.into_iter().map(Some));
        if holds.len() != self.entries.len() {
            return Err(GhostError::Internal {
                context: "symbol table transaction out of sync",
            });
        }
        for (slot, count) in self.entries.iter_mut().zip(holds.iter()) {
            if *count == 0 {
                *slot = None;
            }
        }
        self.holds = holds;
        // Trailing released slots can be dropped (ids of live entries are kept).
        while matches!(self.entries.last(), Some(None)) {
            self.entries.pop();
            self.holds.pop();
        }
        self.variables = variables;
        self.carried = carried;
        Ok(())
    }
}

/// Changes of one period, detached from the table borrow.
pub struct Changes {
    holds: Vec<u32>,
    new_entries: Vec<Entry>,
    variables: Vec<VariableHold>,
    carried: Vec<Vec<usize>>,
}

/// Working state of one period over a borrowed table: the table itself is
/// modified only by [`SymbolTable::commit`], so a failed period leaves it
/// untouched.
pub struct Txn<'t> {
    base: &'t SymbolTable,
    holds: Vec<u32>,
    new_entries: Vec<Entry>,
    pub(super) variables: Vec<VariableHold>,
    pub(super) carried: Vec<Vec<usize>>,
}

const fn internal(context: &'static str) -> GhostError {
    GhostError::Internal { context }
}

impl<'t> Txn<'t> {
    /// Starts a transaction. Holder lists are copied (plain `usize` ids) so that
    /// the persistent table stays intact until commit.
    pub fn new(base: &'t SymbolTable) -> Self {
        let mut holds: Vec<u32> = base.holds.to_vec();
        holds.resize(base.entries.len(), 0);
        Self {
            base,
            holds,
            new_entries: Vec::new(),
            variables: base
                .variables
                .iter()
                .map(|v| VariableHold {
                    name: v.name,
                    values: v.values.to_vec(),
                })
                .collect(),
            carried: base.carried.iter().map(|block| block.to_vec()).collect(),
        }
    }

    /// Detaches the changes (to be applied with [`SymbolTable::commit`]).
    pub fn into_changes(self) -> Changes {
        Changes {
            holds: self.holds,
            new_entries: self.new_entries,
            variables: self.variables,
            carried: self.carried,
        }
    }

    fn entry(&self, id: usize) -> Option<&Entry> {
        match id.checked_sub(self.base.entries.len()) {
            None => self.base.entries.get(id).and_then(Option::as_ref),
            Some(offset) => self.new_entries.get(offset),
        }
    }

    fn live(&self, id: usize) -> bool {
        self.holds.get(id).is_some_and(|count| *count > 0)
    }

    /// Live entries `(id, entry)`.
    pub fn live_entries(&self) -> impl Iterator<Item = (usize, &Entry)> {
        (0..self.holds.len()).filter_map(|id| {
            if self.live(id) {
                self.entry(id).map(|entry| (id, entry))
            } else {
                None
            }
        })
    }

    /// Live entry whose plaintext atom is `plain`.
    pub fn find_plain(&self, plain: &Atom) -> Option<usize> {
        self.live_entries()
            .find(|(_, entry)| entry.plain == *plain)
            .map(|(id, _)| id)
    }

    /// Records the pair `(plain, cipher)` and returns its entry id (not yet
    /// held). The pair must agree with the live entries (the chain walk
    /// guarantees it).
    pub fn bind(&mut self, plain: Atom, cipher: Atom) -> Result<usize, GhostError> {
        if let Some(id) = self.find_plain(&plain) {
            let same = self.entry(id).is_some_and(|entry| entry.cipher == cipher);
            return if same {
                Ok(id)
            } else {
                Err(internal("plaintext atom bound to two cipher atoms"))
            };
        }
        if self.live_entries().any(|(_, entry)| entry.cipher == cipher) {
            return Err(internal("cipher atom bound to two plaintext atoms"));
        }
        let id = self.holds.len();
        self.new_entries.push(Entry { plain, cipher });
        self.holds.push(0);
        Ok(id)
    }

    pub fn acquire(&mut self, id: usize) -> Result<(), GhostError> {
        let count = self
            .holds
            .get_mut(id)
            .ok_or_else(|| internal("unknown symbol entry"))?;
        *count = count.saturating_add(1);
        Ok(())
    }

    pub fn acquire_all(&mut self, ids: &[usize]) -> Result<(), GhostError> {
        ids.iter().try_for_each(|id| self.acquire(*id))
    }

    pub fn release(&mut self, id: usize) -> Result<(), GhostError> {
        let count = self
            .holds
            .get_mut(id)
            .ok_or_else(|| internal("unknown symbol entry"))?;
        *count = count
            .checked_sub(1)
            .ok_or_else(|| internal("symbol entry released twice"))?;
        Ok(())
    }

    pub fn release_all(&mut self, ids: &[usize]) -> Result<(), GhostError> {
        ids.iter().try_for_each(|id| self.release(*id))
    }
}
