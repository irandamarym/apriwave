//! `settings.toml` model (Phase D, H7).
//!
//! ```toml
//! [editor]
//! braces = true
//! comments = true
//! signal_underscore = true
//! symbol_aliases = true
//! multiword = true
//! autocomplete = "auto"            # or "manual" (v4.1)
//!
//! [shortcuts.comparative]
//! prefix = "Alt+C"
//! ">" = "cha"
//!
//! [shortcuts.actions]               # v4.1
//! suggest = "Alt+Down"
//! copy_absolute = "Ctrl+C"
//! copy_relative = "Alt+C"
//!
//! [custom_shortcuts]
//! "Alt+V" = "@live_variables"
//! "F5" = "/^ "
//!
//! [warnings]                        # v4.1, at the user's own risk
//! ERR_SEMANTIC = "warning"
//! ERR_REFERENCE = "error"
//! ```
//!
//! Strict: unknown tables and keys are errors. Missing `[editor]` keys keep
//! their default (`true`, `autocomplete = "auto"`). A present
//! `[shortcuts.comparative]` table defines the **whole** mapping (its `prefix`
//! defaults to `Alt+C`); an absent table keeps the approved default mapping.
//! `[shortcuts.actions]` keys default to the values above; `copy_relative` may
//! equal the comparative prefix, because the two contexts are disjoint (with a
//! selection: relative copy; without: comparative insertion). In `[warnings]`
//! every code is set individually; a code not listed stays an error, and
//! structural errors stay blocking anyway.

use std::fmt;

use super::preprocess::AssistSettings;
use super::toml::{self, Entry, Value};
use crate::ast::Comparative;
use crate::error::{ErrorCode, WarningCodes};

/// Custom shortcut value that lists the live `def` variables.
pub const LIVE_VARIABLES_ACTION: &str = "@live_variables";

const TABLE_EDITOR: &str = "editor";
const TABLE_COMPARATIVE: &str = "shortcuts.comparative";
const TABLE_CUSTOM: &str = "custom_shortcuts";
const TABLE_ACTIONS: &str = "shortcuts.actions";
const TABLE_WARNINGS: &str = "warnings";
const KEY_AUTOCOMPLETE: &str = "autocomplete";
const KEY_SUGGEST: &str = "suggest";
const KEY_COPY_ABSOLUTE: &str = "copy_absolute";
const KEY_COPY_RELATIVE: &str = "copy_relative";
const DOWN_KEY: &str = "Down";
const KEY_PREFIX: &str = "prefix";
const MAX_FUNCTION_KEY: u8 = 12;

// ═══════════════════════════ Chords ═══════════════════════════

/// Key of a chord: one printable character, a function key `F1`–`F12` or the
/// `Down` arrow (v4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// ASCII letters are normalized to uppercase.
    Char(char),
    Function(u8),
    /// Down arrow (`Alt+Down` opens the suggestion menu).
    Down,
}

/// Key combination such as `Alt+C`, `Ctrl+Shift+X` or `F5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: Key,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChordError {
    Empty,
    UnknownModifier {
        modifier: String,
    },
    DuplicateModifier {
        modifier: String,
    },
    InvalidKey {
        key: String,
    },
    /// A printable key without `Ctrl`, `Alt` or `Meta` would hijack typing.
    MissingModifier,
}

impl fmt::Display for ChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty chord"),
            Self::UnknownModifier { modifier } => write!(f, "unknown modifier '{modifier}'"),
            Self::DuplicateModifier { modifier } => write!(f, "duplicate modifier '{modifier}'"),
            Self::InvalidKey { key } => write!(
                f,
                "invalid key '{key}' (expected one printable character, F1-F12 or Down)"
            ),
            Self::MissingModifier => {
                f.write_str("a character key needs a Ctrl, Alt or Meta modifier")
            }
        }
    }
}

impl std::error::Error for ChordError {}

const fn normalize_char(ch: char) -> char {
    ch.to_ascii_uppercase()
}

fn is_printable(ch: char) -> bool {
    !ch.is_control() && !ch.is_whitespace()
}

