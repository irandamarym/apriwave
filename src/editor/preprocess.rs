//! Assisted-input preprocessor (Phase D, H7, B5).
//!
//! Turns relaxed editor input into the canonical protocol text accepted by
//! [`crate::Session::ingest`]. The protocol grammar is unchanged: the output
//! uses single ASCII spaces and keeps Slot Locking `[Tile] [Glyph]`.
//!
//! Token classification mirrors the positional parser: after a period open or
//! a connector comes a signal tile (odd slot), then a glyph (even slot), then
//! case tiles / connectors / closes (odd) alternating with glyphs (even).
//! Conjunctions and case tiles are recognized only in odd slots.
//!
//! Decision E (option 1): the canonical text admits only DTI **primary forms**.
//! With a DTI, every glyph atom written as a synonym (`syn:`) or a display form
//! is replaced by the primary form of its concept, uniformly in every glyph
//! position: signal and case slots, `def` names and values, portability ids,
//! the atoms of anaphora bases (`base~`, `base§mod~`, `glyph~~`) and every `§`
//! segment of a qualified path. Multiword display forms (Normal slots only) are
//! resolved to the primary form as well. Stage 3 resolves primary forms only.

use std::fmt;

use crate::ast::{
    Comparative, Conditional, DefSignal, ExplicitNumber, PrimarySignal, Quantifier, Reflexive,
    SecondarySignal, Temporal, ANAPHORA_MARK, DEEP_SURFACE, QUALIFIER_SEPARATOR, RAW_STRING_QUOTE,
    SEMA_ANAPHORA_MARK,
};
use crate::dti::{Dti, DtiKey, FormResolution};
use crate::error::DiagnosticKind;
use crate::span::Span;
use crate::stage3::{resolve_glyph_text, MorphFlag, ResolveError};
use crate::tiles::{self, SignalParse};

/// Maximum number of tokens joined into a multiword display form.
const MAX_MULTIWORD_TOKENS: usize = 6;

const OPEN_BRACE: &str = "{";
const CLOSE_BRACE: &str = "}";

/// Assisted-input features (the `[editor]` table of `settings.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssistSettings {
    /// `{ }` blocks converted into colon runs.
    pub braces: bool,
    /// `//` line comments.
    pub comments: bool,
    /// `_` signal trains (`ta_cha_zato` → `tachazato`).
    pub signal_underscore: bool,
    /// Symbol aliases inside signal trains (`>` → `cha`, ...).
    pub symbol_aliases: bool,
    /// Multiword display forms resolved through the DTI.
    pub multiword: bool,
}

impl Default for AssistSettings {
    fn default() -> Self {
        Self {
            braces: true,
            comments: true,
            signal_underscore: true,
            symbol_aliases: true,
            multiword: true,
        }
    }
}

/// Successful preprocessing: canonical text plus non-fatal notes.
#[derive(Debug, PartialEq, Eq)]
pub struct AssistOutput {
    /// Canonical protocol text (single ASCII spaces, no comments).
    pub text: String,
    pub notes: Vec<AssistNote>,
}

/// Non-fatal information for the editor.
#[derive(Debug, PartialEq, Eq)]
pub enum AssistNote {
    /// A single glyph whose form is unknown to the DTI. The editor may offer
    /// to reclassify it as a raw-string label (`label_suggestion`, if valid).
    UnknownGlyph {
        token: String,
        span: Span,
        label_suggestion: Option<String>,
    },
    /// A multiword display form replaced by the primary form of its key.
    MultiwordGlyph {
        phrase: String,
        form: String,
        span: Span,
    },
    /// A single glyph atom written as a synonym (`syn:`) or a display form of an
    /// enabled concept, replaced by the primary form of that concept. `span` is
    /// the byte span of the atom alone (one `§` segment of the written token).
    NormalizedForm {
        written: String,
        primary: String,
        span: Span,
    },
}

/// Blocking preprocessing error, with the byte span in the input.
#[derive(Debug, PartialEq, Eq)]
pub struct AssistDiagnostic {
    pub kind: AssistDiagnosticKind,
    pub span: Span,
}

impl fmt::Display for AssistDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {}..{}", self.kind, self.span.start, self.span.end)
    }
}

impl std::error::Error for AssistDiagnostic {}

