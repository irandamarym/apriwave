//! Deterministic binary form of the DTI authorities (`.dti.bin`), on byte
//! buffers only (`no_std`): the file-system cache API lives in the `apriwave`
//! crate (`apriwave::dti::load_or_compile_universal_path`, …).
//!
//! The binary is a compiled cache of an `.awdti` source: it records the FNV-1a
//! 64 hash of the source bytes so a stale cache can be detected and rebuilt.
//!
//! # Layout (all integers little-endian)
//!
//! Header:
//! - magic `b"AWDTIBIN"` (8 bytes);
//! - format version `u32` (= [`BINARY_FORMAT_VERSION`]);
//! - kind `u8` (`0` = universal, `1` = local);
//! - source hash `u64` ([`fnv1a_64`] of the `.awdti` source bytes).
//!
//! Universal metadata: snapshot version `u64`, language `str`, numeral flag
//! `u8` (`0`/`1`; when `1`: base `u8` followed by `base` keys), record count
//! `u32`.
//!
//! Local metadata: `#universal` version `u64`, language `str`, policy `u8`
//! (`0` = degrade, `1` = unknown, `2` = reject), record count `u32`.
//!
//! Then one column per field, each holding exactly `count` entries in record
//! order:
//! - universal: keys, primary forms (`str`), display forms (`opt str`),
//!   synonyms (`list str`), base/param (`opt key`, `opt key`), morph flags
//!   (`u8`), superseded_by (`opt key`), attributes (`list (str, str)`);
//! - local: record kinds (`u8`: `0` = `use`, `1` = `new`), keys, forms
//!   (`opt str`: `form:` of `use`, primary of `new`), display forms
//!   (`opt str`), synonyms (`list str`), base (`opt key`), morph flags (`u8`).
//!
//! Encodings: `key` = 32 ASCII bytes; `str` = `u32` byte length + UTF-8;
//! `opt T` = `u8` tag (`0` absent, `1` present) + `T`; `list T` = `u32` count +
//! items; morph flags = bits `1` AGGL, `2` CONTR, `4` UNION, `8` AGGL_NUM. No
//! trailing bytes are allowed. Lookup indices are not stored: they are rebuilt
//! at load time.
//!
//! # Validation and determinism
//!
//! Decoding builds the model directly from the columns, without producing or
//! parsing any `.awdti` text, and validates it with the **same functions** as
//! the text parser (per-record field checks, header values, references,
//! deprecated keys, `base` cycles, anti-spoofing, unique primary forms), so a
//! binary can never yield a model that the text path would reject. Diagnostics
//! of a binary ([`DtiLoadError::Invalid`]) carry the 1-based record ordinal as
//! line (`0` for metadata). The model is then re-encoded and must match the input
//! byte for byte (otherwise [`DtiLoadError::NonCanonical`]): a binary is accepted
//! only in the canonical form produced by `to_binary`.
//!
//! Records are written in key order, refined so that every key list shared by
//! a synonym or a display form keeps its order (smallest key first among the
//! records allowed by those lists): the same model always yields the same
//! bytes, and loading reproduces the model exactly.

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt;

use crate::prelude::*;

use super::key::{DtiKey, KeyError, KEY_LENGTH};
use super::model::{Lexicon, LocalOverlay, MorphFlags, UniversalSnapshot, UnknownKeyPolicy};
use super::parse::{
    build_overlay, build_universal, check_language, check_numeral_base, check_numeral_digits,
    finish, form_validator, new_sink, validate_record_fields, DtiDiagnostic, DtiSource,
    FormValidator, Header, ParsedFile, RawRecord, RecordKind, Sink, Tags, MAX_SOURCE_BYTES,
};

/// Magic bytes at the start of every `.dti.bin` file.
pub const BINARY_MAGIC: [u8; 8] = *b"AWDTIBIN";
/// Current binary format version.
pub const BINARY_FORMAT_VERSION: u32 = 1;
/// Maximum accepted size of a binary (resource limit). Binaries may be
/// slightly larger than their source (length prefixes), hence the margin.
pub const MAX_BINARY_BYTES: usize = MAX_SOURCE_BYTES.saturating_mul(4);