fn parse_key(text: &str) -> Result<Key, ChordError> {
    let invalid = || ChordError::InvalidKey {
        key: text.to_owned(),
    };
    if text.eq_ignore_ascii_case(DOWN_KEY) {
        return Ok(Key::Down);
    }
    let mut chars = text.chars();
    let first = chars.next().ok_or_else(invalid)?;
    if chars.as_str().is_empty() {
        return if is_printable(first) {
            Ok(Key::Char(normalize_char(first)))
        } else {
            Err(invalid())
        };
    }
    let digits = text
        .strip_prefix('F')
        .or_else(|| text.strip_prefix('f'))
        .ok_or_else(invalid)?;
    if digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    match digits.parse::<u8>() {
        Ok(n) if (1..=MAX_FUNCTION_KEY).contains(&n) => Ok(Key::Function(n)),
        _ => Err(invalid()),
    }
}

impl Chord {
    /// A plain key press without modifiers (e.g. the `>` typed after the prefix).
    pub const fn plain(ch: char) -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key: Key::Char(normalize_char(ch)),
        }
    }

    /// `Alt` + key.
    pub const fn alt(ch: char) -> Self {
        Self {
            alt: true,
            ..Self::plain(ch)
        }
    }

    /// `Ctrl` + key.
    pub const fn ctrl(ch: char) -> Self {
        Self {
            ctrl: true,
            ..Self::plain(ch)
        }
    }

    /// `Alt` + `Down` arrow.
    pub const fn alt_down() -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: false,
            meta: false,
            key: Key::Down,
        }
    }

    /// Parses `Mod+Mod+Key`: modifiers `Ctrl`, `Alt`, `Shift`, `Meta` in any
    /// order and case; key = one printable character or `F1`–`F12`
    /// (`Ctrl++` is the `+` key).
    pub fn parse(text: &str) -> Result<Self, ChordError> {
        if text.is_empty() {
            return Err(ChordError::Empty);
        }
        let (modifiers, key) = match text.strip_suffix("++") {
            Some(modifiers) => (Some(modifiers), "+"),
            None => match text.rsplit_once('+') {
                Some((modifiers, key)) => (Some(modifiers), key),
                None => (None, text),
            },
        };
        let mut chord = Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key: parse_key(key)?,
        };
        if let Some(modifiers) = modifiers {
            for modifier in modifiers.split('+') {
                let flag = match modifier.to_ascii_lowercase().as_str() {
                    "ctrl" => &mut chord.ctrl,
                    "alt" => &mut chord.alt,
                    "shift" => &mut chord.shift,
                    "meta" => &mut chord.meta,
                    _ => {
                        return Err(ChordError::UnknownModifier {
                            modifier: modifier.to_owned(),
                        })
                    }
                };
                if *flag {
                    return Err(ChordError::DuplicateModifier {
                        modifier: modifier.to_owned(),
                    });
                }
                *flag = true;
            }
        }
        if matches!(chord.key, Key::Char(_)) && !(chord.ctrl || chord.alt || chord.meta) {
            return Err(ChordError::MissingModifier);
        }
        Ok(chord)
    }

    /// Same chord with ASCII letter keys in uppercase.
    pub const fn normalized(self) -> Self {
        match self.key {
            Key::Char(ch) => Self {
                key: Key::Char(normalize_char(ch)),
                ..self
            },
            Key::Function(_) | Key::Down => self,
        }
    }
}

impl fmt::Display for Chord {
    /// Normalized form: `Ctrl+Alt+Shift+Meta+Key`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (on, name) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.meta, "Meta"),
        ] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        match self.key {
            Key::Char(ch) => write!(f, "{}", normalize_char(ch)),
            Key::Function(n) => write!(f, "F{n}"),
            Key::Down => f.write_str(DOWN_KEY),
        }
    }
}

// ═══════════════════════════ Model ═══════════════════════════

/// `[shortcuts.comparative]`: prefix chord, then one character per comparative.
#[derive(Debug, PartialEq, Eq)]
pub struct ComparativeShortcuts {
    pub prefix: Chord,
    pub bindings: Vec<(char, Comparative)>,
}

impl Default for ComparativeShortcuts {
    /// Approved mapping (H7): `Alt+C` then `>` `<` `=` `!` `~` `[` `-` `+`.
    fn default() -> Self {
        Self {
            prefix: Chord::alt('c'),
            bindings: vec![
                ('>', Comparative::Cha),
                ('<', Comparative::Chu),
                ('=', Comparative::Col),
                ('!', Comparative::Cla),
                ('~', Comparative::Cal),
                ('[', Comparative::Cua),
                ('-', Comparative::Cuo),
                ('+', Comparative::Cup),
            ],
        }
    }
}

impl ComparativeShortcuts {
    /// Comparative bound to the character typed after the prefix.
    pub fn lookup(&self, ch: char) -> Option<Comparative> {
        let ch = normalize_char(ch);
        self.bindings
            .iter()
            .find(|(key, _)| normalize_char(*key) == ch)
            .map(|(_, comparative)| *comparative)
    }
}

