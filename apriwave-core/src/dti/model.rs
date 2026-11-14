//! DTI data model and joined view (Phase B, D1–D6, B3–B9, B11).
//!
//! Two distinct authorities are kept separate in memory:
//! - [`UniversalSnapshot`]: read-only, replaceable as a whole on update;
//! - [`LocalOverlay`]: enabled universal keys, local lexical forms, local concepts
//!   and the unknown-key policy.
//!
//! [`Dti`] joins them for lookups without ever merging or rewriting either side.

use crate::prelude::*;

use alloc::collections::{BTreeMap, BTreeSet};

use super::key::DtiKey;

/// Generative morphology flags (B9: stored only in Phase B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MorphFlags {
    pub aggl: bool,
    pub contr: bool,
    pub union: bool,
    pub aggl_num: bool,
}

/// Language-neutral concept record.
#[derive(Debug, PartialEq, Eq)]
pub struct ConceptRecord {
    pub key: DtiKey,
    /// Parent concept used for graceful degradation (D3).
    pub base: Option<DtiKey>,
    pub param: Option<DtiKey>,
    pub morph: MorphFlags,
    /// Deprecation pointer (D2): the key itself stays valid forever.
    pub superseded_by: Option<DtiKey>,
    /// Opaque metadata `attr:k§v`, carried without interpretation (B8).
    pub attrs: Box<[(Box<str>, Box<str>)]>,
}

/// Lexical forms of a concept in one language.
#[derive(Debug, PartialEq, Eq)]
pub struct LexicalEntry {
    pub key: DtiKey,
    /// Primary form: a valid glyph path (Q7), unique per language (B3).
    pub primary: Box<str>,
    /// Human form, possibly with spaces (editor and views only).
    pub display: Option<Box<str>>,
    /// Input synonyms (glyph paths), possibly shared by several keys (B3).
    pub synonyms: Box<[Box<str>]>,
}

/// Lexicon of one language.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Lexicon {
    by_key: BTreeMap<DtiKey, LexicalEntry>,
    by_primary: BTreeMap<Box<str>, DtiKey>,
    by_synonym: BTreeMap<Box<str>, Vec<DtiKey>>,
    by_display: BTreeMap<Box<str>, Vec<DtiKey>>,
}

impl Lexicon {
    /// Inserts an entry. Returns `false` (and leaves the lexicon unchanged) if the
    /// key or the primary form is already present.
    pub(crate) fn insert(&mut self, entry: LexicalEntry) -> bool {
        if self.by_key.contains_key(&entry.key) || self.by_primary.contains_key(&entry.primary) {
            return false;
        }
        self.by_primary
            .insert(Box::from(&*entry.primary), entry.key);
        for synonym in entry.synonyms.iter() {
            push_unique(
                self.by_synonym.entry(Box::from(&**synonym)).or_default(),
                entry.key,
            );
        }
        if let Some(display) = &entry.display {
            push_unique(
                self.by_display.entry(Box::from(&**display)).or_default(),
                entry.key,
            );
        }
        self.by_key.insert(entry.key, entry);
        true
    }

    pub fn entry(&self, key: &DtiKey) -> Option<&LexicalEntry> {
        self.by_key.get(key)
    }

    pub fn primary(&self, form: &str) -> Option<DtiKey> {
        self.by_primary.get(form).copied()
    }

    pub fn synonyms(&self, form: &str) -> &[DtiKey] {
        self.by_synonym.get(form).map_or(&[], Vec::as_slice)
    }