const KIND_UNIVERSAL: u8 = 0;
const KIND_LOCAL: u8 = 1;
const RECORD_USE: u8 = 0;
const RECORD_NEW: u8 = 1;
const MORPH_AGGL: u8 = 1;
const MORPH_CONTR: u8 = 2;
const MORPH_UNION: u8 = 4;
const MORPH_AGGL_NUM: u8 = 8;
const MORPH_ALL: u8 = MORPH_AGGL | MORPH_CONTR | MORPH_UNION | MORPH_AGGL_NUM;
/// Smallest encoded universal record (key, empty primary, empty tags/lists).
const MIN_UNIVERSAL_RECORD: usize = KEY_LENGTH + 4 + 1 + 4 + 2 + 1 + 1 + 4;
/// Smallest encoded local record (kind, key, empty tags/lists).
const MIN_LOCAL_RECORD: usize = 1 + KEY_LENGTH + 1 + 1 + 4 + 1 + 1;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit hash (used to tie a binary to the source it was compiled from).
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

// ═══════════════════════════ Errors ═══════════════════════════

/// Failure to load a DTI from its binary form or from its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DtiLoadError {
    /// The input does not start with [`BINARY_MAGIC`].
    BadMagic,
    /// The binary format version is not supported.
    UnsupportedVersion { found: u32 },
    /// The binary holds the other authority (universal/local).
    WrongKind { found: u8 },
    /// The input ends before the value starting at `offset`.
    Truncated { offset: usize },
    /// Unexpected bytes after the last section.
    TrailingBytes { offset: usize },
    /// The string (or key) starting at `offset` is not valid UTF-8.
    InvalidUtf8 { offset: usize },
    /// A length or an offset does not fit in memory.
    LengthOverflow,
    /// The input exceeds [`MAX_BINARY_BYTES`].
    TooLarge { size: usize },
    /// The key starting at `offset` is invalid.
    InvalidKey { offset: usize, error: KeyError },
    /// An invalid tag, flag or character at `offset`.
    Malformed { offset: usize, what: &'static str },
    /// The binary decodes to a valid model but is not its canonical encoding.
    NonCanonical,
    /// The decoded records fail model validation (lines refer to the canonical
    /// text rendering of the binary).
    Invalid(Vec<DtiDiagnostic>),
    /// The `.awdti` source fails to compile.
    Source(Vec<DtiDiagnostic>),
}

impl fmt::Display for DtiLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => f.write_str("not a DTI binary (bad magic)"),
            Self::UnsupportedVersion { found } => {
                write!(f, "unsupported DTI binary format version {found}")
            }
            Self::WrongKind { found } => write!(f, "wrong DTI binary kind {found}"),
            Self::Truncated { offset } => write!(f, "DTI binary truncated at offset {offset}"),
            Self::TrailingBytes { offset } => {
                write!(f, "trailing bytes in DTI binary at offset {offset}")
            }
            Self::InvalidUtf8 { offset } => write!(f, "invalid UTF-8 at offset {offset}"),
            Self::LengthOverflow => f.write_str("length overflow in DTI binary"),
            Self::TooLarge { size } => write!(f, "DTI binary too large ({size} bytes)"),
            Self::InvalidKey { offset, error } => {
                write!(f, "invalid key at offset {offset}: {error}")
            }
            Self::Malformed { offset, what } => {
                write!(f, "malformed DTI binary at offset {offset}: {what}")
            }
            Self::NonCanonical => f.write_str("DTI binary is not in canonical form"),
            Self::Invalid(diagnostics) => write_diagnostics(f, "invalid DTI binary", diagnostics),
            Self::Source(diagnostics) => write_diagnostics(f, "invalid DTI source", diagnostics),
        }
    }
}