/// Action of a custom shortcut.
#[derive(Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Literal text insertion.
    Insert(String),
    /// `@live_variables`: list of the live `def` variables.
    LiveVariables,
}

/// When the suggestion menu opens (v4.1, `[editor] autocomplete`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Autocomplete {
    /// The menu opens automatically under the text while typing.
    #[default]
    Auto,
    /// The menu opens only on request (`suggest` chord, `Alt+Down`).
    Manual,
}

impl Autocomplete {
    pub const fn surface(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

/// `[shortcuts.actions]` (v4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionShortcuts {
    /// Opens the suggestion menu (default `Alt+Down`).
    pub suggest: Chord,
    /// With a selection: copy as an absolute anchor block (default `Ctrl+C`).
    pub copy_absolute: Chord,
    /// With a selection: copy as a relative anchor block (default `Alt+C`).
    pub copy_relative: Chord,
}

impl Default for ActionShortcuts {
    fn default() -> Self {
        Self {
            suggest: Chord::alt_down(),
            copy_absolute: Chord::ctrl('c'),
            copy_relative: Chord::alt('c'),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct EditorSettings {
    /// `[editor]` booleans.
    pub assist: AssistSettings,
    /// `[editor] autocomplete`.
    pub autocomplete: Autocomplete,
    /// `[shortcuts.comparative]`.
    pub comparative: ComparativeShortcuts,
    /// `[shortcuts.actions]`.
    pub actions: ActionShortcuts,
    /// `[custom_shortcuts]`, in file order.
    pub custom: Vec<(Chord, ShortcutAction)>,
    /// `[warnings]`: codes downgraded to warnings (pass them to
    /// [`crate::ParserConfig::warnings`]).
    pub warnings: WarningCodes,
}

// ═══════════════════════════ Errors ═══════════════════════════

#[derive(Debug, PartialEq, Eq)]
pub struct SettingsError {
    /// 1-based line number.
    pub line: usize,
    pub kind: SettingsErrorKind,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.kind)
    }
}

impl std::error::Error for SettingsError {}

#[derive(Debug, PartialEq, Eq)]
pub enum SettingsErrorKind {
    // ── Syntax ──
    InvalidTableHeader,
    InvalidKey,
    ExpectedEquals,
    InvalidValue,
    UnterminatedString,
    InvalidEscape,
    ControlCharacter {
        ch: char,
    },
    TrailingCharacters,
    DuplicateTable {
        name: String,
    },
    DuplicateKey {
        key: String,
    },
    // ── Schema ──
    KeyOutsideTable {
        key: String,
    },
    UnknownTable {
        name: String,
    },
    UnknownKey {
        table: String,
        key: String,
    },
    ExpectedBoolean {
        key: String,
    },
    ExpectedString {
        key: String,
    },
    InvalidComparativeKey {
        key: String,
    },
    InvalidComparativeValue {
        value: String,
    },
    InvalidChord {
        chord: String,
        error: ChordError,
    },
    DuplicateChord {
        chord: String,
    },
    /// A custom chord equal to the comparative prefix or to an editor action.
    ChordConflict {
        chord: String,
    },
    EmptyInsertion,
    ControlCharacterInInsertion {
        ch: char,
    },
    /// `autocomplete` is neither `"auto"` nor `"manual"`.
    InvalidAutocomplete {
        value: String,
    },
    /// A `[warnings]` key that is not one of the 6 error codes.
    UnknownErrorCode {
        code: String,
    },
    /// A `[warnings]` value other than `"warning"` / `"error"`.
    InvalidSeverity {
        value: String,
    },
}

impl fmt::Display for SettingsErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTableHeader => f.write_str("invalid table header"),
            Self::InvalidKey => f.write_str("invalid key"),
            Self::ExpectedEquals => f.write_str("expected '='"),
            Self::InvalidValue => {
                f.write_str("invalid value (expected a string, a boolean or an integer)")
            }
            Self::UnterminatedString => f.write_str("unterminated string"),
            Self::InvalidEscape => f.write_str("invalid escape (only \\\" and \\\\ are allowed)"),
            Self::ControlCharacter { ch } => {
                write!(f, "control character U+{:04X} in string", u32::from(*ch))
            }
            Self::TrailingCharacters => f.write_str("unexpected characters at the end of the line"),
            Self::DuplicateTable { name } => write!(f, "duplicate table [{name}]"),
            Self::DuplicateKey { key } => write!(f, "duplicate key '{key}'"),
            Self::KeyOutsideTable { key } => write!(f, "key '{key}' outside any table"),
            Self::UnknownTable { name } => write!(f, "unknown table [{name}]"),
            Self::UnknownKey { table, key } => write!(f, "unknown key '{key}' in [{table}]"),
            Self::ExpectedBoolean { key } => write!(f, "'{key}' must be true or false"),
            Self::ExpectedString { key } => write!(f, "'{key}' must be a string"),
            Self::InvalidComparativeKey { key } => {
                write!(f, "comparative key '{key}' must be one printable character")
            }
            Self::InvalidComparativeValue { value } => {
                write!(
                    f,
                    "'{value}' is not a comparative (cha chu cla col cal cua cuo cup)"
                )
            }
            Self::InvalidChord { chord, error } => write!(f, "invalid chord '{chord}': {error}"),
            Self::DuplicateChord { chord } => write!(f, "duplicate chord '{chord}'"),
            Self::ChordConflict { chord } => {
                write!(
                    f,
                    "chord '{chord}' is already the comparative prefix or an editor action"
                )
            }
            Self::EmptyInsertion => f.write_str("empty insertion text"),
            Self::ControlCharacterInInsertion { ch } => write!(
                f,
                "control character U+{:04X} in insertion text",
                u32::from(*ch)
            ),
            Self::InvalidAutocomplete { value } => {
                write!(f, "autocomplete '{value}' must be \"auto\" or \"manual\"")
            }
            Self::UnknownErrorCode { code } => write!(
                f,
                "'{code}' is not an error code (ERR_SYNTAX ERR_ORDER ERR_REFERENCE ERR_SEMANTIC ERR_SCOPE ERR_CASE_AMBIGUITY)"
            ),
            Self::InvalidSeverity { value } => {
                write!(f, "severity '{value}' must be \"warning\" or \"error\"")
            }
        }
    }
}