#[derive(Debug, PartialEq, Eq)]
pub enum AssistDiagnosticKind {
    /// `{` without a matching `}` before the period close or the end of input.
    UnclosedBrace,
    /// `}` without a matching `{`.
    UnexpectedCloseBrace,
    /// `{ }` without any clause.
    EmptyBraces,
    /// `{` right after a period open (there is no clause to attach children to).
    BraceAfterPeriodOpen,
    /// `{` or `}` in a slot where it cannot appear.
    MisplacedBrace,
    /// Explicit colon run inside braces.
    MixedBracesAndColons,
    /// Portability keyword inside braces, or braces after a portability block.
    BracesWithPortability,
    /// A new root clause after `}` without an explicit connector.
    MissingConnector,
    /// A case tile right after `}` (it would attach to the last child clause).
    CaseTileAfterBrace,
    /// Removed ambiguous alias (`~=`, `+`, `-`) in a signal train (H7).
    RemovedAlias { alias: String },
    /// Multiword phrase, synonym or display form shared by several concepts:
    /// the user must choose (B3).
    AmbiguousForm { phrase: String, keys: Vec<DtiKey> },
    /// Multiword phrase, synonym or display form of a concept not enabled in
    /// the local overlay (B5).
    DisabledEntry { key: DtiKey, suggestion: String },
    /// A glyph needs a morphology permission (`+UNION`, `+AGGL`, `+CONTR`,
    /// `+AGGL_NUM`) that the concept `key` lacks (B9).
    MorphViolation {
        form: String,
        key: DtiKey,
        flag: MorphFlag,
    },
}

impl fmt::Display for AssistDiagnosticKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnclosedBrace => f.write_str("unclosed brace"),
            Self::UnexpectedCloseBrace => f.write_str("closing brace without a matching '{'"),
            Self::EmptyBraces => f.write_str("empty braces"),
            Self::BraceAfterPeriodOpen => {
                f.write_str("'{' right after a period open: write the parent clause first")
            }
            Self::MisplacedBrace => f.write_str("brace in a wrong slot"),
            Self::MixedBracesAndColons => {
                f.write_str("explicit colon run inside braces: use either braces or colons")
            }
            Self::BracesWithPortability => {
                f.write_str("portability keywords cannot be used with braces")
            }
            Self::MissingConnector => {
                f.write_str("a new root clause after '}' needs an explicit conjunction")
            }
            Self::CaseTileAfterBrace => {
                f.write_str("case tile after '}': move it before the braces")
            }
            Self::RemovedAlias { alias } => {
                write!(f, "the ambiguous alias '{alias}' has been removed")
            }
            Self::AmbiguousForm { phrase, keys } => {
                write!(f, "'{phrase}' is ambiguous between {} concepts", keys.len())
            }
            Self::DisabledEntry { key, suggestion } => {
                write!(f, "concept {key} is not enabled ({suggestion})")
            }
            Self::MorphViolation { form, key, flag } => {
                write!(f, "'{form}' requires {flag} on concept {key}")
            }
        }
    }
}

/// Reasons why a token cannot become a raw-string label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelError {
    Empty,
    ContainsQuote,
    ContainsWhitespace,
    /// Control or bidi/format character (D9).
    ForbiddenCharacter {
        ch: char,
    },
}

impl fmt::Display for LabelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty label"),
            Self::ContainsQuote => f.write_str("a label cannot contain '\"'"),
            Self::ContainsWhitespace => f.write_str("a label cannot contain whitespace"),
            Self::ForbiddenCharacter { ch } => {
                write!(f, "forbidden character U+{:04X} in label", u32::from(*ch))
            }
        }
    }
}

impl std::error::Error for LabelError {}