fn write_diagnostics(
    f: &mut fmt::Formatter<'_>,
    title: &str,
    diagnostics: &[DtiDiagnostic],
) -> fmt::Result {
    f.write_str(title)?;
    for diagnostic in diagnostics {
        write!(f, "; {diagnostic}")?;
    }
    Ok(())
}

impl core::error::Error for DtiLoadError {}

// ═══════════════════════════ Writer ═══════════════════════════

struct Writer {
    out: Vec<u8>,
}

impl Writer {
    fn header(kind: u8, source_hash: u64) -> Self {
        let mut writer = Self { out: Vec::new() };
        writer.out.extend_from_slice(&BINARY_MAGIC);
        writer.u32(BINARY_FORMAT_VERSION);
        writer.u8(kind);
        writer.u64(source_hash);
        writer
    }

    fn u8(&mut self, value: u8) {
        self.out.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.out.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.out.extend_from_slice(&value.to_le_bytes());
    }

    /// Lengths and counts of a valid model are bounded by [`MAX_SOURCE_BYTES`];
    /// a saturated value can only come from an invalid model and makes the
    /// binary undecodable (rejected, then recompiled) instead of panicking.
    fn len(&mut self, len: usize) {
        self.u32(u32::try_from(len).unwrap_or(u32::MAX));
    }

    fn str(&mut self, text: &str) {
        self.len(text.len());
        self.out.extend_from_slice(text.as_bytes());
    }

    fn key(&mut self, key: &DtiKey) {
        self.out.extend_from_slice(key.as_str().as_bytes());
    }

    fn flag(&mut self, present: bool) {
        self.u8(u8::from(present));
    }

    fn opt_str(&mut self, text: Option<&str>) {
        self.flag(text.is_some());
        if let Some(text) = text {
            self.str(text);
        }
    }

    fn opt_key(&mut self, key: Option<&DtiKey>) {
        self.flag(key.is_some());
        if let Some(key) = key {
            self.key(key);
        }
    }

    fn strs(&mut self, items: &[Box<str>]) {
        self.len(items.len());
        for item in items {
            self.str(item);
        }
    }

    fn morph(&mut self, morph: MorphFlags) {
        let mut bits = 0;
        for (set, bit) in [
            (morph.aggl, MORPH_AGGL),
            (morph.contr, MORPH_CONTR),
            (morph.union, MORPH_UNION),
            (morph.aggl_num, MORPH_AGGL_NUM),
        ] {
            if set {
                bits |= bit;
            }
        }
        self.u8(bits);
    }
}

