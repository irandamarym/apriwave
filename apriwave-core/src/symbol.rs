//! Interner of glyph segments (Q28).
//!
//! Every text is allocated **exactly once**, in `strings` (`Vec<Box<str>>`),
//! which resolves a `Symbol` in O(1). `index` holds only the symbols (4 bytes
//! each), sorted by their text: a binary search finds the symbol of a text with
//! a deterministic O(log n) worst case, immune to HashDoS by construction; an
//! insertion shifts at most `n` symbols. No `Arc`/`Rc` is needed, so the
//! interner is `Send + Sync` and builds on every Cortex-M (Cortex-M0+ has no
//! atomic compare-and-swap, hence no `alloc::sync`).
//! `rollback` enables atomic ingestion: a rejected input leaves no
//! residue in the interner.

use crate::prelude::*;

use core::cmp::Ordering;
use core::num::NonZeroU32;

/// Interned glyph segment. Comparison and copy in O(1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(NonZeroU32);

impl Symbol {
    /// Position in the string vector (`get() >= 1` by construction).
    fn slot(self) -> usize {
        // u32 → usize is lossless on the supported 32/64-bit platforms.
        (self.0.get() - 1) as usize
    }
}

/// The symbol space (`u32`) is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolSpaceExhausted;

/// Restore point of the interner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternerMark(usize);

#[derive(Debug, Default)]
pub struct Interner {
    strings: Vec<Box<str>>,
    /// Every symbol, sorted by its text in `strings`.
    index: Vec<Symbol>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Position of `text` in `index` (`Ok`) or where it would be inserted (`Err`).
    fn search(&self, text: &str) -> Result<usize, usize> {
        self.index
            .binary_search_by(|symbol| match self.resolve(*symbol) {
                Some(existing) => existing.cmp(text),
                // Unreachable by construction (every indexed symbol is resolvable);
                // ordering it first keeps the search total.
                None => Ordering::Less,
            })
    }

    /// Returns the symbol of `text`, creating it if absent.
    pub fn intern(&mut self, text: &str) -> Result<Symbol, SymbolSpaceExhausted> {
        let position = match self.search(text) {
            Ok(found) => {
                if let Some(existing) = self.index.get(found) {
                    return Ok(*existing);
                }
                found
            }
            Err(insert_at) => insert_at,
        };
        let raw = u32::try_from(self.strings.len())
            .ok()
            .and_then(|n| n.checked_add(1))
            .and_then(NonZeroU32::new)
            .ok_or(SymbolSpaceExhausted)?;
        let symbol = Symbol(raw);
        self.strings.push(Box::from(text));
        self.index.insert(position, symbol);
        Ok(symbol)
    }

    /// Looks up an existing symbol without creating it.
    pub fn lookup(&self, text: &str) -> Option<Symbol> {
        self.search(text)
            .ok()
            .and_then(|position| self.index.get(position).copied())
    }

    /// Text associated with the symbol (`None` if the symbol belongs to another interner
    /// or has been undone by `rollback`).
    pub fn resolve(&self, symbol: Symbol) -> Option<&str> {
        self.strings.get(symbol.slot()).map(|s| &**s)
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn mark(&self) -> InternerMark {
        InternerMark(self.strings.len())
    }

    /// Removes all symbols created after `mark` (texts and index entries).
    pub fn rollback(&mut self, mark: InternerMark) {
        if self.strings.len() > mark.0 {
            self.strings.truncate(mark.0);
            self.index.retain(|symbol| symbol.slot() < mark.0);
        }
    }
}
