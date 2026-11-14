//! Phase D editor support: assisted-input preprocessor, settings and shortcuts.
//!
//! - [`preprocess_assisted_input`]: turns relaxed editor input (comments, free
//!   whitespace, `{ }` braces, `_` signal trains, symbol aliases, multiword
//!   display forms, synonyms and display forms normalized into DTI primary
//!   forms) into canonical protocol text for [`crate::Session::ingest`].
//! - [`parse_settings`]: strict, zero-dependency parser for the `settings.toml`
//!   subset used by the editor.
//! - [`ShortcutResolver`]: resolves key sequences (comparative prefix chords and
//!   custom shortcuts, including `@live_variables`).

mod preprocess;
mod settings;
mod shortcuts;
mod toml;

pub use preprocess::{
    preprocess_assisted_input, reclassify_as_label, AssistDiagnostic, AssistDiagnosticKind,
    AssistNote, AssistOutput, AssistSettings, LabelError,
};
pub use settings::{
    parse_settings, Chord, ChordError, ComparativeShortcuts, EditorSettings, Key, SettingsError,
    SettingsErrorKind, ShortcutAction, LIVE_VARIABLES_ACTION,
};
pub use shortcuts::{KeyPress, ShortcutOutcome, ShortcutResolver};

#[cfg(test)]
mod tests;