// ═══════════════════════════ Parsing ═══════════════════════════

fn error(errors: &mut Vec<SettingsError>, line: usize, kind: SettingsErrorKind) {
    errors.push(SettingsError { line, kind });
}

fn apply_editor(
    entries: Vec<Entry>,
    assist: &mut AssistSettings,
    autocomplete: &mut Autocomplete,
    errors: &mut Vec<SettingsError>,
) {
    for Entry { key, value, line } in entries {
        if key == KEY_AUTOCOMPLETE {
            match value {
                Value::String(text) => match text.as_str() {
                    "auto" => *autocomplete = Autocomplete::Auto,
                    "manual" => *autocomplete = Autocomplete::Manual,
                    _ => error(
                        errors,
                        line,
                        SettingsErrorKind::InvalidAutocomplete { value: text },
                    ),
                },
                Value::Boolean(_) | Value::Integer(_) => {
                    error(errors, line, SettingsErrorKind::ExpectedString { key });
                }
            }
            continue;
        }
        let flag = match key.as_str() {
            "braces" => &mut assist.braces,
            "comments" => &mut assist.comments,
            "signal_underscore" => &mut assist.signal_underscore,
            "symbol_aliases" => &mut assist.symbol_aliases,
            "multiword" => &mut assist.multiword,
            _ => {
                error(
                    errors,
                    line,
                    SettingsErrorKind::UnknownKey {
                        table: TABLE_EDITOR.to_owned(),
                        key,
                    },
                );
                continue;
            }
        };
        match value {
            Value::Boolean(on) => *flag = on,
            Value::String(_) | Value::Integer(_) => {
                error(errors, line, SettingsErrorKind::ExpectedBoolean { key });
            }
        }
    }
}

fn parse_chord(text: String, line: usize, errors: &mut Vec<SettingsError>) -> Option<Chord> {
    match Chord::parse(&text) {
        Ok(chord) => Some(chord),
        Err(chord_error) => {
            error(
                errors,
                line,
                SettingsErrorKind::InvalidChord {
                    chord: text,
                    error: chord_error,
                },
            );
            None
        }
    }
}