// ═══════════════════════════ Reader ═══════════════════════════

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], DtiLoadError> {
        let offset = self.offset;
        let (head, _) = self
            .bytes
            .get(offset..)
            .and_then(|rest| rest.split_at_checked(len))
            .ok_or(DtiLoadError::Truncated { offset })?;
        self.offset = self
            .offset
            .checked_add(len)
            .ok_or(DtiLoadError::LengthOverflow)?;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DtiLoadError> {
        let offset = self.offset;
        self.take(N)?
            .try_into()
            .map_err(|_| DtiLoadError::Truncated { offset })
    }

    fn u8(&mut self) -> Result<u8, DtiLoadError> {
        self.array::<1>().map(u8::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, DtiLoadError> {
        self.array::<4>().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Result<u64, DtiLoadError> {
        self.array::<8>().map(u64::from_le_bytes)
    }

    /// Reads a `u32` count of items of at least `min_item` bytes each; the
    /// count is capped by the remaining input, so it is safe to preallocate.
    fn count(&mut self, min_item: usize) -> Result<usize, DtiLoadError> {
        let offset = self.offset;
        let count = usize::try_from(self.u32()?).map_err(|_| DtiLoadError::LengthOverflow)?;
        if count > self.remaining() / min_item.max(1) {
            return Err(DtiLoadError::Truncated { offset });
        }
        Ok(count)
    }

    /// Reads a string. Line breaks are rejected: no `.awdti` field contains one.
    fn str(&mut self) -> Result<&'a str, DtiLoadError> {
        let len = self.count(1)?;
        let offset = self.offset;
        let text = core::str::from_utf8(self.take(len)?)
            .map_err(|_| DtiLoadError::InvalidUtf8 { offset })?;
        if text.contains(['\n', '\r']) {
            return Err(DtiLoadError::Malformed {
                offset,
                what: "line break in string",
            });
        }
        Ok(text)
    }

    fn key(&mut self) -> Result<DtiKey, DtiLoadError> {
        let offset = self.offset;
        let text = core::str::from_utf8(self.take(KEY_LENGTH)?)
            .map_err(|_| DtiLoadError::InvalidUtf8 { offset })?;
        DtiKey::parse(text).map_err(|error| DtiLoadError::InvalidKey { offset, error })
    }

    fn flag(&mut self) -> Result<bool, DtiLoadError> {
        let offset = self.offset;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DtiLoadError::Malformed {
                offset,
                what: "invalid presence tag",
            }),
        }
    }

    fn opt_str(&mut self) -> Result<Option<&'a str>, DtiLoadError> {
        if self.flag()? {
            self.str().map(Some)
        } else {
            Ok(None)
        }
    }

    fn opt_key(&mut self) -> Result<Option<DtiKey>, DtiLoadError> {
        if self.flag()? {
            self.key().map(Some)
        } else {
            Ok(None)
        }
    }

    fn strs(&mut self) -> Result<Vec<&'a str>, DtiLoadError> {
        let count = self.count(4)?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(self.str()?);
        }
        Ok(items)
    }

    fn morph(&mut self) -> Result<MorphFlags, DtiLoadError> {
        let offset = self.offset;
        let bits = self.u8()?;
        if bits & !MORPH_ALL != 0 {
            return Err(DtiLoadError::Malformed {
                offset,
                what: "unknown morphology flag",
            });
        }
        Ok(MorphFlags {
            aggl: bits & MORPH_AGGL != 0,
            contr: bits & MORPH_CONTR != 0,
            union: bits & MORPH_UNION != 0,
            aggl_num: bits & MORPH_AGGL_NUM != 0,
        })
    }

    /// Reads `count` items with `read` (the count was capped by [`Self::count`]).
    fn column<T>(
        &mut self,
        count: usize,
        mut read: impl FnMut(&mut Self) -> Result<T, DtiLoadError>,
    ) -> Result<Vec<T>, DtiLoadError> {
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(read(self)?);
        }
        Ok(items)
    }

    fn finish(&self) -> Result<(), DtiLoadError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(DtiLoadError::TrailingBytes {
                offset: self.offset,
            })
        }
    }
}

/// Checks the size limit and the header; returns the stored source hash.
fn read_header(reader: &mut Reader<'_>, kind: u8) -> Result<u64, DtiLoadError> {
    if reader.bytes.len() > MAX_BINARY_BYTES {
        return Err(DtiLoadError::TooLarge {
            size: reader.bytes.len(),
        });
    }
    if reader.array::<8>()? != BINARY_MAGIC {
        return Err(DtiLoadError::BadMagic);
    }
    let version = reader.u32()?;
    if version != BINARY_FORMAT_VERSION {
        return Err(DtiLoadError::UnsupportedVersion { found: version });
    }
    let found = reader.u8()?;
    if found != kind {
        return Err(DtiLoadError::WrongKind { found });
    }
    reader.u64()
}

// ═══════════════════════════ Record order ═══════════════════════════

