//! Ghost Protocol v2 (B4): stream cipher by semantic mapping over DTI keys.
//!
//! A canonical ApriWave period in **indexed wire form** (every glyph atom is a
//! 32-character DTI key or a numeral) is encrypted by replacing each atom with
//! another atom of the same class. The ciphertext is again a valid ApriWave
//! period **at stage 1 and at stage 2** (in the context of its stream), so it
//! travels through any ApriWave channel and intermediate node unchanged; the
//! nonce, the sequence number and the MAC tag travel out of band in the
//! [`GhostEnvelope`]. Decryption is exact.
//!
//! # Construction (G1–G8)
//!
//! - **Domain (G4)**: all non-deprecated keys of the universal snapshot `S`
//!   (`superseded_by` absent), sorted by key bytes. DTI keys always start with
//!   a letter, so a key can never be mistaken for a numeral. Keys outside `S`
//!   (ephemeral session keys and local keys, `K \ S`) form their own class,
//!   permuted by a Feistel network (see [`feistel`]); deprecated keys are
//!   rejected. The snapshot `#version` is bound into the keystream and the MAC.
//! - **Keystream (G2)**: for the token at position `P` of its class (0-based,
//!   textual order, one independent counter per class: see the table in the
//!   `transform` module) the suite XOF is keyed with
//!   `field("ApriWave-Ghost-v2/keystream") || master key || nonce ||
//!   version (u64 LE) || field(class label) || P (u64 LE)`, where
//!   `field(x) = len(x) (u64 LE) || x` (with BLAKE3 the master key is the key of
//!   the native keyed mode instead of being absorbed). It drives the **base
//!   permutation** `τ_P` of the class: Fisher–Yates over the domain with unbiased
//!   rejection sampling (`glyph-atom`, O(N) per token, accepted by design), one
//!   Fisher–Yates permutation of `0..B` per digit (`numeral-digit`), or the
//!   cycle-walked Feistel network (`ephemeral-key`).
//! - **Bound symbols and visible antecedents (G1, option 2; rules 1–3)**: the
//!   atom is mapped with the bijection `σ_P` obtained from `τ_P` by **chain
//!   walking** over the live entries of an injective symbol table (see the
//!   `bijection`, `table` and `walk` modules): every live plaintext atom maps to
//!   its cipher atom, every other atom maps outside the live cipher atoms. The
//!   table holds `def` variable names and values (until `abse ~` or `^^/`), open
//!   anchor ids (until `end ~`), the atoms of the blocks visible as antecedents
//!   (R6 search scope: active DSP stack, level-0 blocks of the period, level-0
//!   blocks carried by `/^/`, released on DSP pruning and on `^/` / `^^/`) and
//!   the block being written. Hence `def` names, R13 references, `abfi`/`absi`/
//!   `abse` names and `anchor`/`rel`/`reloff` ids take the cipher atoms of what
//!   they refer to, no other atom can capture them, and stage 2 resolves the ciphertext
//!   exactly like the plaintext. With an empty table `σ_P = τ_P`.
//! - **What is encrypted (G5)**: every atom of the period: slot glyphs and their
//!   `§` qualifiers, `def` names and values, portability ids.
//!   Numerals keep their length and base (leading zeros may appear); a numeral
//!   without a declared numeral system ([`GhostError::NoNumeralSystem`]) or with
//!   a digit `≥ B` ([`GhostError::DigitOutOfBase`]) is rejected. An ephemeral key
//!   is accepted only as a bound symbol (`def` name, portability id, R13
//!   reference): elsewhere the period is not a valid indexed form
//!   ([`GhostError::UnboundEphemeralKey`]). The bare markers `~` and `~~`,
//!   `abse ~`, `end ~`, connectors and keywords are left untouched; labels
//!   (`LVA…`, v4.1) are rejected ([`GhostError::Label`]). With [`GhostParams::permute_grammar_tiles`] the primary
//!   signal (within `{ta, to, ka, ko, ot}`: `oh` is fixed), the tense slot
//!   (within `{omitted, 6 indicative}` and within the 6 conditionals), the
//!   number of every relational signal tile and of every case tile are also
//!   permuted (G2, option 1); `def` tiles are never permuted.
//! - **Integrity (G3)**: a 32-byte MAC over suite, version, flags, nonce,
//!   sequence number and ciphertext (see [`mac`]); compared in constant time.
//! - **Stream state (G8)**: [`GhostSession`] keeps on both endpoints the
//!   sequence counter (the receiver accepts exactly the next one), the plaintext
//!   and ciphertext stage-2 sessions and the symbol table. Each message is
//!   processed atomically.
//! - **Replay (G6)**: inside a session the sequence number rejects replayed and
//!   reordered messages; the stateless [`decrypt_period`] uses a
//!   [`ReplayWindow`] of nonces.
//! - **KDF (G7)**: [`Argon2idKdf`] (feature `ghost-argon2`) for passphrases,
//!   [`Shake256KeyDerivation`] for high-entropy secrets only.
//!
//! # Stateless API
//!
//! [`encrypt_period`] / [`decrypt_period`] are thin wrappers over a **fresh**
//! [`GhostSession`] (sequence 0, no variables, no carried antecedents): they are
//! correct for a self-contained period that does not depend on previous
//! periods (no `/^≈`, no variables or `/^/` antecedents from earlier messages).
//!
//! # Security notes and limitations
//!
//! - The crate never generates randomness: the caller supplies a unique 128-bit
//!   [`Nonce`] per message. Reusing a nonce with the same key yields the same
//!   ciphertext for the same plaintext and state.
//! - Only the identity of the keys, the digit values of numerals and,
//!   optionally, the grammar tiles are hidden. The structure of the period
//!   (blocks, pairs, connectors, anaphora markers, qualifier counts, `def` and
//!   portability layout, which atoms are numerals or ephemeral keys and their
//!   lengths) is visible.
//! - Atoms outside the symbol table use independent permutations per position.
//!   While atoms are held by the table (bound symbols and visible antecedents),
//!   equal plaintext atoms map to equal cipher atoms: equality inside the
//!   visible scope is the price of a stage-2 valid ciphertext.