/// Returns the mapping and the line of the `prefix` key (if written).
fn parse_comparative(
    entries: Vec<Entry>,
    errors: &mut Vec<SettingsError>,
) -> (ComparativeShortcuts, Option<usize>) {
    let mut shortcuts = ComparativeShortcuts {
        bindings: Vec::new(),
        ..ComparativeShortcuts::default()
    };
    let mut prefix_line = None;
    for Entry { key, value, line } in entries {
        let Value::String(text) = value else {
            error(errors, line, SettingsErrorKind::ExpectedString { key });
            continue;
        };
        if key == KEY_PREFIX {
            prefix_line = Some(line);
            if let Some(chord) = parse_chord(text, line, errors) {
                shortcuts.prefix = chord;
            }
            continue;
        }
        let mut chars = key.chars();
        let ch = match (chars.next(), chars.next()) {
            (Some(ch), None) if is_printable(ch) => ch,
            _ => {
                error(
                    errors,
                    line,
                    SettingsErrorKind::InvalidComparativeKey { key },
                );
                continue;
            }
        };
        let Some(comparative) = Comparative::ALL.into_iter().find(|c| c.surface() == text) else {
            error(
                errors,
                line,
                SettingsErrorKind::InvalidComparativeValue { value: text },
            );
            continue;
        };
        if shortcuts.lookup(ch).is_some() {
            // Same letter in another case (e.g. `c` and `C`).
            error(errors, line, SettingsErrorKind::DuplicateKey { key });
            continue;
        }
        shortcuts.bindings.push((ch, comparative));
    }
    (shortcuts, prefix_line)
}

/// `[shortcuts.actions]`: every key is optional (defaults of
/// [`ActionShortcuts`]).
fn parse_actions(entries: Vec<Entry>, errors: &mut Vec<SettingsError>) -> ActionShortcuts {
    let mut actions = ActionShortcuts::default();
    for Entry { key, value, line } in entries {
        let slot = match key.as_str() {
            KEY_SUGGEST => &mut actions.suggest,
            KEY_COPY_ABSOLUTE => &mut actions.copy_absolute,
            KEY_COPY_RELATIVE => &mut actions.copy_relative,
            _ => {
                error(
                    errors,
                    line,
                    SettingsErrorKind::UnknownKey {
                        table: TABLE_ACTIONS.to_owned(),
                        key,
                    },
                );
                continue;
            }
        };
        let Value::String(text) = value else {
            error(errors, line, SettingsErrorKind::ExpectedString { key });
            continue;
        };
        if let Some(chord) = parse_chord(text, line, errors) {
            *slot = chord;
        }
    }
    actions
}

/// `[warnings]`: one entry per error code, `"warning"` or `"error"`.
fn parse_warnings(entries: Vec<Entry>, errors: &mut Vec<SettingsError>) -> WarningCodes {
    let mut warnings = WarningCodes::NONE;
    for Entry { key, value, line } in entries {
        let Some(code) = ErrorCode::from_name(&key) else {
            error(
                errors,
                line,
                SettingsErrorKind::UnknownErrorCode { code: key },
            );
            continue;
        };
        let Value::String(text) = value else {
            error(errors, line, SettingsErrorKind::ExpectedString { key });
            continue;
        };
        match text.as_str() {
            "warning" => warnings = warnings.with(code),
            "error" => warnings = warnings.without(code),
            _ => error(
                errors,
                line,
                SettingsErrorKind::InvalidSeverity { value: text },
            ),
        }
    }
    warnings
}

fn parse_action(
    text: String,
    line: usize,
    errors: &mut Vec<SettingsError>,
) -> Option<ShortcutAction> {
    if text == LIVE_VARIABLES_ACTION {
        return Some(ShortcutAction::LiveVariables);
    }
    if text.is_empty() {
        error(errors, line, SettingsErrorKind::EmptyInsertion);
        return None;
    }
    if let Some(ch) = text.chars().find(|c| c.is_control()) {
        error(
            errors,
            line,
            SettingsErrorKind::ControlCharacterInInsertion { ch },
        );
        return None;
    }
    Some(ShortcutAction::Insert(text))
}

fn parse_custom(
    entries: Vec<Entry>,
    errors: &mut Vec<SettingsError>,
) -> Vec<(Chord, ShortcutAction, usize)> {
    let mut custom: Vec<(Chord, ShortcutAction, usize)> = Vec::new();
    for Entry { key, value, line } in entries {
        let Value::String(text) = value else {
            error(errors, line, SettingsErrorKind::ExpectedString { key });
            continue;
        };
        let chord = parse_chord(key, line, errors);
        let action = parse_action(text, line, errors);
        let (Some(chord), Some(action)) = (chord, action) else {
            continue;
        };
        if custom.iter().any(|(c, _, _)| *c == chord) {
            error(
                errors,
                line,
                SettingsErrorKind::DuplicateChord {
                    chord: chord.to_string(),
                },
            );
            continue;
        }
        custom.push((chord, action, line));
    }
    custom
}