/// D9: control characters plus format characters that alter the direction or
/// visibility of text.
fn is_forbidden_label_character(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// Wraps an unknown glyph as a raw-string label `"token"`.
pub fn reclassify_as_label(token: &str) -> Result<String, LabelError> {
    if token.is_empty() {
        return Err(LabelError::Empty);
    }
    for ch in token.chars() {
        if ch == '"' {
            return Err(LabelError::ContainsQuote);
        }
        if ch.is_whitespace() {
            return Err(LabelError::ContainsWhitespace);
        }
        if is_forbidden_label_character(ch) {
            return Err(LabelError::ForbiddenCharacter { ch });
        }
    }
    Ok(format!("\"{token}\""))
}

// ═══════════════════════════ Tokenizer ═══════════════════════════

#[derive(Debug, Clone, Copy)]
struct Token<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

impl Token<'_> {
    fn span(self) -> Span {
        span_of(self.start, self.end)
    }
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn span_of(start: usize, end: usize) -> Span {
    Span::new(offset(start), offset(end))
}

/// Splits on any whitespace and drops `//` comments. A `"` toggles the raw
/// string state inside a token (`"http://x"` is not a comment); a `/` that
/// follows `^` belongs to a period delimiter (`/^/`, `^/`, `^^/`).
fn tokenize<'a>(input: &'a str, comments: bool) -> Vec<Token<'a>> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    let mut token_start: Option<usize> = None;
    let mut in_quote = false;
    let mut previous: Option<char> = None;
    let mut in_comment = false;

    let push = |tokens: &mut Vec<Token<'a>>, start: usize, end: usize| {
        if let Some(text) = input.get(start..end) {
            tokens.push(Token { text, start, end });
        }
    };

    while let Some((index, ch)) = chars.next() {
        if in_comment {
            if ch == '\n' {
                in_comment = false;
            }
            continue;
        }
        if ch.is_whitespace() {
            if let Some(start) = token_start.take() {
                push(&mut tokens, start, index);
            }
            in_quote = false;
            previous = None;
            continue;
        }
        if ch == '"' {
            in_quote = !in_quote;
        } else if comments
            && !in_quote
            && ch == '/'
            && previous != Some('^')
            && matches!(chars.peek(), Some((_, '/')))
        {
            if let Some(start) = token_start.take() {
                push(&mut tokens, start, index);
            }
            in_comment = true;
            in_quote = false;
            previous = None;
            continue;
        }
        if token_start.is_none() {
            token_start = Some(index);
        }
        previous = Some(ch);
    }
    if let Some(start) = token_start {
        push(&mut tokens, start, input.len());
    }
    tokens
}

// ═══════════════════════════ Signal trains ═══════════════════════════

/// Class slot of a determiner in the fixed order
/// quantifier → `ref` → comparative → temporal|conditional → number (rule 1B).
type DeterminerSlot = u8;

/// Next determiner at the start of `text`: `(slot, byte length)`.
fn next_determiner(text: &str) -> Option<(DeterminerSlot, usize)> {
    if let Some(q) = Quantifier::ALL
        .into_iter()
        .find(|q| text.starts_with(q.surface()))
    {
        return Some((0, q.surface().len()));
    }
    if text.starts_with(Reflexive::SURFACE) {
        return Some((1, Reflexive::SURFACE.len()));
    }
    if let Some(c) = Comparative::ALL
        .into_iter()
        .find(|c| text.starts_with(c.surface()))
    {
        return Some((2, c.surface().len()));
    }
    if let Some(t) = Temporal::ALL
        .into_iter()
        .find(|t| text.starts_with(t.surface()))
    {
        return Some((3, t.surface().len()));
    }
    if let Some(conditional) = text.strip_prefix(Conditional::PREFIX).and_then(|rest| {
        Temporal::ALL
            .into_iter()
            .find(|t| rest.starts_with(t.surface()))
            .map(|t| {
                (
                    3,
                    Conditional::PREFIX.len().saturating_add(t.surface().len()),
                )
            })
    }) {
        return Some(conditional);
    }
    // Rule 1B: the optional number of the signal pair closes the tile.
    ExplicitNumber::ALL
        .into_iter()
        .find(|n| text.starts_with(n.surface()))
        .map(|n| (4, n.surface().len()))
}

/// Splits a run of agglutinated determiners; `None` if any part is unknown.
fn split_determiners<'a>(
    mut text: &'a str,
    out: &mut Vec<(DeterminerSlot, &'a str)>,
) -> Option<()> {
    while !text.is_empty() {
        let (slot, length) = next_determiner(text)?;
        let (surface, rest) = text.split_at_checked(length)?;
        out.push((slot, surface));
        text = rest;
    }
    Some(())
}