mod bijection;
pub mod feistel;
pub mod kdf;
mod keccak;
pub mod mac;
mod permutation;
mod primitives;
mod session;
mod table;
mod transform;
mod walk;

#[cfg(test)]
mod tests;

use std::fmt;

use crate::dti::{Dti, KeyError};
use crate::error::Diagnostic;
use crate::span::Span;

pub use kdf::{Argon2idKdf, Kdf, Shake256KeyDerivation};
pub use keccak::{Shake256, Shake256Reader, SHAKE256_RATE};
pub use mac::{ReplayWindow, MAC_LABEL, MAC_LENGTH};
pub use primitives::{CryptoSuite, Xof, XofReader};
pub use session::GhostSession;
pub use table::{Atom, SymbolTable};
pub use transform::KEYSTREAM_LABEL;

/// Length of the master key in bytes.
pub const MASTER_KEY_LENGTH: usize = 32;
/// Length of the nonce in bytes.
pub const NONCE_LENGTH: usize = 16;

/// 256-bit master key.
///
/// `Debug` never prints the key. On `Drop` the bytes are overwritten with
/// zeros; note that without `unsafe` volatile writes (forbidden in this crate)
/// the compiler is allowed to elide that store, and copies made by the caller or
/// by moves are not wiped, so this is best effort only.
pub struct MasterKey([u8; MASTER_KEY_LENGTH]);