/// Deterministic record order: smallest key first, constrained so that every
/// key list of a synonym or display form keeps its order (those lists depend
/// on the insertion order). `keys` must be sorted.
fn record_order(keys: &[DtiKey], lexicon: &Lexicon) -> Vec<usize> {
    let index: BTreeMap<DtiKey, usize> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
    let mut successors: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); keys.len()];
    let mut indegree: Vec<usize> = vec![0; keys.len()];
    let mut seen_synonyms: BTreeSet<&str> = BTreeSet::new();
    let mut seen_displays: BTreeSet<&str> = BTreeSet::new();
    let mut chain = |list: &[DtiKey]| {
        for pair in list.windows(2) {
            let (Some(from), Some(to)) = (
                pair.first().and_then(|k| index.get(k)),
                pair.get(1).and_then(|k| index.get(k)),
            ) else {
                continue;
            };
            let added = successors.get_mut(*from).is_some_and(|s| s.insert(*to));
            if added {
                if let Some(degree) = indegree.get_mut(*to) {
                    *degree = degree.saturating_add(1);
                }
            }
        }
    };
    for key in keys {
        let Some(entry) = lexicon.entry(key) else {
            continue;
        };
        for synonym in entry.synonyms.iter() {
            if seen_synonyms.insert(&**synonym) {
                chain(lexicon.synonyms(synonym));
            }
        }
        if let Some(display) = entry.display.as_deref() {
            if seen_displays.insert(display) {
                chain(lexicon.displays(display));
            }
        }
    }

    let mut ready: BTreeSet<usize> = indegree
        .iter()
        .enumerate()
        .filter(|(_, degree)| **degree == 0)
        .map(|(i, _)| i)
        .collect();
    let mut order = Vec::with_capacity(keys.len());
    let mut emitted = vec![false; keys.len()];
    while let Some(next) = ready.pop_first() {
        order.push(next);
        if let Some(slot) = emitted.get_mut(next) {
            *slot = true;
        }
        for successor in successors.get(next).into_iter().flatten() {
            if let Some(degree) = indegree.get_mut(*successor) {
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.insert(*successor);
                }
            }
        }
    }
    // Lists built by insertion never form a cycle; keep the function total anyway.
    order.extend(
        emitted
            .iter()
            .enumerate()
            .filter(|(_, done)| !**done)
            .map(|(i, _)| i),
    );
    order
}

fn ordered<'k>(keys: &'k [DtiKey], lexicon: &Lexicon) -> impl Iterator<Item = &'k DtiKey> {
    record_order(keys, lexicon)
        .into_iter()
        .filter_map(|i| keys.get(i))
}

// ═══════════════════════════ Canonical text ═══════════════════════════

const fn policy_code(policy: UnknownKeyPolicy) -> u8 {
    match policy {
        UnknownKeyPolicy::DegradeToBase => 0,
        UnknownKeyPolicy::MarkUnknown => 1,
        UnknownKeyPolicy::Reject => 2,
    }
}

/// Morph tag of a decoded record: no flag is the same as an absent tag.
fn morph_tag(morph: MorphFlags) -> Option<MorphFlags> {
    (morph != MorphFlags::default()).then_some(morph)
}

/// 1-based record ordinal used as the "line" of binary diagnostics.
fn ordinal(index: usize) -> u32 {
    u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX)
}

/// Shared per-record checks, then insertion (a failing record is reported and
/// skipped, exactly as a failing line of the text parser).
fn push_record(
    records: &mut Vec<RawRecord>,
    record: RawRecord,
    sink: &mut Sink,
    forms: &mut FormValidator,
) {
    match validate_record_fields(&record, forms) {
        Ok(()) => records.push(record),
        Err(kind) => sink.report(record.line, kind),
    }
}

// ═══════════════════════════ Universal ═══════════════════════════