enum Head {
    /// Byte length of a complete head (`ta`, `quata`, `deep`).
    Complete(usize),
    /// A secondary signal alone: the primary must be the next part.
    SecondaryOnly,
    /// `def`: only the number agglutinates, parts are joined in order.
    Def,
}

fn signal_head(part: &str) -> Option<Head> {
    if part.starts_with(DEEP_SURFACE) {
        return Some(Head::Complete(DEEP_SURFACE.len()));
    }
    if part.starts_with(DefSignal::SURFACE) {
        return Some(Head::Def);
    }
    if let Some(secondary) = SecondarySignal::ALL
        .into_iter()
        .find(|s| part.starts_with(s.surface()))
    {
        let rest = part.get(secondary.surface().len()..).unwrap_or_default();
        if rest.is_empty() {
            return Some(Head::SecondaryOnly);
        }
        return PrimarySignal::ALL
            .into_iter()
            .find(|p| rest.starts_with(p.surface()))
            .map(|p| Head::Complete(secondary.surface().len().saturating_add(p.surface().len())));
    }
    PrimarySignal::ALL
        .into_iter()
        .find(|p| part.starts_with(p.surface()))
        .map(|p| Head::Complete(p.surface().len()))
}

fn is_primary(part: &str) -> bool {
    PrimarySignal::ALL.into_iter().any(|p| p.surface() == part)
}

/// Signal-shaped token: a valid signal tile or one whose only faults are
/// determiner order/duplication (reported by the parser), `def` with
/// determiners or a secondary without primary.
fn is_signal_like(text: &str) -> bool {
    matches!(
        tiles::parse_signal(text),
        Ok(_)
            | Err(DiagnosticKind::DuplicateDeterminer { .. }
                | DiagnosticKind::DeterminerOutOfOrder { .. }
                | DiagnosticKind::DeterminerOnDef
                | DiagnosticKind::MissingPrimaryAfterSecondary)
    )
}

fn is_def_signal(text: &str) -> bool {
    matches!(tiles::parse_signal(text), Ok(SignalParse::Def(_)))
}

// ═══════════════════════════ Preprocessor ═══════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalSite {
    /// Right after a period open (or at the start of a fragment).
    Head,
    /// After a colon run or a conjunction.
    Connector,
    /// Right after `{`: the colons are emitted before the clause.
    BraceStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlyphRole {
    /// Signal glyph slot, case slots and `def` values: multiword display forms,
    /// synonym normalization, morphology check and unknown-glyph notes.
    Normal,
    /// Variable name of a `def` block: a session symbol. Synonyms are
    /// normalized (so that later references stay consistent, R13), but unknown
    /// names are legitimate and produce no note.
    DefName,
    /// Operand of a portability block: an identifier, normalized like a `def`
    /// name.
    PortabilityOperand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Signal(SignalSite),
    Glyph(GlyphRole),
    AfterPair { closed_brace: bool },
}

struct Frame {
    /// Number of colons prepended to every clause start inside this frame.
    child_depth: usize,
    span: Span,
}

struct Assist<'a, 's> {
    settings: &'s AssistSettings,
    dti: Option<&'s Dti>,
    tokens: Vec<Token<'a>>,
    out: Vec<String>,
    diagnostics: Vec<AssistDiagnostic>,
    notes: Vec<AssistNote>,
    frames: Vec<Frame>,
    state: State,
    /// Depth (number of colons) of the clause being written.
    clause_depth: usize,
    /// Depth assigned by the last connector to the next clause.
    next_depth: usize,
    /// The clause being written is a portability block.
    last_portability: bool,
}

/// Preprocesses assisted input into canonical protocol text.
///
/// `Err` carries **all** the blocking diagnostics, in input order.
pub fn preprocess_assisted_input(
    input: &str,
    settings: &AssistSettings,
    dti: Option<&Dti>,
) -> Result<AssistOutput, Vec<AssistDiagnostic>> {
    let mut assist = Assist {
        settings,
        dti,
        tokens: tokenize(input, settings.comments),
        out: Vec::new(),
        diagnostics: Vec::new(),
        notes: Vec::new(),
        frames: Vec::new(),
        state: State::Signal(SignalSite::Head),
        clause_depth: 0,
        next_depth: 0,
        last_portability: false,
    };
    assist.run();
    if assist.diagnostics.is_empty() {
        Ok(AssistOutput {
            text: assist.out.join(" "),
            notes: assist.notes,
        })
    } else {
        assist
            .diagnostics
            .sort_by_key(|d| (d.span.start, d.span.end));
        Err(assist.diagnostics)
    }
}

