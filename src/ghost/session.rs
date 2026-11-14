//! G8: [`GhostSession`], the stateful Ghost stream (decision C, option 1).
//!
//! Both endpoints keep the same state:
//!
//! - a **sequence counter**: message `n` of the stream carries `sequence = n`,
//!   authenticated by the MAC; the receiver accepts exactly the next sequence
//!   number (`Replay` for an old one, `OutOfOrder` for a future one), so the
//!   states of the two endpoints never diverge;
//! - the plaintext [`Session`] and the ciphertext [`Session`] (stage 2 in
//!   context: `def` variables, antecedents carried by `/^/`, `/^≈`
//!   synchronisation);
//! - the injective [`SymbolTable`] of the visible antecedents and bound symbols
//!   (G1, option 2): live `def` variables (until `abse ~` or `^^/`), carried
//!   level-0 blocks (until `^/` or `^^/`); anchors are period-local.
//!
//! The sender analyses the plaintext (stage 2), encrypts it, and checks that the
//! ciphertext is stage-2 valid with the same resolutions before committing. The
//! receiver verifies the MAC and the sequence number, analyses the ciphertext
//! (stage 2), decrypts it in textual order and ingests the plaintext. Every
//! failure leaves the session unchanged (both sessions are ingested with
//! [`Session::ingest_checked`], the symbol table is changed only on success).
//!
//! A session models **one ordered stream**: use one session per direction (a
//! session used for both `encrypt` and `decrypt` interleaves both directions in
//! a single half-duplex stream).

use crate::analyze::ClauseId;
use crate::dti::Dti;
use crate::session::{CheckedIngestError, PeriodId, Session};

use super::mac::{self, MacInput};
use super::permutation::Direction;
use super::table::{Changes, Snapshot, SymbolTable, Txn};
use super::transform::{self, apply_edits, parse_single, Keystream, Layout};
use super::walk::{self, PeriodView};
use super::{ensure_available, GhostEnvelope, GhostError, GhostParams, MasterKey, Nonce};

/// Stateful Ghost stream (sender or receiver).
#[derive(Debug, Default)]
pub struct GhostSession {
    params: GhostParams,
    next_sequence: u64,
    plain: Session,
    cipher: Session,
    table: SymbolTable,
}

fn single(ids: &[PeriodId]) -> Result<PeriodId, GhostError> {
    match ids {
        [id] => Ok(*id),
        _ => Err(GhostError::PeriodCount { found: ids.len() }),
    }
}

fn from_ingest(error: CheckedIngestError<GhostError>) -> GhostError {
    match error {
        CheckedIngestError::Grammar(diagnostics) => GhostError::Stage2 { diagnostics },
        CheckedIngestError::Check(errors) => {
            errors.into_iter().next().unwrap_or(GhostError::Internal {
                context: "empty check error",
            })
        }
    }
}

/// The ciphertext must resolve every pair like the plaintext (R13 references,
/// glyph and sema anaphora); the effective numbers may differ when the grammar
/// tiles are permuted.
fn same_resolutions(
    left: &Session,
    left_id: PeriodId,
    right: &Session,
    right_id: PeriodId,
) -> Result<(), GhostError> {
    let mismatch = GhostError::Internal {
        context: "ciphertext resolves differently from the plaintext",
    };
    let (Some(a), Some(b)) = (left.analysis(left_id), right.analysis(right_id)) else {
        return Err(mismatch);
    };
    if left_id != right_id || a.clauses().len() != b.clauses().len() {
        return Err(mismatch);
    }
    for (index, clause) in (0_u32..).zip(a.clauses()) {
        let other = b.clause(ClauseId::new(index)).ok_or(GhostError::Internal {
            context: "ciphertext clause missing",
        })?;
        if clause.depth() != other.depth() || clause.slots().len() != other.slots().len() {
            return Err(mismatch);
        }
        for (x, y) in clause.slots().iter().zip(other.slots()) {
            let same = match (x, y) {
                (None, None) => true,
                (Some(x), Some(y)) => x.resolution() == y.resolution() && x.value() == y.value(),
                _ => false,
            };
            if !same {
                return Err(GhostError::Internal {
                    context: "ciphertext resolves differently from the plaintext",
                });
            }
        }
    }
    Ok(())
}

/// Parsed input of one message.
struct Prepared<'d> {
    snapshot: Snapshot<'d>,
    layout: Layout,
    close: crate::ast::PeriodClose,
}

