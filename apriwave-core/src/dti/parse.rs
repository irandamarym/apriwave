//! `.awdti` text format: parsing and validation (Phase B).
//!
//! One record per line, fields separated by a single space. Only the values of
//! `display:` and `attr:` may contain spaces, enclosed in `"…"`; every other
//! field (kind, key, lexical forms, `form:`, `syn:`) is taken literally, so a
//! lexical form may contain any visible character, `"` included (v4.1). Lines
//! starting with `#` are header directives and must precede the records; empty
//! lines are ignored.
//!
//! Universal snapshot:
//! ```text
//! #awdti 1
//! #role universal
//! #version 42
//! #lang en
//! #numeral base:10 digits:<K0>,…,<K9>
//! rec <KEY> <primary> [display:"…"] [syn:a,b] [base:<KEY>] [param:<KEY>] [morph:+AGGL,…] [superseded_by:<KEY>] [attr:k§v]
//! ```
//! Local overlay:
//! ```text
//! #awdti 1
//! #role local
//! #universal 42
//! #lang it
//! #policy degrade
//! use <KEY> [form:<primary>] [display:"…"] [syn:…]
//! new <KEY> <primary> [base:<KEY>] [display:"…"] [syn:…] [morph:…]
//! label <KEY> <form>
//! ```
//!
//! v4.1: a lexical form is any non-empty token of visible characters (`§`,
//! `~` and `~~` included); only invisible characters are refused (D9). A `label` record censuses a local label: the
//! same form may coexist as a glyph and as a label. A census warning (never an
//! error) is recorded when a local glyph or label form is already the primary
//! form of another universal glyph in the exchange language.

use crate::prelude::*;

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt;

use super::key::{DtiKey, KeyError};
use super::model::{
    CensusKind, CensusWarning, ConceptRecord, Dti, LabelLexicon, LexicalEntry, Lexicon,
    LocalOverlay, MorphFlags, NumeralSystem, UniversalSnapshot, UnknownKeyPolicy,
};

/// Only supported value of the `#awdti` directive.
const FORMAT_VERSION: &str = "1";
/// Maximum accepted size of a single `.awdti` source (resource limit).
pub const MAX_SOURCE_BYTES: usize = 256 * 1024 * 1024;

// ═══════════════════════════ Diagnostics ═══════════════════════════

/// Which of the two files a diagnostic refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DtiSource {
    Universal,
    Local,
}

/// Errors detected while loading a DTI (kept apart from the 6 grammar error
/// codes, Q26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DtiErrorKind {
    SourceTooLarge,
    TooManyLines,
    MissingHeader {
        name: &'static str,
    },
    DuplicateHeader {
        name: &'static str,
    },
    UnknownHeader,
    HeaderAfterRecords,
    InvalidHeaderValue {
        name: &'static str,
    },
    WrongRole,
    UnsupportedFormatVersion,
    EmptyField,
    UnterminatedQuote,
    UnknownRecordKind,
    MissingField {
        name: &'static str,
    },
    UnexpectedField,
    InvalidKey(KeyError),
    DuplicateKey(DtiKey),
    InvalidForm,
    DuplicatePrimary,
    /// The form is already a label of this overlay (v4.1).
    DuplicateLabel,
    DuplicateSynonym,
    InvalidDisplay,
    InvalidAttribute,
    UnknownTag,
    DuplicateTag,
    TagNotAllowed,
    UnknownMorphFlag,
    DuplicateMorphFlag,
    OrphanReference(DtiKey),
    /// Anti-spoofing: a local concept reuses a universal key.
    LocalShadowsUniversal(DtiKey),
    BaseCycle(DtiKey),
    /// B6: `use` on a deprecated key; the successor is reported explicitly.
    SupersededKey {
        key: DtiKey,
        successor: DtiKey,
    },
    VersionMismatch {
        snapshot: u64,
        overlay: u64,
    },
    InvalidNumeral,
}