    pub fn displays(&self, form: &str) -> &[DtiKey] {
        self.by_display.get(form).map_or(&[], Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.by_key.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

fn push_unique(keys: &mut Vec<DtiKey>, key: DtiKey) {
    if !keys.contains(&key) {
        keys.push(key);
    }
}

/// Numeral system declared by the snapshot (B4, option 1): ASCII digits `0..base`
/// are the exchange convention; `digits[i]` is the concept of digit `i`.
#[derive(Debug, PartialEq, Eq)]
pub struct NumeralSystem {
    pub base: u8,
    pub digits: Box<[DtiKey]>,
}

/// Read-only universal snapshot.
#[derive(Debug, PartialEq, Eq)]
pub struct UniversalSnapshot {
    pub(crate) version: u64,
    pub(crate) language: Box<str>,
    pub(crate) numeral: Option<NumeralSystem>,
    /// Concept records by key. Public only for the frozen `apriwave` Ghost
    /// module; use [`UniversalSnapshot::concepts`] instead.
    pub concepts: BTreeMap<DtiKey, ConceptRecord>,
    pub(crate) lexicon: Lexicon,
}

impl UniversalSnapshot {
    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub const fn numeral(&self) -> Option<&NumeralSystem> {
        self.numeral.as_ref()
    }

    pub fn concept(&self, key: &DtiKey) -> Option<&ConceptRecord> {
        self.concepts.get(key)
    }

    pub const fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    /// Every concept record, in key order.
    pub const fn concepts(&self) -> &BTreeMap<DtiKey, ConceptRecord> {
        &self.concepts
    }

    pub fn concept_count(&self) -> usize {
        self.concepts.len()
    }
}

/// Behaviour for keys that are unknown or not enabled locally (D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownKeyPolicy {
    /// Walk the `base:` chain up to the nearest enabled concept.
    DegradeToBase,
    /// Report the key as unknown.
    MarkUnknown,
    /// Reject the key.
    Reject,
}

/// Local overlay: user choices on top of the snapshot.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalOverlay {
    pub(crate) universal_version: u64,
    pub(crate) language: Box<str>,
    pub(crate) policy: UnknownKeyPolicy,
    pub(crate) enabled: BTreeSet<DtiKey>,
    pub(crate) local: BTreeMap<DtiKey, ConceptRecord>,
    pub(crate) lexicon: Lexicon,
}

impl LocalOverlay {
    pub const fn universal_version(&self) -> u64 {
        self.universal_version
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub const fn policy(&self) -> UnknownKeyPolicy {
        self.policy
    }

    pub const fn lexicon(&self) -> &Lexicon {
        &self.lexicon
    }

    pub fn local_concept(&self, key: &DtiKey) -> Option<&ConceptRecord> {
        self.local.get(key)
    }
}

/// Result of looking up a lexical form.
#[derive(Debug, PartialEq, Eq)]
pub enum FormResolution {
    Unique(DtiKey),
    /// Several concepts share the form (synonym or display): the editor must ask (B3).
    Ambiguous(Vec<DtiKey>),
    Unknown,
}

/// Result of resolving a key against the local overlay (D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyResolution {
    /// Enabled universal key or local concept.
    Enabled(DtiKey),
    /// Not enabled: degraded to the nearest enabled `base:` ancestor.
    Degraded {
        requested: DtiKey,
        to: DtiKey,
    },
    Unknown,
    Rejected,
}

/// Joined view of the two authorities.
#[derive(Debug, PartialEq, Eq)]
pub struct Dti {
    pub(crate) universal: UniversalSnapshot,
    pub(crate) overlay: LocalOverlay,
}

impl Dti {
    pub const fn universal(&self) -> &UniversalSnapshot {
        &self.universal
    }

    pub const fn overlay(&self) -> &LocalOverlay {
        &self.overlay
    }

    /// Concept record from either authority.
    pub fn concept(&self, key: &DtiKey) -> Option<&ConceptRecord> {
        self.overlay
            .local
            .get(key)
            .or_else(|| self.universal.concepts.get(key))
    }

    /// `true` for enabled universal keys and for local concepts.
    pub fn is_enabled(&self, key: &DtiKey) -> bool {
        self.overlay.enabled.contains(key) || self.overlay.local.contains_key(key)
    }

    /// Looks up a lexical form in the local lexicon and in the exchange-language
    /// lexicon. Primary forms are tried first; if they map to different keys in
    /// the two languages the result is ambiguous (no language has priority).
    pub fn resolve_form(&self, form: &str) -> FormResolution {
        let mut primaries: Vec<DtiKey> = Vec::new();
        for key in [
            self.overlay.lexicon.primary(form),
            self.universal.lexicon.primary(form),
        ]
        .into_iter()
        .flatten()
        {
            push_unique(&mut primaries, key);
        }
        if !primaries.is_empty() {
            return collapse(primaries);
        }
        let mut aliases: Vec<DtiKey> = Vec::new();
        for lexicon in [&self.overlay.lexicon, &self.universal.lexicon] {
            for key in lexicon.synonyms(form).iter().chain(lexicon.displays(form)) {
                push_unique(&mut aliases, *key);
            }
        }
        collapse(aliases)
    }