fn prepare<'d>(text: &str, dti: &'d Dti) -> Result<Prepared<'d>, GhostError> {
    let period = parse_single(text)?;
    let layout = Layout::period(&period, text)?;
    let snapshot = Snapshot {
        dti,
        domain: transform::domain(dti),
    };
    // Pass 1: every atom is a numeral or a valid key (before stage 2).
    for site in &layout.atoms {
        walk::parse_atom(text, site, &snapshot)?;
    }
    Ok(Prepared {
        snapshot,
        layout,
        close: period.close.node,
    })
}

impl GhostSession {
    pub fn new(params: GhostParams) -> Self {
        Self {
            params,
            ..Self::default()
        }
    }

    pub const fn params(&self) -> GhostParams {
        self.params
    }

    /// Sequence number of the next message (sent or expected).
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// Stage-2 state of the plaintext stream.
    pub fn plain_session(&self) -> &Session {
        &self.plain
    }

    /// Stage-2 state of the ciphertext stream.
    pub fn cipher_session(&self) -> &Session {
        &self.cipher
    }

    /// Symbol table of the visible antecedents and bound symbols.
    pub fn symbols(&self) -> &SymbolTable {
        &self.table
    }

    fn keystream<'k>(&self, key: &'k MasterKey, nonce: Nonce, dti: &Dti) -> Keystream<'k> {
        Keystream {
            suite: self.params.suite,
            key,
            nonce,
            snapshot_version: dti.universal().version(),
        }
    }

    /// Encrypts the next period of the stream (one canonical period in indexed
    /// wire form, stage-2 valid in the context of the stream).
    pub fn encrypt(
        &mut self,
        plain: &str,
        dti: &Dti,
        key: &MasterKey,
        nonce: Nonce,
    ) -> Result<GhostEnvelope, GhostError> {
        ensure_available(self.params.suite)?;
        let prepared = prepare(plain, dti)?;
        let keystream = self.keystream(key, nonce, dti);
        let sequence = self.next_sequence;
        let params = self.params;
        let Self {
            plain: plain_session,
            cipher: cipher_session,
            table,
            ..
        } = self;
        let mut produced: Option<(String, Changes)> = None;
        plain_session
            .ingest_checked(plain, |session, ids| {
                encrypt_in_context(
                    plain,
                    session,
                    ids,
                    cipher_session,
                    table,
                    &prepared,
                    &keystream,
                    params,
                )
                .map(|output| produced = Some(output))
                .map_err(|error| vec![error])
            })
            .map_err(from_ingest)?;
        let (ciphertext, changes) = produced.ok_or(GhostError::Internal {
            context: "encryption produced no output",
        })?;
        let mac_tag = mac::compute_tag(
            key,
            &MacInput {
                suite: params.suite,
                snapshot_version: keystream.snapshot_version,
                permute_grammar_tiles: params.permute_grammar_tiles,
                nonce: &nonce,
                sequence,
                ciphertext: &ciphertext,
            },
        )?;
        self.table.commit(changes)?;
        self.next_sequence = sequence.saturating_add(1);
        Ok(GhostEnvelope {
            suite: params.suite,
            snapshot_version: keystream.snapshot_version,
            nonce,
            sequence,
            ciphertext,
            mac_tag,
        })
    }

    /// Checks suite, snapshot version and MAC of an envelope (no state change).
    pub fn verify(
        &self,
        envelope: &GhostEnvelope,
        dti: &Dti,
        key: &MasterKey,
    ) -> Result<(), GhostError> {
        if envelope.suite != self.params.suite {
            return Err(GhostError::SuiteMismatch {
                expected: self.params.suite,
                found: envelope.suite,
            });
        }
        ensure_available(self.params.suite)?;
        let snapshot_version = dti.universal().version();
        if envelope.snapshot_version != snapshot_version {
            return Err(GhostError::SnapshotVersionMismatch {
                expected: snapshot_version,
                found: envelope.snapshot_version,
            });
        }
        let expected_tag = mac::compute_tag(
            key,
            &MacInput {
                suite: self.params.suite,
                snapshot_version,
                permute_grammar_tiles: self.params.permute_grammar_tiles,
                nonce: &envelope.nonce,
                sequence: envelope.sequence,
                ciphertext: &envelope.ciphertext,
            },
        )?;
        if mac::tags_equal(&expected_tag, &envelope.mac_tag) {
            Ok(())
        } else {
            Err(GhostError::MacMismatch)
        }
    }

    /// Verifies and decrypts the next envelope of the stream.
    ///
    /// Order: suite → snapshot version → MAC → sequence number → stage 2 on
    /// the ciphertext → decryption → stage 2 on the plaintext → commit.
    pub fn decrypt(
        &mut self,
        envelope: &GhostEnvelope,
        dti: &Dti,
        key: &MasterKey,
    ) -> Result<String, GhostError> {
        self.verify(envelope, dti, key)?;
        let expected = self.next_sequence;
        if envelope.sequence < expected {
            return Err(GhostError::Replay);
        }
        if envelope.sequence > expected {
            return Err(GhostError::OutOfOrder {
                expected,
                found: envelope.sequence,
            });
        }
        let ciphertext = envelope.ciphertext.as_str();
        let prepared = prepare(ciphertext, dti)?;
        let keystream = self.keystream(key, envelope.nonce, dti);
        let params = self.params;
        let Self {
            plain: plain_session,
            cipher: cipher_session,
            table,
            ..
        } = self;
        let mut produced: Option<(String, Changes)> = None;
        cipher_session
            .ingest_checked(ciphertext, |session, ids| {
                decrypt_in_context(
                    ciphertext,
                    session,
                    ids,
                    plain_session,
                    table,
                    &prepared,
                    &keystream,
                    params,
                )
                .map(|output| produced = Some(output))
                .map_err(|error| vec![error])
            })
            .map_err(from_ingest)?;
        let (plaintext, changes) = produced.ok_or(GhostError::Internal {
            context: "decryption produced no output",
        })?;
        self.table.commit(changes)?;
        self.next_sequence = expected.saturating_add(1);
        Ok(plaintext)
    }
}