impl Assist<'_, '_> {
    fn run(&mut self) {
        let mut index: usize = 0;
        while let Some(token) = self.tokens.get(index).copied() {
            let extra = self.step(token, index);
            index = index.saturating_add(1).saturating_add(extra);
        }
        self.unwind_braces();
    }

    fn report(&mut self, kind: AssistDiagnosticKind, span: Span) {
        self.diagnostics.push(AssistDiagnostic { kind, span });
    }

    fn emit(&mut self, text: &str) {
        self.out.push(text.to_owned());
    }

    fn emit_colons(&mut self, count: usize) {
        if count > 0 {
            self.out.push(":".repeat(count));
        }
    }

    fn child_depth(&self) -> Option<usize> {
        self.frames.last().map(|f| f.child_depth)
    }

    /// Processes one token; returns the number of **extra** tokens consumed.
    fn step(&mut self, token: Token<'_>, index: usize) -> usize {
        let text = token.text;
        if self.settings.braces {
            if text == OPEN_BRACE {
                self.open_brace(token.span());
                return 0;
            }
            if text == CLOSE_BRACE {
                self.close_brace(token.span());
                return 0;
            }
        }
        if tiles::period_open(text).is_some() || tiles::period_close(text).is_some() {
            self.unwind_braces();
            self.emit(text);
            self.state = State::Signal(SignalSite::Head);
            self.next_depth = 0;
            self.clause_depth = 0;
            self.last_portability = false;
            return 0;
        }
        if let Some(count) = tiles::colon_run(text) {
            if !self.frames.is_empty() {
                self.report(AssistDiagnosticKind::MixedBracesAndColons, token.span());
            }
            self.emit(text);
            self.next_depth = count;
            self.state = State::Signal(SignalSite::Connector);
            return 0;
        }
        match self.state {
            State::Signal(site) => {
                self.signal_slot(token, site);
                0
            }
            State::Glyph(role) => self.glyph_slot(index, role),
            State::AfterPair { closed_brace } => {
                self.odd_slot(token, closed_brace);
                0
            }
        }
    }

    // ───────────────────────── Braces ─────────────────────────

    fn open_brace(&mut self, span: Span) {
        match self.state {
            State::AfterPair {
                closed_brace: false,
            } => {
                if self.last_portability {
                    self.report(AssistDiagnosticKind::BracesWithPortability, span);
                }
            }
            State::Signal(SignalSite::Head) => {
                self.report(AssistDiagnosticKind::BraceAfterPeriodOpen, span);
            }
            State::Signal(_) | State::Glyph(_) | State::AfterPair { closed_brace: true } => {
                self.report(AssistDiagnosticKind::MisplacedBrace, span);
            }
        }
        // The frame is pushed even after an error, so that the matching `}`
        // does not produce a second diagnostic.
        self.frames.push(Frame {
            child_depth: self.clause_depth.saturating_add(1),
            span,
        });
        self.state = State::Signal(SignalSite::BraceStart);
    }

    fn close_brace(&mut self, span: Span) {
        let Some(frame) = self.frames.pop() else {
            self.report(AssistDiagnosticKind::UnexpectedCloseBrace, span);
            return;
        };
        match self.state {
            State::AfterPair { .. } => {}
            State::Signal(SignalSite::BraceStart) => {
                self.report(
                    AssistDiagnosticKind::EmptyBraces,
                    Span::new(frame.span.start, span.end),
                );
            }
            State::Signal(_) | State::Glyph(_) => {
                self.report(AssistDiagnosticKind::MisplacedBrace, span);
            }
        }
        self.clause_depth = frame.child_depth.saturating_sub(1);
        self.last_portability = false;
        self.state = State::AfterPair { closed_brace: true };
    }

    /// Reports every brace still open (period close, period open or end of input).
    fn unwind_braces(&mut self) {
        let frames = std::mem::take(&mut self.frames);
        for frame in frames {
            self.report(AssistDiagnosticKind::UnclosedBrace, frame.span);
        }
    }

    // ───────────────────────── Odd slots ─────────────────────────