impl fmt::Display for DtiErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceTooLarge => f.write_str("source exceeds the maximum size"),
            Self::TooManyLines => f.write_str("too many lines"),
            Self::MissingHeader { name } => write!(f, "missing header #{name}"),
            Self::DuplicateHeader { name } => write!(f, "duplicate header #{name}"),
            Self::UnknownHeader => f.write_str("unknown header directive"),
            Self::HeaderAfterRecords => f.write_str("header directive after the first record"),
            Self::InvalidHeaderValue { name } => write!(f, "invalid value for header #{name}"),
            Self::WrongRole => f.write_str("file role does not match its position (universal/local)"),
            Self::UnsupportedFormatVersion => f.write_str("unsupported #awdti format version"),
            Self::EmptyField => f.write_str("empty field (fields are separated by a single space)"),
            Self::UnterminatedQuote => f.write_str("unterminated quoted value"),
            Self::UnknownRecordKind => f.write_str("unknown record kind"),
            Self::MissingField { name } => write!(f, "missing field <{name}>"),
            Self::UnexpectedField => f.write_str("unexpected positional field"),
            Self::InvalidKey(err) => write!(f, "{err}"),
            Self::DuplicateKey(key) => write!(f, "duplicate key {key}"),
            Self::InvalidForm => f.write_str(
                "lexical form is empty or contains a space or an invisible character",
            ),
            Self::DuplicateLabel => f.write_str("label form already censused in this overlay"),
            Self::DuplicatePrimary => f.write_str("primary form already used in this language"),
            Self::DuplicateSynonym => f.write_str("duplicate synonym in the same record"),
            Self::InvalidDisplay => f.write_str("invalid display form"),
            Self::InvalidAttribute => f.write_str("invalid attribute (expected attr:key§value)"),
            Self::UnknownTag => f.write_str("unknown tag"),
            Self::DuplicateTag => f.write_str("duplicate tag"),
            Self::TagNotAllowed => f.write_str("tag not allowed for this record kind"),
            Self::UnknownMorphFlag => f.write_str("unknown morphology flag"),
            Self::DuplicateMorphFlag => f.write_str("duplicate morphology flag"),
            Self::OrphanReference(key) => write!(f, "reference to unknown key {key}"),
            Self::LocalShadowsUniversal(key) => {
                write!(f, "local concept reuses the universal key {key}")
            }
            Self::BaseCycle(key) => write!(f, "cycle in the base chain of {key}"),
            Self::SupersededKey { key, successor } => {
                write!(f, "key {key} is deprecated: use {successor}")
            }
            Self::VersionMismatch { snapshot, overlay } => write!(
                f,
                "overlay built for snapshot version {overlay}, loaded snapshot is version {snapshot}"
            ),
            Self::InvalidNumeral => f.write_str("invalid #numeral declaration"),
        }
    }
}

/// Positioned DTI diagnostic (`line` is 1-based; `0` = whole file).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DtiDiagnostic {
    pub source: DtiSource,
    pub line: u32,
    pub kind: DtiErrorKind,
}

impl fmt::Display for DtiDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let source = match self.source {
            DtiSource::Universal => "universal",
            DtiSource::Local => "local",
        };
        write!(f, "[DTI {source}:{}] {}", self.line, self.kind)
    }
}

impl core::error::Error for DtiDiagnostic {}

pub(super) struct Sink {
    source: DtiSource,
    diagnostics: Vec<DtiDiagnostic>,
}

impl Sink {
    pub(super) fn report(&mut self, line: u32, kind: DtiErrorKind) {
        self.diagnostics.push(DtiDiagnostic {
            source: self.source,
            line,
            kind,
        });
    }
}

// ═══════════════════════════ Lexical layer ═══════════════════════════

/// `true` for the tag fields whose value may be quoted (`display:`, `attr:`).
fn quotable(field_start: &str) -> bool {
    field_start.starts_with("display:") || field_start.starts_with("attr:")
}

/// Splits a record line into fields. `"…"` quoting is honoured only inside the
/// `display:` and `attr:` fields; every other field ends at the next space.
fn fields(line: &str) -> Result<Vec<&str>, DtiErrorKind> {
    let mut out = Vec::new();
    let mut start = 0_usize;
    let mut quoted = false;
    let mut quoting = line.get(start..).is_some_and(quotable);
    for (index, ch) in line.char_indices() {
        match ch {
            '"' if quoting => quoted = !quoted,
            ' ' if !quoted => {
                let field = line.get(start..index).ok_or(DtiErrorKind::EmptyField)?;
                if field.is_empty() {
                    return Err(DtiErrorKind::EmptyField);
                }
                out.push(field);
                start = index.saturating_add(1);
                quoting = line.get(start..).is_some_and(quotable);
            }
            _ => {}
        }
    }
    if quoted {
        return Err(DtiErrorKind::UnterminatedQuote);
    }
    let last = line.get(start..).ok_or(DtiErrorKind::EmptyField)?;
    if last.is_empty() {
        return Err(DtiErrorKind::EmptyField);
    }
    out.push(last);
    Ok(out)
}

/// Same character policy as glyphs (D9): no control or format/bidi characters.
fn is_forbidden_text_character(ch: char) -> bool {
    crate::glyph::is_invisible_character(ch)
}

fn unquote(value: &str) -> Result<&str, DtiErrorKind> {
    match value.strip_prefix('"') {
        Some(inner) => inner
            .strip_suffix('"')
            .ok_or(DtiErrorKind::UnterminatedQuote),
        None => Ok(value),
    }
}

fn parse_key(text: &str) -> Result<DtiKey, DtiErrorKind> {
    DtiKey::parse(text).map_err(DtiErrorKind::InvalidKey)
}

pub(super) struct FormValidator;

impl FormValidator {
    /// A lexical form (primary, `form:`, `syn:`, label) is any non-empty
    /// sequence of visible characters without spaces (v4.1), `§`, `~` and `~~`
    /// included (a form containing `§` is looked up whole before any segment
    /// resolution; `~` / `~~` are used as glyphs or labels in slots annotated
    /// as such). Only invisible characters are refused (D9). Human multiword
    /// forms use `display:`.
    fn check(&mut self, form: &str) -> Result<(), DtiErrorKind> {
        if form.is_empty() || form.contains(' ') {
            return Err(DtiErrorKind::InvalidForm);
        }
        crate::glyph::check_glyph_text(form).map_err(|_| DtiErrorKind::InvalidForm)
    }
}