impl UniversalSnapshot {
    /// Deterministic binary encoding (see the module documentation).
    /// `source_hash` should be [`fnv1a_64`] of the source bytes.
    pub fn to_binary(&self, source_hash: u64) -> Vec<u8> {
        // `BTreeMap` keys are already in key order.
        let keys: Vec<DtiKey> = self.concepts.keys().copied().collect();
        let order: Vec<&DtiKey> = ordered(&keys, &self.lexicon).collect();
        let entries: Vec<_> = order.iter().map(|k| self.lexicon.entry(k)).collect();
        let concepts: Vec<_> = order.iter().map(|k| self.concepts.get(k)).collect();

        let mut w = Writer::header(KIND_UNIVERSAL, source_hash);
        w.u64(self.version);
        w.str(&self.language);
        w.flag(self.numeral.is_some());
        if let Some(numeral) = &self.numeral {
            w.u8(numeral.base);
            for digit in numeral.digits.iter() {
                w.key(digit);
            }
        }
        w.len(order.len());
        for key in &order {
            w.key(key);
        }
        // A missing entry or concept cannot occur in a validated snapshot; the
        // placeholders below would be rejected at load time.
        for entry in &entries {
            w.str(entry.map_or("", |e| &*e.primary));
        }
        for entry in &entries {
            w.opt_str(entry.and_then(|e| e.display.as_deref()));
        }
        for entry in &entries {
            w.strs(entry.map_or(&[], |e| &*e.synonyms));
        }
        for concept in &concepts {
            w.opt_key(concept.and_then(|c| c.base.as_ref()));
            w.opt_key(concept.and_then(|c| c.param.as_ref()));
        }
        for concept in &concepts {
            w.morph(concept.map(|c| c.morph).unwrap_or_default());
        }
        for concept in &concepts {
            w.opt_key(concept.and_then(|c| c.superseded_by.as_ref()));
        }
        for concept in &concepts {
            let attrs = concept.map_or(&[][..], |c| &*c.attrs);
            w.len(attrs.len());
            for (name, value) in attrs {
                w.str(name);
                w.str(value);
            }
        }
        w.out
    }

    /// Decodes and validates a binary produced by [`Self::to_binary`]; returns
    /// the snapshot and the stored source hash.
    pub fn from_binary(bytes: &[u8]) -> Result<(Self, u64), DtiLoadError> {
        let mut r = Reader::new(bytes);
        let hash = read_header(&mut r, KIND_UNIVERSAL)?;
        let version = r.u64()?;
        let language = r.str()?;
        let numeral = if r.flag()? {
            let base = r.u8()?;
            Some((base, r.column(usize::from(base), Reader::key)?))
        } else {
            None
        };
        let count = r.count(MIN_UNIVERSAL_RECORD)?;
        let keys = r.column(count, Reader::key)?;
        let primaries = r.column(count, Reader::str)?;
        let displays = r.column(count, Reader::opt_str)?;
        let synonyms = r.column(count, Reader::strs)?;
        let links = r.column(count, |r| Ok((r.opt_key()?, r.opt_key()?)))?;
        let morphs = r.column(count, Reader::morph)?;
        let superseded = r.column(count, Reader::opt_key)?;
        let attrs = r.column(count, |r| {
            let count = r.count(8)?;
            r.column(count, |r| Ok((r.str()?, r.str()?)))
        })?;
        r.finish()?;

        // The model is built directly from the columns and validated by the same
        // functions as the text parser (no `.awdti` text is produced).
        let mut sink = new_sink(DtiSource::Universal);
        let mut forms = form_validator();
        let mut header = Header {
            format: Some(()),
            role: Some(DtiSource::Universal),
            version: Some(version),
            ..Header::default()
        };
        match check_language(language) {
            Ok(()) => header.language = Some(Box::from(language)),
            Err(kind) => sink.report(0, kind),
        }
        if let Some((base, digits)) = numeral {
            match check_numeral_base(base).and_then(|()| check_numeral_digits(base, &digits)) {
                Ok(()) => header.numeral = Some((0, base, digits)),
                Err(kind) => sink.report(0, kind),
            }
        }
        let mut records = Vec::with_capacity(count);
        let rows = keys
            .iter()
            .zip(primaries)
            .zip(displays)
            .zip(synonyms)
            .zip(links)
            .zip(morphs)
            .zip(superseded)
            .zip(attrs)
            .enumerate();
        for (index, (((((((key, primary), display), syns), (base, param)), morph), sup), attrs)) in
            rows
        {
            let record = RawRecord {
                line: ordinal(index),
                kind: RecordKind::Rec,
                key: *key,
                primary: Some(Box::from(primary)),
                tags: Tags {
                    display: display.map(Box::from),
                    synonyms: syns.into_iter().map(Box::from).collect(),
                    base,
                    param,
                    morph: morph_tag(morph),
                    superseded_by: sup,
                    attrs: attrs
                        .into_iter()
                        .map(|(name, value)| (Box::from(name), Box::from(value)))
                        .collect(),
                    form: None,
                },
            };
            push_record(&mut records, record, &mut sink, &mut forms);
        }
        let snapshot = build_universal(ParsedFile { header, records }, &mut sink);
        let snapshot = finish(snapshot, sink).map_err(DtiLoadError::Invalid)?;
        if snapshot.to_binary(hash) != bytes {
            return Err(DtiLoadError::NonCanonical);
        }
        Ok((snapshot, hash))
    }
}