    /// Looks up a **primary** form only (local and exchange-language lexicons, no
    /// priority between the languages). Canonical text admits only primary forms:
    /// synonyms and `display` forms are normalized by the editor before stage 1.
    pub fn resolve_primary(&self, form: &str) -> FormResolution {
        let mut primaries: Vec<DtiKey> = Vec::new();
        for key in [
            self.overlay.lexicon.primary(form),
            self.universal.lexicon.primary(form),
        ]
        .into_iter()
        .flatten()
        {
            push_unique(&mut primaries, key);
        }
        collapse(primaries)
    }

    /// Enabled concepts composed as `base:<base>` + `param:<param>`, ordered by key
    /// (structural composition used by the set operands `<K1>§<K2>`, §14).
    pub fn compounds(&self, base: &DtiKey, param: &DtiKey) -> Vec<DtiKey> {
        let enabled = self.overlay.enabled.iter().chain(self.overlay.local.keys());
        let mut out: Vec<DtiKey> = enabled
            .filter(|key| {
                self.concept(key)
                    .is_some_and(|c| c.base == Some(*base) && c.param == Some(*param))
            })
            .copied()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Enabled concepts that have `key` in their `base:` chain (B3, S1). The walk
    /// of every chain is bounded by the number of concepts (cycles are rejected at
    /// load time). The result is ordered by key.
    pub fn descendants(&self, key: &DtiKey) -> alloc::collections::BTreeSet<DtiKey> {
        let limit = self
            .universal
            .concepts
            .len()
            .saturating_add(self.overlay.local.len());
        let enabled = self.overlay.enabled.iter().chain(self.overlay.local.keys());
        let mut out = alloc::collections::BTreeSet::new();
        for candidate in enabled {
            let mut current = self.concept(candidate).and_then(|c| c.base);
            for _ in 0..limit {
                let Some(ancestor) = current else {
                    break;
                };
                if ancestor == *key {
                    out.insert(*candidate);
                    break;
                }
                current = self.concept(&ancestor).and_then(|c| c.base);
            }
        }
        out
    }

    /// Resolves a key with the overlay policy (D3). The `base:` walk is iterative
    /// and bounded by the number of concepts (cycles are rejected at load time).
    pub fn resolve_key(&self, key: &DtiKey) -> KeyResolution {
        if self.is_enabled(key) {
            return KeyResolution::Enabled(*key);
        }
        match self.overlay.policy {
            UnknownKeyPolicy::Reject => KeyResolution::Rejected,
            UnknownKeyPolicy::MarkUnknown => KeyResolution::Unknown,
            UnknownKeyPolicy::DegradeToBase => {
                let limit = self
                    .universal
                    .concepts
                    .len()
                    .saturating_add(self.overlay.local.len());
                let mut current = self.concept(key).and_then(|c| c.base);
                for _ in 0..limit {
                    let Some(candidate) = current else {
                        break;
                    };
                    if self.is_enabled(&candidate) {
                        return KeyResolution::Degraded {
                            requested: *key,
                            to: candidate,
                        };
                    }
                    current = self.concept(&candidate).and_then(|c| c.base);
                }
                KeyResolution::Unknown
            }
        }
    }

    /// Primary form of a key: local language first, then exchange language.
    pub fn primary_form(&self, key: &DtiKey) -> Option<&str> {
        self.overlay
            .lexicon
            .entry(key)
            .or_else(|| self.universal.lexicon.entry(key))
            .map(|entry| &*entry.primary)
    }

    /// Human display form of a key, if declared.
    pub fn display_form(&self, key: &DtiKey) -> Option<&str> {
        self.overlay
            .lexicon
            .entry(key)
            .and_then(|e| e.display.as_deref())
            .or_else(|| {
                self.universal
                    .lexicon
                    .entry(key)
                    .and_then(|e| e.display.as_deref())
            })
    }
}

fn collapse(mut keys: Vec<DtiKey>) -> FormResolution {
    match keys.len() {
        0 => FormResolution::Unknown,
        1 => keys
            .pop()
            .map_or(FormResolution::Unknown, FormResolution::Unique),
        _ => {
            keys.sort();
            FormResolution::Ambiguous(keys)
        }
    }
}