impl MasterKey {
    /// Wraps raw key bytes (the caller's copy is not wiped).
    pub const fn new(bytes: [u8; MASTER_KEY_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Derives a key with any [`Kdf`].
    pub fn derive(kdf: &dyn Kdf, secret: &[u8], salt: &[u8]) -> Result<Self, GhostError> {
        let mut key = Self([0; MASTER_KEY_LENGTH]);
        kdf.derive(secret, salt, &mut key.0)?;
        Ok(key)
    }

    pub(crate) const fn bytes(&self) -> &[u8; MASTER_KEY_LENGTH] {
        &self.0
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(<redacted>)")
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.0.iter_mut().for_each(|byte| *byte = 0);
        std::hint::black_box(&mut self.0);
    }
}

/// 128-bit nonce, supplied by the caller (the crate never generates randomness).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nonce(pub [u8; NONCE_LENGTH]);

/// Session parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GhostParams {
    pub suite: CryptoSuite,
    /// Also permute primary signals, tense slots, signal-tile numbers and
    /// case-tile numbers (default `false`; `def` tiles are never permuted).
    /// Bound into the MAC: a mismatch is detected as `MacMismatch`.
    pub permute_grammar_tiles: bool,
}

impl Default for GhostParams {
    fn default() -> Self {
        Self {
            suite: CryptoSuite::Shake256,
            permute_grammar_tiles: false,
        }
    }
}

/// Encrypted period plus out-of-band metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostEnvelope {
    pub suite: CryptoSuite,
    pub snapshot_version: u64,
    pub nonce: Nonce,
    /// Position of the message in its [`GhostSession`] stream (authenticated).
    pub sequence: u64,
    /// Stage-1 and stage-2 valid ApriWave period.
    pub ciphertext: String,
    pub mac_tag: [u8; MAC_LENGTH],
}

/// Ghost Protocol errors (separate from the 6 grammar error codes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GhostError {
    /// The suite's Cargo feature is not enabled.
    SuiteUnavailable { suite: CryptoSuite },
    /// The Argon2id KDF requires the `ghost-argon2` feature.
    KdfUnavailable,
    /// Envelope suite differs from the session suite.
    SuiteMismatch {
        expected: CryptoSuite,
        found: CryptoSuite,
    },
    /// Envelope snapshot version differs from the loaded universal snapshot.
    SnapshotVersionMismatch { expected: u64, found: u64 },
    /// The text is not a stage-1 valid input (lexer/parser diagnostics).
    Stage1 { diagnostics: Vec<Diagnostic> },
    /// The period is not valid at stage 2 in the context of the stream.
    Stage2 { diagnostics: Vec<Diagnostic> },
    /// Exactly one period is required.
    PeriodCount { found: usize },
    /// Labels (`LVAAAA<form>`, v4.1) cannot be encrypted.
    Label { span: Span },
    /// A numeral appears but the universal snapshot declares no numeral system.
    NoNumeralSystem { span: Span },
    /// A numeral digit is not below the base of the snapshot numeral system
    /// (`span` covers the offending digit).
    DigitOutOfBase { span: Span, digit: u8, base: u8 },
    /// A glyph atom is not a valid DTI key.
    InvalidKey { span: Span, error: KeyError },
    /// A deprecated universal key.
    KeyNotInDomain { span: Span },
    /// A key outside the universal snapshot (ephemeral or local key) outside a
    /// bound position (`def` name, portability id, R13 reference).
    UnboundEphemeralKey { span: Span },
    /// An AST span does not match the text.
    InvalidSpan { span: Span },
    /// The keystream kept producing rejected draws.
    KeystreamExhausted,
    /// MAC verification failed.
    MacMismatch,
    /// The nonce (stateless API) or the sequence number has already been
    /// accepted.
    Replay,
    /// The sequence number is ahead of the next expected one.
    OutOfOrder { expected: u64, found: u64 },
    /// The KDF salt is shorter than required.
    SaltTooShort { minimum: usize, found: usize },
    /// The KDF rejected its parameters or input.
    Kdf { reason: String },
    /// Internal invariant violated (never expected).
    Internal { context: &'static str },
}