    fn signal_slot(&mut self, token: Token<'_>, site: SignalSite) {
        let text = self.normalize_signal(token);
        let depth = match site {
            SignalSite::Head => 0,
            SignalSite::Connector => self.next_depth,
            SignalSite::BraceStart => {
                let depth = self.child_depth().unwrap_or_default();
                self.emit_colons(depth);
                depth
            }
        };
        self.start_clause(token, text, depth);
    }

    fn start_clause(&mut self, token: Token<'_>, text: String, depth: usize) {
        let portability = tiles::portability_keyword(&text).is_some();
        if portability && !self.frames.is_empty() {
            self.report(AssistDiagnosticKind::BracesWithPortability, token.span());
        }
        let role = if portability {
            GlyphRole::PortabilityOperand
        } else if is_def_signal(&text) {
            GlyphRole::DefName
        } else {
            GlyphRole::Normal
        };
        self.out.push(text);
        self.clause_depth = depth;
        self.last_portability = portability;
        self.state = State::Glyph(role);
    }

    /// Odd slot after a complete pair: case tile, conjunction or (inside
    /// braces) the start of a sibling clause.
    fn odd_slot(&mut self, token: Token<'_>, closed_brace: bool) {
        let text = token.text;
        if tiles::conjunction(text).is_some() {
            self.emit(text);
            match self.child_depth() {
                Some(depth) => {
                    self.emit_colons(depth);
                    self.next_depth = depth;
                }
                None => self.next_depth = 0,
            }
            self.state = State::Signal(SignalSite::Connector);
            return;
        }

        let normalized = self.normalize_signal(token);
        if is_signal_like(&normalized) || tiles::portability_keyword(&normalized).is_some() {
            match self.child_depth() {
                Some(depth) => {
                    self.emit_colons(depth);
                    self.start_clause(token, normalized, depth);
                }
                None => {
                    if closed_brace {
                        self.report(AssistDiagnosticKind::MissingConnector, token.span());
                    }
                    // Outside braces the parser reports the missing connector.
                    let depth = self.clause_depth;
                    self.start_clause(token, normalized, depth);
                }
            }
            return;
        }

        if closed_brace {
            self.report(AssistDiagnosticKind::CaseTileAfterBrace, token.span());
        }
        self.emit(text);
        self.state = State::Glyph(GlyphRole::Normal);
    }

    /// Joins a `_` signal train and reorders its determiners. Returns the
    /// token unchanged if it is not a well-formed train.
    fn normalize_signal(&mut self, token: Token<'_>) -> String {
        match self.join_train(token) {
            Some(joined) => joined,
            None => token.text.to_owned(),
        }
    }

    fn join_train(&mut self, token: Token<'_>) -> Option<String> {
        if !self.settings.signal_underscore || !token.text.contains('_') {
            return None;
        }
        let mut parts: Vec<(&str, usize)> = Vec::new();
        let mut cursor = token.start;
        for part in token.text.split('_') {
            if part.is_empty() {
                return None;
            }
            parts.push((part, cursor));
            cursor = cursor.saturating_add(part.len()).saturating_add(1);
        }
        let (first, _) = *parts.first()?;
        let head = signal_head(first)?;

        let mut rest: Vec<&str> = Vec::new();
        for (part, start) in parts.iter().skip(1).copied() {
            rest.push(self.map_alias(part, start));
        }

        let (head_text, first_tail, determiner_parts): (String, &str, &[&str]) = match head {
            Head::Def => {
                let mut joined = String::from(first);
                joined.extend(rest);
                return Some(joined);
            }
            Head::SecondaryOnly => {
                let (primary, others) = rest.split_first()?;
                if !is_primary(primary) {
                    return None;
                }
                (format!("{first}{primary}"), "", others)
            }
            Head::Complete(length) => {
                let (head, tail) = first.split_at_checked(length)?;
                (String::from(head), tail, rest.as_slice())
            }
        };

        let mut determiners: Vec<(DeterminerSlot, &str)> = Vec::new();
        split_determiners(first_tail, &mut determiners)?;
        for part in determiner_parts {
            split_determiners(part, &mut determiners)?;
        }
        // Stable sort: duplicates stay (the parser reports ERR_ORDER).
        determiners.sort_by_key(|(slot, _)| *slot);
        let mut joined = head_text;
        for (_, surface) in determiners {
            joined.push_str(surface);
        }
        Some(joined)
    }

