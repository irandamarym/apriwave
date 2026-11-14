//! Shortcut resolution (Phase D, H7): comparative prefix chords and custom
//! shortcuts, including `@live_variables`.

use super::settings::{Chord, EditorSettings, Key, ShortcutAction};
use crate::session::Session;

/// A key press as delivered by the editor front end.
pub type KeyPress = Chord;

/// Result of resolving a key sequence.
#[derive(Debug, PartialEq, Eq)]
pub enum ShortcutOutcome {
    /// The comparative prefix has been typed: waiting for the next key.
    Pending,
    /// Text to insert at the cursor.
    Insert(String),
    /// Names of the live `def` variables, sorted.
    LiveVariables(Vec<String>),
    NoMatch,
}

/// Resolves key sequences against borrowed [`EditorSettings`].
#[derive(Debug, Clone, Copy)]
pub struct ShortcutResolver<'s> {
    settings: &'s EditorSettings,
}

impl<'s> ShortcutResolver<'s> {
    pub const fn new(settings: &'s EditorSettings) -> Self {
        Self { settings }
    }

    /// Resolves the whole `sequence` typed so far.
    ///
    /// - `[prefix]` → [`ShortcutOutcome::Pending`];
    /// - `[prefix, ch]` → the comparative bound to `ch` (no modifier, or only
    ///   `Shift`, which many layouts need for `>` `<` `+` ...);
    /// - `[custom chord]` → its action.
    pub fn resolve(&self, session: &Session, sequence: &[KeyPress]) -> ShortcutOutcome {
        let Some((first, rest)) = sequence.split_first() else {
            return ShortcutOutcome::NoMatch;
        };
        let first = first.normalized();
        let comparative = &self.settings.comparative;

        if first == comparative.prefix.normalized() {
            return match rest {
                [] => ShortcutOutcome::Pending,
                [second] => {
                    let plain = !(second.ctrl || second.alt || second.meta);
                    match second.key {
                        Key::Char(ch) if plain => comparative
                            .lookup(ch)
                            .map_or(ShortcutOutcome::NoMatch, |c| {
                                ShortcutOutcome::Insert(c.surface().to_owned())
                            }),
                        Key::Char(_) | Key::Function(_) => ShortcutOutcome::NoMatch,
                    }
                }
                _ => ShortcutOutcome::NoMatch,
            };
        }

        if !rest.is_empty() {
            return ShortcutOutcome::NoMatch;
        }
        match self
            .settings
            .custom
            .iter()
            .find(|(chord, _)| chord.normalized() == first)
        {
            Some((_, ShortcutAction::Insert(text))) => ShortcutOutcome::Insert(text.to_owned()),
            Some((_, ShortcutAction::LiveVariables)) => ShortcutOutcome::LiveVariables(
                session
                    .live_variables()
                    .into_iter()
                    .map(|(name, _)| name.to_owned())
                    .collect(),
            ),
            None => ShortcutOutcome::NoMatch,
        }
    }
}
