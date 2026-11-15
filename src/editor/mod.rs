//! Phase D editor support: assisted-input preprocessor, settings, shortcuts,
//! suggestion menu and copy with anchors.
//!
//! - [`preprocess_assisted_input`]: turns relaxed editor input (comments, free
//!   whitespace, `{ }` braces, `_` signal trains, symbol aliases, multiword
//!   display forms, synonyms and display forms normalized into DTI primary
//!   forms) into canonical protocol text for [`crate::Session::ingest`].
//! - [`parse_settings`]: strict, zero-dependency parser for the `settings.toml`
//!   subset used by the editor.
//! - [`ShortcutResolver`]: resolves key sequences (comparative prefix chords,
//!   editor actions and custom shortcuts, including `@live_variables`).
//! - [`suggestions`] (v4.1): recent glyphs and semas for a prefix, with the
//!   Glyph / Label disambiguation of the local DTI.
//! - [`copy_subordinates`] (v4.1): copies selected subordinates as an
//!   `anchor` / `anchorrel` block.

mod copy;
mod preprocess;
mod settings;
mod shortcuts;
mod suggest;
mod toml;

pub use copy::{copy_subordinates, CopiedBlock, CopyError, CopyMode, GENERATED_ID_PREFIX};
pub use preprocess::{
    preprocess_assisted_input, preprocess_with_choices, AssistDiagnostic, AssistDiagnosticKind,
    AssistNote, AssistOutput, AssistSettings, SlotChoice,
};
pub use settings::{
    parse_settings, ActionShortcuts, Autocomplete, Chord, ChordError, ComparativeShortcuts,
    EditorSettings, Key, SettingsError, SettingsErrorKind, ShortcutAction, LIVE_VARIABLES_ACTION,
};
pub use shortcuts::{KeyPress, ShortcutContext, ShortcutOutcome, ShortcutResolver};
pub use suggest::{suggestions, Suggestion, SuggestionKind, MAX_SUGGESTIONS};

#[cfg(test)]
mod tests;