// ═══════════════════════════ Local ═══════════════════════════

impl LocalOverlay {
    /// Deterministic binary encoding (see the module documentation).
    /// `source_hash` should be [`fnv1a_64`] of the source bytes.
    pub fn to_binary(&self, source_hash: u64) -> Vec<u8> {
        let mut keys: Vec<DtiKey> = self
            .enabled
            .iter()
            .chain(self.local.keys())
            .copied()
            .collect();
        keys.sort_unstable();
        let order: Vec<&DtiKey> = ordered(&keys, &self.lexicon).collect();
        let entries: Vec<_> = order.iter().map(|k| self.lexicon.entry(k)).collect();
        let concepts: Vec<_> = order.iter().map(|k| self.local.get(k)).collect();

        let mut w = Writer::header(KIND_LOCAL, source_hash);
        w.u64(self.universal_version);
        w.str(&self.language);
        w.u8(policy_code(self.policy));
        w.len(order.len());
        for concept in &concepts {
            w.u8(if concept.is_some() {
                RECORD_NEW
            } else {
                RECORD_USE
            });
        }
        for key in &order {
            w.key(key);
        }
        for entry in &entries {
            w.opt_str(entry.map(|e| &*e.primary));
        }
        for entry in &entries {
            w.opt_str(entry.and_then(|e| e.display.as_deref()));
        }
        for entry in &entries {
            w.strs(entry.map_or(&[], |e| &*e.synonyms));
        }
        for concept in &concepts {
            w.opt_key(concept.and_then(|c| c.base.as_ref()));
        }
        for concept in &concepts {
            w.morph(concept.map(|c| c.morph).unwrap_or_default());
        }
        w.out
    }