/// Parses `settings.toml`. `Err` carries every error with its line number.
pub fn parse_settings(src: &str) -> Result<EditorSettings, Vec<SettingsError>> {
    let document = toml::parse(src)?;
    let mut errors = Vec::new();
    let mut settings = EditorSettings::default();
    let mut custom: Vec<(Chord, ShortcutAction, usize)> = Vec::new();
    let mut prefix_line: Option<usize> = None;

    for Entry { key, line, .. } in document.root {
        error(
            &mut errors,
            line,
            SettingsErrorKind::KeyOutsideTable { key },
        );
    }
    for table in document.tables {
        match table.name.as_str() {
            TABLE_EDITOR => apply_editor(
                table.entries,
                &mut settings.assist,
                &mut settings.autocomplete,
                &mut errors,
            ),
            TABLE_ACTIONS => settings.actions = parse_actions(table.entries, &mut errors),
            TABLE_WARNINGS => settings.warnings = parse_warnings(table.entries, &mut errors),
            TABLE_COMPARATIVE => {
                let (shortcuts, line) = parse_comparative(table.entries, &mut errors);
                settings.comparative = shortcuts;
                prefix_line = line.or(Some(table.line));
            }
            TABLE_CUSTOM => custom = parse_custom(table.entries, &mut errors),
            _ => error(
                &mut errors,
                table.line,
                SettingsErrorKind::UnknownTable { name: table.name },
            ),
        }
    }

    let prefix = settings.comparative.prefix;
    let actions = settings.actions;
    let reserved = [
        prefix,
        actions.suggest,
        actions.copy_absolute,
        actions.copy_relative,
    ];
    for (chord, action, line) in custom {
        if reserved.contains(&chord) {
            error(
                &mut errors,
                line.max(prefix_line.unwrap_or_default()),
                SettingsErrorKind::ChordConflict {
                    chord: chord.to_string(),
                },
            );
            continue;
        }
        settings.custom.push((chord, action));
    }

    if errors.is_empty() {
        Ok(settings)
    } else {
        errors.sort_by_key(|e| e.line);
        Err(errors)
    }
}

impl EditorSettings {
    /// Renders the settings as `settings.toml` (round trip with [`parse_settings`]).
    pub fn to_toml(&self) -> String {
        let mut out = String::from("# ApriWave editor settings\n\n[editor]\n");
        let assist = &self.assist;
        for (name, on) in [
            ("braces", assist.braces),
            ("comments", assist.comments),
            ("signal_underscore", assist.signal_underscore),
            ("symbol_aliases", assist.symbol_aliases),
            ("multiword", assist.multiword),
        ] {
            out.push_str(&format!("{name} = {on}\n"));
        }
        out.push_str(&format!(
            "{KEY_AUTOCOMPLETE} = {}\n",
            toml::quote(self.autocomplete.surface())
        ));

        out.push_str(&format!(
            "\n[{TABLE_COMPARATIVE}]\n{KEY_PREFIX} = {}\n",
            toml::quote(&self.comparative.prefix.to_string())
        ));
        for (ch, comparative) in &self.comparative.bindings {
            out.push_str(&format!(
                "{} = {}\n",
                toml::quote(&ch.to_string()),
                toml::quote(comparative.surface())
            ));
        }

        out.push_str(&format!("\n[{TABLE_ACTIONS}]\n"));
        for (name, chord) in [
            (KEY_SUGGEST, self.actions.suggest),
            (KEY_COPY_ABSOLUTE, self.actions.copy_absolute),
            (KEY_COPY_RELATIVE, self.actions.copy_relative),
        ] {
            out.push_str(&format!("{name} = {}\n", toml::quote(&chord.to_string())));
        }

        out.push_str(&format!("\n[{TABLE_CUSTOM}]\n"));
        for (chord, action) in &self.custom {
            let value = match action {
                ShortcutAction::Insert(text) => toml::quote(text),
                ShortcutAction::LiveVariables => toml::quote(LIVE_VARIABLES_ACTION),
            };
            out.push_str(&format!("{} = {value}\n", toml::quote(&chord.to_string())));
        }

        out.push_str(&format!("\n[{TABLE_WARNINGS}]\n"));
        for code in ErrorCode::ALL {
            let severity = if self.warnings.contains(code) {
                "warning"
            } else {
                "error"
            };
            out.push_str(&format!("{} = {}\n", code.as_str(), toml::quote(severity)));
        }
        out
    }
}