fn check_display(display: &str) -> Result<(), DtiErrorKind> {
    let well_spaced = !display.is_empty()
        && !display.starts_with(' ')
        && !display.ends_with(' ')
        && !display.contains("  ");
    if well_spaced
        && !display
            .chars()
            .any(|c| c == '"' || is_forbidden_text_character(c))
    {
        Ok(())
    } else {
        Err(DtiErrorKind::InvalidDisplay)
    }
}

fn parse_morph(value: &str) -> Result<MorphFlags, DtiErrorKind> {
    let mut flags = MorphFlags::default();
    for flag in value.split(',') {
        let slot = match flag {
            "+AGGL" => &mut flags.aggl,
            "+CONTR" => &mut flags.contr,
            "+UNION" => &mut flags.union,
            "+AGGL_NUM" => &mut flags.aggl_num,
            _ => return Err(DtiErrorKind::UnknownMorphFlag),
        };
        if *slot {
            return Err(DtiErrorKind::DuplicateMorphFlag);
        }
        *slot = true;
    }
    Ok(flags)
}

fn parse_attr(value: &str) -> Result<(Box<str>, Box<str>), DtiErrorKind> {
    let value = unquote(value)?;
    let (name, content) = value
        .split_once('§')
        .ok_or(DtiErrorKind::InvalidAttribute)?;
    Ok((Box::from(name), Box::from(content)))
}

/// `true` when `attr:<name>§<content>`, spelled canonically (quoted, or bare
/// when it holds an inner quote and does not start with one), is one `.awdti`
/// field that reads back as the same pair: no unquoted space, balanced quotes.
/// Checked on the characters, without building the text.
fn attr_round_trips(name: &str, content: &str) -> bool {
    let starts_with_quote = name.starts_with('"');
    let bare = (name.contains('"') || content.contains('"')) && !starts_with_quote;
    let wrapper: &[char] = if bare { &[] } else { &['"'] };
    let chars = wrapper
        .iter()
        .copied()
        .chain(name.chars())
        .chain(core::iter::once('§'))
        .chain(content.chars())
        .chain(wrapper.iter().copied());
    let mut quoted = false;
    for ch in chars {
        match ch {
            '"' => quoted = !quoted,
            ' ' if !quoted => return false,
            _ => {}
        }
    }
    !quoted
}

/// Shared attribute check (B8: opaque metadata, but well formed).
pub(super) fn check_attr(name: &str, content: &str) -> Result<(), DtiErrorKind> {
    let valid = |text: &str| !text.is_empty() && !text.chars().any(is_forbidden_text_character);
    if valid(name) && valid(content) && !name.contains('§') && attr_round_trips(name, content) {
        Ok(())
    } else {
        Err(DtiErrorKind::InvalidAttribute)
    }
}

/// Shared per-record checks of the text parser and of the `.dti.bin` decoder:
/// tags allowed by the record kind, glyph-token forms, unique
/// synonyms, display and attribute forms.
pub(super) fn validate_record_fields(
    record: &RawRecord,
    forms: &mut FormValidator,
) -> Result<(), DtiErrorKind> {
    let tags = &record.tags;
    let present = [
        ("display", tags.display.is_some()),
        ("syn", !tags.synonyms.is_empty()),
        ("base", tags.base.is_some()),
        ("param", tags.param.is_some()),
        ("morph", tags.morph.is_some()),
        ("superseded_by", tags.superseded_by.is_some()),
        ("attr", !tags.attrs.is_empty()),
        ("form", tags.form.is_some()),
    ];
    if present
        .iter()
        .any(|(name, set)| *set && !record.kind.allows(name))
    {
        return Err(DtiErrorKind::TagNotAllowed);
    }
    if let Some(primary) = &record.primary {
        forms.check(primary)?;
    }
    if let Some(form) = &tags.form {
        forms.check(form)?;
    }
    for (index, synonym) in tags.synonyms.iter().enumerate() {
        forms.check(synonym)?;
        if tags.synonyms.iter().take(index).any(|s| s == synonym) {
            return Err(DtiErrorKind::DuplicateSynonym);
        }
    }
    if let Some(display) = &tags.display {
        check_display(display)?;
    }
    for (name, content) in &tags.attrs {
        check_attr(name, content)?;
    }
    Ok(())
}