    fn map_alias<'p>(&mut self, part: &'p str, start: usize) -> &'p str {
        if !self.settings.symbol_aliases {
            return part;
        }
        match part {
            ">" => Comparative::Cha.surface(),
            "<" => Comparative::Chu.surface(),
            "=" => Comparative::Col.surface(),
            "!=" => Comparative::Cla.surface(),
            "~=" | "+" | "-" => {
                self.report(
                    AssistDiagnosticKind::RemovedAlias {
                        alias: part.to_owned(),
                    },
                    span_of(start, start.saturating_add(part.len())),
                );
                part
            }
            _ => part,
        }
    }

    // ───────────────────────── Even slots ─────────────────────────

    fn glyph_slot(&mut self, index: usize, role: GlyphRole) -> usize {
        let Some(token) = self.tokens.get(index).copied() else {
            return 0;
        };
        self.state = State::AfterPair {
            closed_brace: false,
        };
        if role == GlyphRole::Normal {
            if let Some(extra) = self.try_multiword(index) {
                return extra;
            }
        }
        let text = self.normalize_glyph(token);
        if role == GlyphRole::Normal {
            self.check_unknown(token, &text);
        }
        self.out.push(text);
        0
    }

    /// Normalizes every DTI atom of an even-slot token into its primary form:
    /// the atoms of a qualified path (`a§b`), of a glyph anaphora base
    /// (`base~`, `base§mod~`) and of a semantic anaphora (`glyph~~`). Raw
    /// labels, the bare marks `~` / `~~` and reserved tokens are unchanged.
    fn normalize_glyph(&mut self, token: Token<'_>) -> String {
        let text = token.text;
        if self.dti.is_none()
            || text.starts_with(RAW_STRING_QUOTE)
            || text == SEMA_ANAPHORA_MARK
            || text.strip_prefix(ANAPHORA_MARK) == Some("")
            || tiles::is_reserved(text)
        {
            return text.to_owned();
        }
        let stem = text
            .strip_suffix(SEMA_ANAPHORA_MARK)
            .or_else(|| text.strip_suffix(ANAPHORA_MARK))
            .unwrap_or(text);
        let mark = text.get(stem.len()..).unwrap_or_default();
        let mut normalized = String::with_capacity(text.len());
        let mut cursor = token.start;
        for (position, atom) in stem.split(QUALIFIER_SEPARATOR).enumerate() {
            if position > 0 {
                normalized.push(QUALIFIER_SEPARATOR);
            }
            normalized.push_str(&self.normalize_atom(atom, cursor));
            cursor = cursor
                .saturating_add(atom.len())
                .saturating_add(QUALIFIER_SEPARATOR.len_utf8());
        }
        normalized.push_str(mark);
        normalized
    }

    /// Normalizes one glyph atom written at byte offset `start` (decision E,
    /// option 1). Primary forms, numerals and free identifiers are returned
    /// unchanged; a synonym or display form of exactly one enabled concept
    /// becomes its primary form (local language first). Disabled or ambiguous
    /// forms are reported and returned unchanged (the output is discarded).
    fn normalize_atom(&mut self, atom: &str, start: usize) -> String {
        let Some(dti) = self.dti else {
            return atom.to_owned();
        };
        let is_numeral = !atom.is_empty() && atom.bytes().all(|b| b.is_ascii_digit());
        if atom.is_empty() || is_numeral || is_primary_form(dti, atom) {
            return atom.to_owned();
        }
        let span = span_of(start, start.saturating_add(atom.len()));
        match dti.resolve_form(atom) {
            FormResolution::Unknown => atom.to_owned(),
            FormResolution::Unique(key) => {
                if !dti.is_enabled(&key) {
                    let suggestion = format!("use {key}");
                    self.report(
                        AssistDiagnosticKind::DisabledEntry { key, suggestion },
                        span,
                    );
                    return atom.to_owned();
                }
                match dti.primary_form(&key) {
                    Some(primary) => {
                        self.notes.push(AssistNote::NormalizedForm {
                            written: atom.to_owned(),
                            primary: primary.to_owned(),
                            span,
                        });
                        primary.to_owned()
                    }
                    None => atom.to_owned(),
                }
            }
            FormResolution::Ambiguous(keys) => {
                self.report(
                    AssistDiagnosticKind::AmbiguousForm {
                        phrase: atom.to_owned(),
                        keys,
                    },
                    span,
                );
                atom.to_owned()
            }
        }
    }

    /// Tokens that can never be the 2nd..nth word of a multiword glyph.
    fn blocks_multiword(text: &str) -> bool {
        text == OPEN_BRACE
            || text == CLOSE_BRACE
            || tiles::is_reserved(text)
            || tiles::conjunction(text).is_some()
            || tiles::portability_keyword(text).is_some()
            || tiles::parse_case(text).is_some()
            || is_signal_like(text)
    }

    /// Longest match (at most 6 tokens, at least 2) of a DTI display form.
    fn try_multiword(&mut self, index: usize) -> Option<usize> {
        if !self.settings.multiword {
            return None;
        }
        let dti = self.dti?;
        let available = self.tokens.len().saturating_sub(index);
        let longest = MAX_MULTIWORD_TOKENS.min(available);
        for count in (2..=longest).rev() {
            let Some(window) = self.tokens.get(index..index.saturating_add(count)) else {
                continue;
            };
            if window
                .iter()
                .skip(1)
                .any(|t| Self::blocks_multiword(t.text))
            {
                continue;
            }
            let (Some(first), Some(last)) = (window.first(), window.last()) else {
                continue;
            };
            let span = span_of(first.start, last.end);
            let phrase = window.iter().map(|t| t.text).collect::<Vec<_>>().join(" ");
            match dti.resolve_form(&phrase) {
                FormResolution::Unknown => continue,
                FormResolution::Unique(key) => {
                    if !dti.is_enabled(&key) {
                        let suggestion = format!("use {key}");
                        self.report(
                            AssistDiagnosticKind::DisabledEntry { key, suggestion },
                            span,
                        );
                    } else if let Some(form) = dti.primary_form(&key) {
                        self.emit(form);
                        self.notes.push(AssistNote::MultiwordGlyph {
                            phrase,
                            form: form.to_owned(),
                            span,
                        });
                    } else {
                        continue;
                    }
                }
                FormResolution::Ambiguous(keys) => {
                    // The output is discarded on error: nothing is emitted.
                    self.report(AssistDiagnosticKind::AmbiguousForm { phrase, keys }, span);
                }
            }
            return Some(count.saturating_sub(1));
        }
        None
    }

    /// Checks a Normal glyph slot after normalization: `text` is the
    /// normalized (primary-form) text of `token`.
    fn check_unknown(&mut self, token: Token<'_>, text: &str) {
        let Some(dti) = self.dti else {
            return;
        };
        if text.starts_with(RAW_STRING_QUOTE) || text.contains(ANAPHORA_MARK) {
            return;
        }
        // B9: the morphology flags are validated as in stage 3, on primary forms.
        if let Err(ResolveError::MorphViolation { form, key, flag }) = resolve_glyph_text(text, dti)
        {
            self.report(
                AssistDiagnosticKind::MorphViolation { form, key, flag },
                token.span(),
            );
        }
        // DTI forms are atoms: the glyph is unknown when one of its non-numeral
        // atoms is absent from the DTI (for a single atom: the whole glyph).
        let has_unknown_atom = text.split(QUALIFIER_SEPARATOR).any(|atom| {
            let is_numeral = !atom.is_empty() && atom.bytes().all(|b| b.is_ascii_digit());
            !is_numeral && dti.resolve_form(atom) == FormResolution::Unknown
        });
        if has_unknown_atom {
            self.notes.push(AssistNote::UnknownGlyph {
                token: token.text.to_owned(),
                span: token.span(),
                label_suggestion: reclassify_as_label(token.text).ok(),
            });
        }
    }
}

/// `true` if `atom` is the primary form of a concept in the local or in the
/// exchange-language lexicon (enabled or not: `def` names and portability ids
/// of non-enabled concepts are legitimate session symbols).
fn is_primary_form(dti: &Dti, atom: &str) -> bool {
    dti.overlay().lexicon().primary(atom).is_some()
        || dti.universal().lexicon().primary(atom).is_some()
}