    /// Decodes a binary produced by [`Self::to_binary`] and validates it
    /// against `universal`; returns the overlay and the stored source hash.
    pub fn from_binary(
        bytes: &[u8],
        universal: &UniversalSnapshot,
    ) -> Result<(Self, u64), DtiLoadError> {
        let mut r = Reader::new(bytes);
        let hash = read_header(&mut r, KIND_LOCAL)?;
        let universal_version = r.u64()?;
        let language = r.str()?;
        let policy_offset = r.offset;
        let policy = match r.u8()? {
            0 => UnknownKeyPolicy::DegradeToBase,
            1 => UnknownKeyPolicy::MarkUnknown,
            2 => UnknownKeyPolicy::Reject,
            _ => {
                return Err(DtiLoadError::Malformed {
                    offset: policy_offset,
                    what: "unknown policy",
                })
            }
        };
        let count = r.count(MIN_LOCAL_RECORD)?;
        let kinds = r.column(count, |r| {
            let offset = r.offset;
            match r.u8()? {
                RECORD_USE => Ok(RecordKind::Use),
                RECORD_NEW => Ok(RecordKind::New),
                _ => Err(DtiLoadError::Malformed {
                    offset,
                    what: "unknown record kind",
                }),
            }
        })?;
        let keys = r.column(count, Reader::key)?;
        let forms_column = r.column(count, Reader::opt_str)?;
        let displays = r.column(count, Reader::opt_str)?;
        let synonyms = r.column(count, Reader::strs)?;
        let bases = r.column(count, Reader::opt_key)?;
        let morphs = r.column(count, Reader::morph)?;
        r.finish()?;

        // Built directly from the columns, validated by the shared functions.
        let mut sink = new_sink(DtiSource::Local);
        let mut forms = form_validator();
        let mut header = Header {
            format: Some(()),
            role: Some(DtiSource::Local),
            universal: Some(universal_version),
            policy: Some(policy),
            ..Header::default()
        };
        match check_language(language) {
            Ok(()) => header.language = Some(Box::from(language)),
            Err(kind) => sink.report(0, kind),
        }
        let mut records = Vec::with_capacity(count);
        let rows = kinds
            .into_iter()
            .zip(keys)
            .zip(forms_column)
            .zip(displays)
            .zip(synonyms)
            .zip(bases)
            .zip(morphs)
            .enumerate();
        for (index, ((((((kind, key), form), display), syns), base), morph)) in rows {
            // `use`: the column holds the local `form:`; `new`: the primary form.
            let (primary, form) = match kind {
                RecordKind::Use => (None, form.map(Box::from)),
                RecordKind::New | RecordKind::Rec => (form.map(Box::from), None),
            };
            let record = RawRecord {
                line: ordinal(index),
                kind,
                key,
                primary,
                tags: Tags {
                    display: display.map(Box::from),
                    synonyms: syns.into_iter().map(Box::from).collect(),
                    base,
                    morph: morph_tag(morph),
                    form,
                    ..Tags::default()
                },
            };
            push_record(&mut records, record, &mut sink, &mut forms);
        }
        let overlay = build_overlay(ParsedFile { header, records }, universal, &mut sink);
        let overlay = finish(overlay, sink).map_err(DtiLoadError::Invalid)?;
        if overlay.to_binary(hash) != bytes {
            return Err(DtiLoadError::NonCanonical);
        }
        Ok((overlay, hash))
    }
}

// ═══════════════════════════ Cache helpers ═══════════════════════════

/// Stored source hash, if `bytes` has a valid header of the given kind.
fn stored_hash(bytes: &[u8], kind: u8) -> Option<u64> {
    read_header(&mut Reader::new(bytes), kind).ok()
}

/// Loads the snapshot from `binary` when it decodes and was compiled from
/// exactly `source` (FNV-1a 64 hash); returns `None` as second element in that
/// case. Otherwise (absent, corrupt or stale binary) compiles `source` and
/// returns the fresh binary.
pub fn load_or_compile_universal(
    source: &str,
    binary: Option<&[u8]>,
) -> Result<(UniversalSnapshot, Option<Vec<u8>>), DtiLoadError> {
    let hash = fnv1a_64(source.as_bytes());
    if let Some(bytes) = binary {
        if stored_hash(bytes, KIND_UNIVERSAL) == Some(hash) {
            if let Ok((snapshot, _)) = UniversalSnapshot::from_binary(bytes) {
                return Ok((snapshot, None));
            }
        }
    }
    let snapshot = UniversalSnapshot::parse(source).map_err(DtiLoadError::Source)?;
    let fresh = snapshot.to_binary(hash);
    Ok((snapshot, Some(fresh)))
}

/// Local counterpart of [`load_or_compile_universal`]; the binary is always
/// re-validated against `universal`.
pub fn load_or_compile_local(
    source: &str,
    binary: Option<&[u8]>,
    universal: &UniversalSnapshot,
) -> Result<(LocalOverlay, Option<Vec<u8>>), DtiLoadError> {
    let hash = fnv1a_64(source.as_bytes());
    if let Some(bytes) = binary {
        if stored_hash(bytes, KIND_LOCAL) == Some(hash) {
            if let Ok((overlay, _)) = LocalOverlay::from_binary(bytes, universal) {
                return Ok((overlay, None));
            }
        }
    }
    let overlay = LocalOverlay::parse(source, universal).map_err(DtiLoadError::Source)?;
    let fresh = overlay.to_binary(hash);
    Ok((overlay, Some(fresh)))
}

#[cfg(test)]
#[path = "binary_tests.rs"]
mod binary_tests;