/// Runs the walk on a period already analysed in `analysis`.
#[allow(clippy::too_many_arguments)]
fn run_walk(
    text: &str,
    analysis: &Session,
    period: PeriodId,
    plain: &Session,
    table: &SymbolTable,
    prepared: &Prepared<'_>,
    keystream: &Keystream<'_>,
    params: GhostParams,
    direction: Direction,
) -> Result<(String, Changes), GhostError> {
    let view = PeriodView {
        text,
        layout: &prepared.layout,
        analysis,
        period,
        plain,
        close: prepared.close,
    };
    walk::check_ephemeral(&view, &prepared.snapshot)?;
    let mut txn = Txn::new(table);
    let mut edits = walk::run(&view, &prepared.snapshot, keystream, &mut txn, direction)?;
    if params.permute_grammar_tiles {
        prepared
            .layout
            .grammar_edits(keystream, direction, &mut edits)?;
    }
    let output = apply_edits(text, edits)?;
    // Self-check: the output must be a single stage-1 valid period.
    parse_single(&output).map_err(|_| GhostError::Internal {
        context: "output is not a valid period",
    })?;
    Ok((output, txn.into_changes()))
}

#[allow(clippy::too_many_arguments)]
fn encrypt_in_context(
    plain: &str,
    plain_session: &Session,
    ids: &[PeriodId],
    cipher_session: &mut Session,
    table: &SymbolTable,
    prepared: &Prepared<'_>,
    keystream: &Keystream<'_>,
    params: GhostParams,
) -> Result<(String, Changes), GhostError> {
    let period = single(ids)?;
    let (ciphertext, changes) = run_walk(
        plain,
        plain_session,
        period,
        plain_session,
        table,
        prepared,
        keystream,
        params,
        Direction::Forward,
    )?;
    // The ciphertext must be stage-2 valid in the ciphertext stream, with the
    // same resolutions as the plaintext.
    cipher_session
        .ingest_checked(&ciphertext, |session, cipher_ids| {
            single(cipher_ids)
                .and_then(|cipher| same_resolutions(plain_session, period, session, cipher))
                .map_err(|error| vec![error])
        })
        .map_err(|error| match from_ingest(error) {
            GhostError::Stage2 { .. } => GhostError::Internal {
                context: "ciphertext is not stage-2 valid",
            },
            other => other,
        })?;
    Ok((ciphertext, changes))
}

#[allow(clippy::too_many_arguments)]
fn decrypt_in_context(
    ciphertext: &str,
    cipher_session: &Session,
    ids: &[PeriodId],
    plain_session: &mut Session,
    table: &SymbolTable,
    prepared: &Prepared<'_>,
    keystream: &Keystream<'_>,
    params: GhostParams,
) -> Result<(String, Changes), GhostError> {
    let period = single(ids)?;
    let (plaintext, changes) = run_walk(
        ciphertext,
        cipher_session,
        period,
        plain_session,
        table,
        prepared,
        keystream,
        params,
        Direction::Inverse,
    )?;
    plain_session
        .ingest_checked(&plaintext, |session, plain_ids| {
            single(plain_ids)
                .and_then(|plain| same_resolutions(session, plain, cipher_session, period))
                .map_err(|error| vec![error])
        })
        .map_err(|error| match from_ingest(error) {
            GhostError::Stage2 { .. } => GhostError::Internal {
                context: "decrypted period is not stage-2 valid",
            },
            other => other,
        })?;
    Ok((plaintext, changes))
}