/// Tags of a record (each one at most once, except `attr`).
#[derive(Default)]
pub(super) struct Tags {
    pub(super) display: Option<Box<str>>,
    pub(super) synonyms: Vec<Box<str>>,
    pub(super) base: Option<DtiKey>,
    pub(super) param: Option<DtiKey>,
    pub(super) morph: Option<MorphFlags>,
    pub(super) superseded_by: Option<DtiKey>,
    pub(super) attrs: Vec<(Box<str>, Box<str>)>,
    pub(super) form: Option<Box<str>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RecordKind {
    Rec,
    Use,
    New,
    /// `label <KEY> <form>` (local only, no tags, v4.1).
    Label,
}

impl RecordKind {
    fn allows(self, tag: &str) -> bool {
        match self {
            Self::Label => false,
            Self::Rec => matches!(
                tag,
                "display" | "syn" | "base" | "param" | "morph" | "superseded_by" | "attr"
            ),
            Self::Use => matches!(tag, "form" | "display" | "syn"),
            Self::New => matches!(tag, "base" | "display" | "syn" | "morph"),
        }
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), DtiErrorKind> {
    if slot.is_some() {
        return Err(DtiErrorKind::DuplicateTag);
    }
    *slot = Some(value);
    Ok(())
}

fn parse_tags(kind: RecordKind, raw: &[&str]) -> Result<Tags, DtiErrorKind> {
    let mut tags = Tags::default();
    let mut synonyms_seen = false;
    for field in raw {
        let (name, value) = field.split_once(':').ok_or(DtiErrorKind::UnexpectedField)?;
        if !matches!(
            name,
            "display" | "syn" | "base" | "param" | "morph" | "superseded_by" | "attr" | "form"
        ) {
            return Err(DtiErrorKind::UnknownTag);
        }
        if !kind.allows(name) {
            return Err(DtiErrorKind::TagNotAllowed);
        }
        match name {
            "display" => set_once(&mut tags.display, Box::from(unquote(value)?))?,
            "syn" => {
                if synonyms_seen {
                    return Err(DtiErrorKind::DuplicateTag);
                }
                synonyms_seen = true;
                tags.synonyms.extend(value.split(',').map(Box::from));
            }
            "base" => set_once(&mut tags.base, parse_key(value)?)?,
            "param" => set_once(&mut tags.param, parse_key(value)?)?,
            "superseded_by" => set_once(&mut tags.superseded_by, parse_key(value)?)?,
            "morph" => set_once(&mut tags.morph, parse_morph(value)?)?,
            "attr" => tags.attrs.push(parse_attr(value)?),
            // "form" (the only remaining accepted name).
            _ => set_once(&mut tags.form, Box::from(value))?,
        }
    }
    Ok(tags)
}

/// One record before cross-record validation, produced by the text parser (one
/// line) or by the `.dti.bin` decoder (one row of the columns). `line` is the
/// source line, or the 1-based record ordinal for a binary.
pub(super) struct RawRecord {
    pub(super) line: u32,
    pub(super) kind: RecordKind,
    pub(super) key: DtiKey,
    pub(super) primary: Option<Box<str>>,
    pub(super) tags: Tags,
}

fn parse_record(
    line: &str,
    number: u32,
    forms: &mut FormValidator,
) -> Result<RawRecord, DtiErrorKind> {
    let parts = fields(line)?;
    let mut iter = parts.iter();
    let kind = match iter.next().copied() {
        Some("rec") => RecordKind::Rec,
        Some("use") => RecordKind::Use,
        Some("new") => RecordKind::New,
        Some("label") => RecordKind::Label,
        _ => return Err(DtiErrorKind::UnknownRecordKind),
    };
    let key = parse_key(
        iter.next()
            .copied()
            .ok_or(DtiErrorKind::MissingField { name: "key" })?,
    )?;
    let primary = match kind {
        RecordKind::Use => None,
        RecordKind::Rec | RecordKind::New | RecordKind::Label => {
            let form = iter
                .next()
                .copied()
                .ok_or(DtiErrorKind::MissingField { name: "primary" })?;
            Some(Box::from(form))
        }
    };
    let rest: Vec<&str> = iter.copied().collect();
    let tags = parse_tags(kind, &rest)?;
    let record = RawRecord {
        line: number,
        kind,
        key,
        primary,
        tags,
    };
    validate_record_fields(&record, forms)?;
    Ok(record)
}

// ═══════════════════════════ Header ═══════════════════════════

#[derive(Default)]
pub(super) struct Header {
    pub(super) format: Option<()>,
    pub(super) role: Option<DtiSource>,
    pub(super) version: Option<u64>,
    pub(super) universal: Option<u64>,
    pub(super) language: Option<Box<str>>,
    pub(super) numeral: Option<(u32, u8, Vec<DtiKey>)>,
    pub(super) policy: Option<UnknownKeyPolicy>,
}

/// Shared check of the `#lang` value (text header and binary metadata).
pub(super) fn check_language(tag: &str) -> Result<(), DtiErrorKind> {
    if valid_language(tag) {
        Ok(())
    } else {
        Err(DtiErrorKind::InvalidHeaderValue { name: "lang" })
    }
}

fn valid_language(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 35
        && tag
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn parse_numeral(value: &str) -> Result<(u8, Vec<DtiKey>), DtiErrorKind> {
    let (base_field, digits_field) = value.split_once(' ').ok_or(DtiErrorKind::InvalidNumeral)?;
    let base: u8 = base_field
        .strip_prefix("base:")
        .and_then(|b| b.parse().ok())
        .ok_or(DtiErrorKind::InvalidNumeral)?;
    check_numeral_base(base)?;
    let digits = digits_field
        .strip_prefix("digits:")
        .ok_or(DtiErrorKind::InvalidNumeral)?
        .split(',')
        .map(parse_key)
        .collect::<Result<Vec<DtiKey>, DtiErrorKind>>()?;
    check_numeral_digits(base, &digits)?;
    Ok((base, digits))
}

/// B4 option 1: the exchange convention is the ASCII digits 0–9 (base 2–10).
pub(super) fn check_numeral_base(base: u8) -> Result<(), DtiErrorKind> {
    if (2..=10).contains(&base) {
        Ok(())
    } else {
        Err(DtiErrorKind::InvalidNumeral)
    }
}

/// One digit concept per digit of the base.
pub(super) fn check_numeral_digits(base: u8, digits: &[DtiKey]) -> Result<(), DtiErrorKind> {
    if digits.len() == usize::from(base) {
        Ok(())
    } else {
        Err(DtiErrorKind::InvalidNumeral)
    }
}

fn apply_header(header: &mut Header, line: &str, number: u32) -> Result<(), DtiErrorKind> {
    let directive = line.strip_prefix('#').unwrap_or(line);
    let (name, value) = directive
        .split_once(' ')
        .ok_or(DtiErrorKind::UnknownHeader)?;
    match name {
        "awdti" => {
            if value != FORMAT_VERSION {
                return Err(DtiErrorKind::UnsupportedFormatVersion);
            }
            set_once_header(&mut header.format, (), "awdti")
        }
        "role" => {
            let role = match value {
                "universal" => DtiSource::Universal,
                "local" => DtiSource::Local,
                _ => return Err(DtiErrorKind::InvalidHeaderValue { name: "role" }),
            };
            set_once_header(&mut header.role, role, "role")
        }
        "version" => {
            let version = value
                .parse()
                .map_err(|_| DtiErrorKind::InvalidHeaderValue { name: "version" })?;
            set_once_header(&mut header.version, version, "version")
        }
        "universal" => {
            let version = value
                .parse()
                .map_err(|_| DtiErrorKind::InvalidHeaderValue { name: "universal" })?;
            set_once_header(&mut header.universal, version, "universal")
        }
        "lang" => {
            check_language(value)?;
            set_once_header(&mut header.language, Box::from(value), "lang")
        }
        "numeral" => {
            let (base, digits) = parse_numeral(value)?;
            set_once_header(&mut header.numeral, (number, base, digits), "numeral")
        }
        "policy" => {
            let policy = match value {
                "degrade" => UnknownKeyPolicy::DegradeToBase,
                "unknown" => UnknownKeyPolicy::MarkUnknown,
                "reject" => UnknownKeyPolicy::Reject,
                _ => return Err(DtiErrorKind::InvalidHeaderValue { name: "policy" }),
            };
            set_once_header(&mut header.policy, policy, "policy")
        }
        _ => Err(DtiErrorKind::UnknownHeader),
    }
}

fn set_once_header<T>(
    slot: &mut Option<T>,
    value: T,
    name: &'static str,
) -> Result<(), DtiErrorKind> {
    if slot.is_some() {
        return Err(DtiErrorKind::DuplicateHeader { name });
    }
    *slot = Some(value);
    Ok(())
}

/// Header plus records, before cross-record validation (text or binary).
pub(super) struct ParsedFile {
    pub(super) header: Header,
    pub(super) records: Vec<RawRecord>,
}

fn parse_file(src: &str, sink: &mut Sink, forms: &mut FormValidator) -> ParsedFile {
    let mut header = Header::default();
    let mut records = Vec::new();
    if src.len() > MAX_SOURCE_BYTES {
        sink.report(0, DtiErrorKind::SourceTooLarge);
        return ParsedFile { header, records };
    }
    for (index, raw_line) in src.split('\n').enumerate() {
        let Some(number) = index.checked_add(1).and_then(|n| u32::try_from(n).ok()) else {
            sink.report(0, DtiErrorKind::TooManyLines);
            break;
        };
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            if !records.is_empty() {
                sink.report(number, DtiErrorKind::HeaderAfterRecords);
                continue;
            }
            if let Err(kind) = apply_header(&mut header, line, number) {
                sink.report(number, kind);
            }
            continue;
        }
        match parse_record(line, number, forms) {
            Ok(record) => records.push(record),
            Err(kind) => sink.report(number, kind),
        }
    }
    ParsedFile { header, records }
}

fn require<T>(sink: &mut Sink, value: Option<T>, name: &'static str) -> Option<T> {
    if value.is_none() {
        sink.report(0, DtiErrorKind::MissingHeader { name });
    }
    value
}

/// Header fields that belong to the other role are rejected.
fn reject_foreign_headers(sink: &mut Sink, header: &Header) {
    let foreign = match sink.source {
        DtiSource::Universal => header.universal.is_some() || header.policy.is_some(),
        DtiSource::Local => header.version.is_some() || header.numeral.is_some(),
    };
    if foreign {
        sink.report(0, DtiErrorKind::WrongRole);
    }
}

// ═══════════════════════════ Cross validation ═══════════════════════════

/// Detects cycles in `base:` chains. Iterative, each key visited a bounded number
/// of times thanks to the `acyclic` memo.
fn check_base_cycles(
    sink: &mut Sink,
    starts: &[(u32, DtiKey)],
    base_of: &dyn Fn(&DtiKey) -> Option<DtiKey>,
    limit: usize,
) {
    let mut acyclic: BTreeSet<DtiKey> = BTreeSet::new();
    for (line, start) in starts {
        let mut path: Vec<DtiKey> = Vec::new();
        let mut current = Some(*start);
        let mut cyclic = false;
        while let Some(key) = current {
            if acyclic.contains(&key) {
                break;
            }
            if path.contains(&key) || path.len() > limit {
                cyclic = true;
                break;
            }
            path.push(key);
            current = base_of(&key);
        }
        if cyclic {
            sink.report(*line, DtiErrorKind::BaseCycle(*start));
        } else {
            acyclic.extend(path);
        }
    }
}

pub(super) fn build_universal(file: ParsedFile, sink: &mut Sink) -> Option<UniversalSnapshot> {
    let ParsedFile { header, records } = file;
    reject_foreign_headers(sink, &header);
    let format = require(sink, header.format, "awdti");
    let role = require(sink, header.role, "role");
    if role.is_some_and(|r| r != DtiSource::Universal) {
        sink.report(0, DtiErrorKind::WrongRole);
    }
    let version = require(sink, header.version, "version");
    let language = require(sink, header.language, "lang");

    let mut concepts: BTreeMap<DtiKey, ConceptRecord> = BTreeMap::new();
    let mut lexicon = Lexicon::default();
    let mut lines: Vec<(u32, DtiKey)> = Vec::new();
    for record in records {
        let RawRecord {
            line,
            kind,
            key,
            primary,
            tags,
        } = record;
        if kind != RecordKind::Rec {
            sink.report(line, DtiErrorKind::UnknownRecordKind);
            continue;
        }
        if concepts.contains_key(&key) {
            sink.report(line, DtiErrorKind::DuplicateKey(key));
            continue;
        }
        let Some(primary) = primary else {
            sink.report(line, DtiErrorKind::MissingField { name: "primary" });
            continue;
        };
        let entry = LexicalEntry {
            key,
            primary,
            display: tags.display,
            synonyms: tags.synonyms.into_boxed_slice(),
        };
        if !lexicon.insert(entry) {
            sink.report(line, DtiErrorKind::DuplicatePrimary);
            continue;
        }
        concepts.insert(
            key,
            ConceptRecord {
                key,
                base: tags.base,
                param: tags.param,
                morph: tags.morph.unwrap_or_default(),
                superseded_by: tags.superseded_by,
                attrs: tags.attrs.into_boxed_slice(),
            },
        );
        lines.push((line, key));
    }

    for (line, key) in &lines {
        if let Some(concept) = concepts.get(key) {
            for reference in [concept.base, concept.param, concept.superseded_by]
                .into_iter()
                .flatten()
            {
                if !concepts.contains_key(&reference) {
                    sink.report(*line, DtiErrorKind::OrphanReference(reference));
                }
            }
        }
    }
    let base_of = |key: &DtiKey| concepts.get(key).and_then(|c| c.base);
    check_base_cycles(sink, &lines, &base_of, concepts.len());

    let numeral = match header.numeral {
        Some((line, base, digits)) => {
            for digit in &digits {
                if !concepts.contains_key(digit) {
                    sink.report(line, DtiErrorKind::OrphanReference(*digit));
                }
            }
            Some(NumeralSystem {
                base,
                digits: digits.into_boxed_slice(),
            })
        }
        None => None,
    };

    format?;
    Some(UniversalSnapshot {
        version: version?,
        language: language?,
        numeral,
        concepts,
        lexicon,
    })
}

pub(super) fn build_overlay(
    file: ParsedFile,
    universal: &UniversalSnapshot,
    sink: &mut Sink,
) -> Option<LocalOverlay> {
    let ParsedFile { header, records } = file;
    reject_foreign_headers(sink, &header);
    let format = require(sink, header.format, "awdti");
    let role = require(sink, header.role, "role");
    if role.is_some_and(|r| r != DtiSource::Local) {
        sink.report(0, DtiErrorKind::WrongRole);
    }
    let universal_version = require(sink, header.universal, "universal");
    let language = require(sink, header.language, "lang");
    if let Some(overlay) = universal_version {
        if overlay != universal.version {
            sink.report(
                0,
                DtiErrorKind::VersionMismatch {
                    snapshot: universal.version,
                    overlay,
                },
            );
        }
    }

    let mut enabled: BTreeSet<DtiKey> = BTreeSet::new();
    let mut local: BTreeMap<DtiKey, ConceptRecord> = BTreeMap::new();
    let mut lexicon = Lexicon::default();
    let mut labels = LabelLexicon::default();
    let mut census_warnings: Vec<CensusWarning> = Vec::new();
    let mut local_lines: Vec<(u32, DtiKey)> = Vec::new();
    // v4.1: warns when a local form is the exchange-language primary form of
    // another universal glyph.
    let mut census = |line: u32, kind: CensusKind, form: &str, own: Option<DtiKey>| {
        if let Some(universal_key) = universal.lexicon.primary(form) {
            if own != Some(universal_key) {
                census_warnings.push(CensusWarning {
                    line,
                    kind,
                    form: Box::from(form),
                    universal: universal_key,
                });
            }
        }
    };

    for record in records {
        let RawRecord {
            line,
            kind,
            key,
            primary,
            tags,
        } = record;
        match kind {
            RecordKind::Rec => sink.report(line, DtiErrorKind::UnknownRecordKind),
            RecordKind::Use => {
                let Some(concept) = universal.concepts.get(&key) else {
                    sink.report(line, DtiErrorKind::OrphanReference(key));
                    continue;
                };
                if let Some(successor) = concept.superseded_by {
                    sink.report(line, DtiErrorKind::SupersededKey { key, successor });
                    continue;
                }
                if !enabled.insert(key) {
                    sink.report(line, DtiErrorKind::DuplicateKey(key));
                    continue;
                }
                if let Some(form) = tags.form {
                    census(line, CensusKind::Glyph, &form, Some(key));
                    let entry = LexicalEntry {
                        key,
                        primary: form,
                        display: tags.display,
                        synonyms: tags.synonyms.into_boxed_slice(),
                    };
                    if !lexicon.insert(entry) {
                        sink.report(line, DtiErrorKind::DuplicatePrimary);
                    }
                } else if tags.display.is_some() || !tags.synonyms.is_empty() {
                    // Display and synonyms need a local primary form to attach to.
                    sink.report(line, DtiErrorKind::MissingField { name: "form" });
                }
            }
            RecordKind::New => {
                if universal.concepts.contains_key(&key) {
                    sink.report(line, DtiErrorKind::LocalShadowsUniversal(key));
                    continue;
                }
                if local.contains_key(&key) || enabled.contains(&key) {
                    sink.report(line, DtiErrorKind::DuplicateKey(key));
                    continue;
                }
                if labels.contains_key(&key) {
                    sink.report(line, DtiErrorKind::DuplicateKey(key));
                    continue;
                }
                let Some(primary) = primary else {
                    sink.report(line, DtiErrorKind::MissingField { name: "primary" });
                    continue;
                };
                census(line, CensusKind::Glyph, &primary, None);
                let entry = LexicalEntry {
                    key,
                    primary,
                    display: tags.display,
                    synonyms: tags.synonyms.into_boxed_slice(),
                };
                if !lexicon.insert(entry) {
                    sink.report(line, DtiErrorKind::DuplicatePrimary);
                    continue;
                }
                local.insert(
                    key,
                    ConceptRecord {
                        key,
                        base: tags.base,
                        param: None,
                        morph: tags.morph.unwrap_or_default(),
                        superseded_by: None,
                        attrs: Box::new([]),
                    },
                );
                local_lines.push((line, key));
            }
            RecordKind::Label => {
                if universal.concepts.contains_key(&key) {
                    sink.report(line, DtiErrorKind::LocalShadowsUniversal(key));
                    continue;
                }
                if local.contains_key(&key) || enabled.contains(&key) {
                    sink.report(line, DtiErrorKind::DuplicateKey(key));
                    continue;
                }
                let Some(form) = primary else {
                    sink.report(line, DtiErrorKind::MissingField { name: "primary" });
                    continue;
                };
                census(line, CensusKind::Label, &form, None);
                if labels.contains_key(&key) {
                    sink.report(line, DtiErrorKind::DuplicateKey(key));
                } else if !labels.insert(key, form) {
                    sink.report(line, DtiErrorKind::DuplicateLabel);
                }
            }
        }
    }

    for (line, key) in &local_lines {
        if let Some(base) = local.get(key).and_then(|c| c.base) {
            if !local.contains_key(&base) && !universal.concepts.contains_key(&base) {
                sink.report(*line, DtiErrorKind::OrphanReference(base));
            }
        }
    }
    let base_of = |key: &DtiKey| {
        local
            .get(key)
            .or_else(|| universal.concepts.get(key))
            .and_then(|c| c.base)
    };
    let limit = local.len().saturating_add(universal.concepts.len());
    check_base_cycles(sink, &local_lines, &base_of, limit);

    format?;
    Some(LocalOverlay {
        universal_version: universal_version?,
        language: language?,
        policy: header.policy.unwrap_or(UnknownKeyPolicy::DegradeToBase),
        enabled,
        local,
        lexicon,
        labels,
        census_warnings,
    })
}

// ═══════════════════════════ Entry points ═══════════════════════════

pub(super) const fn form_validator() -> FormValidator {
    FormValidator
}

pub(super) const fn new_sink(source: DtiSource) -> Sink {
    Sink {
        source,
        diagnostics: Vec::new(),
    }
}

/// Parses and validates a universal source. Diagnostics are left in `sink`.
fn compile_universal(
    src: &str,
    sink: &mut Sink,
    forms: &mut FormValidator,
) -> Option<UniversalSnapshot> {
    let file = parse_file(src, sink, forms);
    build_universal(file, sink)
}

/// Parses a local source and validates it against `universal`. Line-level
/// diagnostics are reported even when no snapshot is available.
fn compile_local(
    src: &str,
    universal: Option<&UniversalSnapshot>,
    sink: &mut Sink,
    forms: &mut FormValidator,
) -> Option<LocalOverlay> {
    let file = parse_file(src, sink, forms);
    universal.and_then(|u| build_overlay(file, u, sink))
}

/// Accepts a built value only when no diagnostic was reported; otherwise
/// returns the diagnostics sorted by line.
pub(super) fn finish<T>(value: Option<T>, sink: Sink) -> Result<T, Vec<DtiDiagnostic>> {
    let mut diagnostics = sink.diagnostics;
    match value {
        Some(value) if diagnostics.is_empty() => Ok(value),
        _ => {
            diagnostics.sort_by_key(|d| d.line);
            Err(diagnostics)
        }
    }
}

impl UniversalSnapshot {
    /// Parses and validates a universal `.awdti` source (`#role universal`).
    /// On failure every diagnostic is returned, sorted by line.
    pub fn parse(src: &str) -> Result<Self, Vec<DtiDiagnostic>> {
        let mut sink = new_sink(DtiSource::Universal);
        let universal = compile_universal(src, &mut sink, &mut form_validator());
        finish(universal, sink)
    }
}

impl LocalOverlay {
    /// Parses a local `.awdti` source (`#role local`) and validates it against
    /// `universal`. On failure every diagnostic is returned, sorted by line.
    pub fn parse(src: &str, universal: &UniversalSnapshot) -> Result<Self, Vec<DtiDiagnostic>> {
        let mut sink = new_sink(DtiSource::Local);
        let overlay = compile_local(src, Some(universal), &mut sink, &mut form_validator());
        finish(overlay, sink)
    }
}

impl Dti {
    /// Joins two independently loaded authorities. The overlay is cross-checked
    /// against the snapshot (version, enabled keys, local concepts and their
    /// `base:` chains), because it may have been validated against another
    /// snapshot. Diagnostics refer to the local file with line `0`.
    pub fn from_parts(
        universal: UniversalSnapshot,
        overlay: LocalOverlay,
    ) -> Result<Self, Vec<DtiDiagnostic>> {
        let mut sink = new_sink(DtiSource::Local);
        if overlay.universal_version != universal.version {
            sink.report(
                0,
                DtiErrorKind::VersionMismatch {
                    snapshot: universal.version,
                    overlay: overlay.universal_version,
                },
            );
        }
        // `BTreeSet` iteration is already in key order.
        for key in overlay.enabled.iter().copied() {
            match universal.concepts.get(&key) {
                None => sink.report(0, DtiErrorKind::OrphanReference(key)),
                Some(concept) => {
                    if let Some(successor) = concept.superseded_by {
                        sink.report(0, DtiErrorKind::SupersededKey { key, successor });
                    }
                }
            }
        }
        // `BTreeMap` keys are already in key order.
        let local: Vec<(u32, DtiKey)> = overlay.local.keys().map(|key| (0, *key)).collect();
        for (_, key) in &local {
            if universal.concepts.contains_key(key) {
                sink.report(0, DtiErrorKind::LocalShadowsUniversal(*key));
            }
            if let Some(base) = overlay.local.get(key).and_then(|c| c.base) {
                if !overlay.local.contains_key(&base) && !universal.concepts.contains_key(&base) {
                    sink.report(0, DtiErrorKind::OrphanReference(base));
                }
            }
        }
        for (key, _) in overlay.labels.iter() {
            if universal.concepts.contains_key(&key) {
                sink.report(0, DtiErrorKind::LocalShadowsUniversal(key));
            }
        }
        let base_of = |key: &DtiKey| {
            overlay
                .local
                .get(key)
                .or_else(|| universal.concepts.get(key))
                .and_then(|c| c.base)
        };
        let limit = overlay.local.len().saturating_add(universal.concepts.len());
        check_base_cycles(&mut sink, &local, &base_of, limit);
        if sink.diagnostics.is_empty() {
            Ok(Self { universal, overlay })
        } else {
            Err(sink.diagnostics)
        }
    }

    /// Loads and validates the two authorities. On failure every diagnostic of
    /// both files is returned, sorted by file and line.
    ///
    /// Same pipeline as [`UniversalSnapshot::parse`] and [`LocalOverlay::parse`];
    /// in addition the local file is still parsed line by line when the
    /// snapshot is invalid, so that its diagnostics are reported too.
    pub fn load(universal_src: &str, overlay_src: &str) -> Result<Self, Vec<DtiDiagnostic>> {
        let mut forms = form_validator();
        let mut universal_sink = new_sink(DtiSource::Universal);
        let universal = compile_universal(universal_src, &mut universal_sink, &mut forms);

        let mut local_sink = new_sink(DtiSource::Local);
        let overlay = compile_local(overlay_src, universal.as_ref(), &mut local_sink, &mut forms);

        let mut diagnostics = universal_sink.diagnostics;
        diagnostics.extend(local_sink.diagnostics);
        match (universal, overlay) {
            (Some(universal), Some(overlay)) if diagnostics.is_empty() => {
                Ok(Self { universal, overlay })
            }
            _ => {
                diagnostics.sort_by_key(|d| (d.source == DtiSource::Local, d.line));
                Err(diagnostics)
            }
        }
    }
}
