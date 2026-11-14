//! Zero-dependency parser for the TOML subset used by `settings.toml`.
//!
//! Supported: `#` comments, tables `[a]` / `[a.b]`, bare or `"quoted"` keys,
//! basic strings with the `\"` and `\\` escapes, `true` / `false` and decimal
//! integers. Everything else (arrays, inline tables, dotted keys in pairs,
//! multi-line strings, other escapes) is rejected. Duplicate tables and
//! duplicate keys in the same table are errors.

use super::settings::{SettingsError, SettingsErrorKind};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Value {
    String(String),
    Boolean(bool),
    Integer(i64),
}

#[derive(Debug)]
pub(super) struct Entry {
    pub(super) key: String,
    pub(super) value: Value,
    pub(super) line: usize,
}

#[derive(Debug)]
pub(super) struct Table {
    /// Dotted name, parts joined by `.`.
    pub(super) name: String,
    pub(super) line: usize,
    pub(super) entries: Vec<Entry>,
}

#[derive(Debug, Default)]
pub(super) struct Document {
    /// Pairs written before any table header.
    pub(super) root: Vec<Entry>,
    pub(super) tables: Vec<Table>,
}

enum Line {
    Blank,
    Table(String),
    Pair(String, Value),
}

/// Parses the whole document, collecting every syntax error with its line
/// number (1-based).
pub(super) fn parse(src: &str) -> Result<Document, Vec<SettingsError>> {
    let mut document = Document::default();
    let mut errors = Vec::new();

    for (index, raw) in src.lines().enumerate() {
        let line = index.saturating_add(1);
        match parse_line(raw) {
            Err(kind) => errors.push(SettingsError { line, kind }),
            Ok(Line::Blank) => {}
            Ok(Line::Table(name)) => {
                if document.tables.iter().any(|t| t.name == name) {
                    errors.push(SettingsError {
                        line,
                        kind: SettingsErrorKind::DuplicateTable {
                            name: String::from(name.as_str()),
                        },
                    });
                    // Later pairs still go to a table so that they are checked.
                }
                document.tables.push(Table {
                    name,
                    line,
                    entries: Vec::new(),
                });
            }
            Ok(Line::Pair(key, value)) => {
                let entries = match document.tables.last_mut() {
                    Some(table) => &mut table.entries,
                    None => &mut document.root,
                };
                if entries.iter().any(|e| e.key == key) {
                    errors.push(SettingsError {
                        line,
                        kind: SettingsErrorKind::DuplicateKey { key },
                    });
                } else {
                    entries.push(Entry { key, value, line });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(document)
    } else {
        Err(errors)
    }
}

struct Cursor<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn rest(&self) -> &'a str {
        self.text.get(self.pos..).unwrap_or_default()
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos = self.pos.saturating_add(ch.len_utf8());
        Some(ch)
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.bump();
        }
    }

    /// `true` at the end of the line or at the start of a comment.
    fn at_line_end(&mut self) -> bool {
        self.skip_spaces();
        matches!(self.peek(), None | Some('#'))
    }
}

const fn is_bare_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn parse_line(raw: &str) -> Result<Line, SettingsErrorKind> {
    let text = raw.strip_suffix('\r').unwrap_or(raw);
    let mut cursor = Cursor { text, pos: 0 };
    if cursor.at_line_end() {
        return Ok(Line::Blank);
    }
    if cursor.eat('[') {
        return parse_table_header(&mut cursor);
    }
    let key = parse_key(&mut cursor)?;
    cursor.skip_spaces();
    if cursor.peek() == Some('.') {
        return Err(SettingsErrorKind::InvalidKey);
    }
    if !cursor.eat('=') {
        return Err(SettingsErrorKind::ExpectedEquals);
    }
    cursor.skip_spaces();
    let value = parse_value(&mut cursor)?;
    if !cursor.at_line_end() {
        return Err(SettingsErrorKind::TrailingCharacters);
    }
    Ok(Line::Pair(key, value))
}

fn parse_table_header(cursor: &mut Cursor<'_>) -> Result<Line, SettingsErrorKind> {
    if cursor.peek() == Some('[') {
        // Arrays of tables are outside the subset.
        return Err(SettingsErrorKind::InvalidTableHeader);
    }
    let mut parts: Vec<String> = Vec::new();
    loop {
        cursor.skip_spaces();
        parts.push(parse_key(cursor).map_err(|_| SettingsErrorKind::InvalidTableHeader)?);
        cursor.skip_spaces();
        if cursor.eat('.') {
            continue;
        }
        if cursor.eat(']') {
            break;
        }
        return Err(SettingsErrorKind::InvalidTableHeader);
    }
    if !cursor.at_line_end() {
        return Err(SettingsErrorKind::TrailingCharacters);
    }
    Ok(Line::Table(parts.join(".")))
}

fn parse_key(cursor: &mut Cursor<'_>) -> Result<String, SettingsErrorKind> {
    if cursor.eat('"') {
        return parse_basic_string(cursor);
    }
    let start = cursor.pos;
    while cursor.peek().is_some_and(is_bare_key_char) {
        cursor.bump();
    }
    match cursor.text.get(start..cursor.pos) {
        Some(key) if !key.is_empty() => Ok(key.to_owned()),
        _ => Err(SettingsErrorKind::InvalidKey),
    }
}

/// Body of a basic string, after the opening `"`.
fn parse_basic_string(cursor: &mut Cursor<'_>) -> Result<String, SettingsErrorKind> {
    let mut out = String::new();
    loop {
        match cursor.bump() {
            None => return Err(SettingsErrorKind::UnterminatedString),
            Some('"') => return Ok(out),
            Some('\\') => match cursor.bump() {
                Some(ch @ ('"' | '\\')) => out.push(ch),
                _ => return Err(SettingsErrorKind::InvalidEscape),
            },
            Some(ch) if ch.is_control() && ch != '\t' => {
                return Err(SettingsErrorKind::ControlCharacter { ch });
            }
            Some(ch) => out.push(ch),
        }
    }
}

fn parse_value(cursor: &mut Cursor<'_>) -> Result<Value, SettingsErrorKind> {
    if cursor.eat('"') {
        return parse_basic_string(cursor).map(Value::String);
    }
    let start = cursor.pos;
    while cursor
        .peek()
        .is_some_and(|ch| is_bare_key_char(ch) || ch == '+')
    {
        cursor.bump();
    }
    let word = cursor.text.get(start..cursor.pos).unwrap_or_default();
    match word {
        "true" => Ok(Value::Boolean(true)),
        "false" => Ok(Value::Boolean(false)),
        _ => parse_integer(word).map(Value::Integer),
    }
}

fn parse_integer(word: &str) -> Result<i64, SettingsErrorKind> {
    let digits = word
        .strip_prefix('+')
        .or_else(|| word.strip_prefix('-'))
        .unwrap_or(word);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SettingsErrorKind::InvalidValue);
    }
    word.strip_prefix('+')
        .unwrap_or(word)
        .parse::<i64>()
        .map_err(|_| SettingsErrorKind::InvalidValue)
}

/// Renders `text` as a TOML basic string.
pub(super) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_add(2));
    out.push('"');
    for ch in text.chars() {
        if matches!(ch, '"' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}
