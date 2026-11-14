//! Two-level DTI (Phase B, D1–D6, B1–B9, B11).
//!
//! - [`DtiKey`]: 32-character immutable concept key with Luhn mod 36 check.
//! - [`UniversalSnapshot`]: read-only universal authority (one exchange language).
//! - [`LocalOverlay`]: enabled keys, local forms, local concepts, policy.
//! - [`Dti`]: joined lookup view (`resolve_form`, `resolve_key`, forms).
//! - `.dti.bin` deterministic binary cache on buffers: [`UniversalSnapshot::to_binary`],
//!   [`load_or_compile_universal`], … (the file-system API is in `apriwave`).
//!
//! DTI diagnostics are separate from the 6 grammar error codes (Q26).

mod binary;
mod key;
mod model;
mod parse;

pub use binary::{
    fnv1a_64, load_or_compile_local, load_or_compile_universal, DtiLoadError,
    BINARY_FORMAT_VERSION, BINARY_MAGIC, MAX_BINARY_BYTES,
};
pub use key::{DtiKey, KeyError, KEY_BODY_LENGTH, KEY_LENGTH, SESSION_SYMBOL_DOMAIN};
pub use model::{
    ConceptRecord, Dti, FormResolution, KeyResolution, LexicalEntry, Lexicon, LocalOverlay,
    MorphFlags, NumeralSystem, UniversalSnapshot, UnknownKeyPolicy,
};
pub use parse::{DtiDiagnostic, DtiErrorKind, DtiSource, MAX_SOURCE_BYTES};

#[cfg(test)]
mod tests;