impl fmt::Display for GhostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SuiteUnavailable { suite } => {
                write!(
                    f,
                    "crypto suite {suite} is not compiled in (enable its feature)"
                )
            }
            Self::KdfUnavailable => f.write_str("Argon2id requires the `ghost-argon2` feature"),
            Self::SuiteMismatch { expected, found } => {
                write!(
                    f,
                    "suite mismatch: session uses {expected}, envelope uses {found}"
                )
            }
            Self::SnapshotVersionMismatch { expected, found } => write!(
                f,
                "snapshot version mismatch: loaded #{expected}, envelope #{found}"
            ),
            Self::Stage1 { diagnostics } => {
                write!(
                    f,
                    "input is not a valid period ({} diagnostics)",
                    diagnostics.len()
                )?;
                if let Some(first) = diagnostics.first() {
                    write!(f, ": {first}")?;
                }
                Ok(())
            }
            Self::Stage2 { diagnostics } => {
                write!(
                    f,
                    "period is not valid at stage 2 ({} diagnostics)",
                    diagnostics.len()
                )?;
                if let Some(first) = diagnostics.first() {
                    write!(f, ": {first}")?;
                }
                Ok(())
            }
            Self::PeriodCount { found } => write!(f, "exactly one period required, found {found}"),
            Self::Label { span } => {
                write!(
                    f,
                    "label at {}..{} cannot be encrypted",
                    span.start, span.end
                )
            }
            Self::NoNumeralSystem { span } => write!(
                f,
                "numeral at {}..{} cannot be encrypted: the snapshot declares no numeral system",
                span.start, span.end
            ),
            Self::DigitOutOfBase { span, digit, base } => write!(
                f,
                "digit {digit} at {}..{} is not below the numeral base {base}",
                span.start, span.end
            ),
            Self::InvalidKey { span, error } => write!(
                f,
                "glyph atom at {}..{} is not a DTI key: {error}",
                span.start, span.end
            ),
            Self::KeyNotInDomain { span } => write!(
                f,
                "key at {}..{} is a deprecated key of the universal snapshot",
                span.start, span.end
            ),
            Self::UnboundEphemeralKey { span } => write!(
                f,
                "key at {}..{} is outside the universal snapshot and not a bound symbol",
                span.start, span.end
            ),
            Self::InvalidSpan { span } => {
                write!(
                    f,
                    "span {}..{} does not match the text",
                    span.start, span.end
                )
            }
            Self::KeystreamExhausted => f.write_str("keystream rejected too many draws"),
            Self::MacMismatch => f.write_str("MAC verification failed"),
            Self::Replay => f.write_str("message already accepted (replay)"),
            Self::OutOfOrder { expected, found } => write!(
                f,
                "message out of order: expected sequence {expected}, found {found}"
            ),
            Self::SaltTooShort { minimum, found } => {
                write!(
                    f,
                    "salt too short: {found} bytes, at least {minimum} required"
                )
            }
            Self::Kdf { reason } => write!(f, "key derivation failed: {reason}"),
            Self::Internal { context } => write!(f, "internal error: {context}"),
        }
    }
}

impl std::error::Error for GhostError {}

fn ensure_available(suite: CryptoSuite) -> Result<(), GhostError> {
    if suite.is_available() {
        Ok(())
    } else {
        Err(GhostError::SuiteUnavailable { suite })
    }
}

/// Encrypts one self-contained canonical period in indexed wire form
/// (stateless: a fresh [`GhostSession`], sequence 0).
pub fn encrypt_period(
    plain: &str,
    dti: &Dti,
    key: &MasterKey,
    nonce: Nonce,
    params: &GhostParams,
) -> Result<GhostEnvelope, GhostError> {
    GhostSession::new(*params).encrypt(plain, dti, key, nonce)
}

/// Verifies and decrypts an envelope produced by [`encrypt_period`]
/// (stateless: a fresh [`GhostSession`], so the sequence number must be 0).
///
/// Order: suite/version check → MAC → nonce freshness → decryption → the nonce
/// is recorded only after a successful decryption.
pub fn decrypt_period(
    envelope: &GhostEnvelope,
    dti: &Dti,
    key: &MasterKey,
    params: &GhostParams,
    replay: &mut ReplayWindow,
) -> Result<String, GhostError> {
    let mut session = GhostSession::new(*params);
    session.verify(envelope, dti, key)?;
    if replay.contains(&envelope.nonce) {
        return Err(GhostError::Replay);
    }
    let plain = session.decrypt(envelope, dti, key)?;
    replay.record(envelope.nonce);
    Ok(plain)
}
