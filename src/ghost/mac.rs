//! G3/G6: message authentication and replay protection.

use std::collections::{HashSet, VecDeque};
use std::num::NonZeroUsize;

use super::primitives::{absorb_field, keyed_xof, CryptoSuite};
use super::{GhostError, MasterKey, Nonce};

/// Domain-separation label of the MAC.
pub const MAC_LABEL: &[u8] = b"ApriWave-Ghost-v2/mac";

/// Length of the MAC tag in bytes.
pub const MAC_LENGTH: usize = 32;

/// Fields authenticated by the MAC.
pub struct MacInput<'a> {
    pub suite: CryptoSuite,
    pub snapshot_version: u64,
    pub permute_grammar_tiles: bool,
    pub nonce: &'a Nonce,
    /// Position of the message in its [`super::GhostSession`] stream.
    pub sequence: u64,
    pub ciphertext: &'a str,
}

/// `tag = PRF_key(field(label) || field(suite) || field(version) || field(flags)
/// || field(nonce) || field(sequence) || field(ciphertext))`, truncated to 32
/// bytes (`sequence` as u64 LE).
///
/// The PRF is the keyed XOF of the suite (keyed SHAKE256 by default, BLAKE3
/// keyed mode with `ghost-blake3`); every field is length-prefixed, and neither a
/// sponge nor BLAKE3 is subject to length extension.
pub fn compute_tag(key: &MasterKey, input: &MacInput<'_>) -> Result<[u8; MAC_LENGTH], GhostError> {
    let mut xof = keyed_xof(input.suite, MAC_LABEL, key)?;
    absorb_field(xof.as_mut(), &[input.suite.id()]);
    absorb_field(xof.as_mut(), &input.snapshot_version.to_le_bytes());
    absorb_field(xof.as_mut(), &[u8::from(input.permute_grammar_tiles)]);
    absorb_field(xof.as_mut(), &input.nonce.0);
    absorb_field(xof.as_mut(), &input.sequence.to_le_bytes());
    absorb_field(xof.as_mut(), input.ciphertext.as_bytes());
    let mut tag = [0_u8; MAC_LENGTH];
    xof.finalize_xof().squeeze(&mut tag);
    Ok(tag)
}

/// Constant-time equality of two tags: every byte is examined (no early exit).
pub fn tags_equal(left: &[u8; MAC_LENGTH], right: &[u8; MAC_LENGTH]) -> bool {
    let difference = left
        .iter()
        .zip(right.iter())
        .fold(0_u8, |acc, (a, b)| acc | (a ^ b));
    std::hint::black_box(difference) == 0
}

/// Bounded set of recently accepted nonces (FIFO eviction).
///
/// Only the last `capacity` nonces are remembered: a nonce evicted from the
/// window would be accepted again, so the capacity must cover the replay horizon
/// of the application (or nonces must be monotonic and checked by the caller).
#[derive(Debug)]
pub struct ReplayWindow {
    capacity: NonZeroUsize,
    order: VecDeque<Nonce>,
    seen: HashSet<Nonce>,
}

impl ReplayWindow {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity.get()),
            seen: HashSet::with_capacity(capacity.get()),
        }
    }

    pub const fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// `true` if `nonce` is in the window.
    pub fn contains(&self, nonce: &Nonce) -> bool {
        self.seen.contains(nonce)
    }

    /// Records `nonce`, evicting the oldest entry if the window is full.
    /// Returns `false` if the nonce was already present.
    pub fn record(&mut self, nonce: Nonce) -> bool {
        if !self.seen.insert(nonce) {
            return false;
        }
        self.order.push_back(nonce);
        while self.order.len() > self.capacity.get() {
            match self.order.pop_front() {
                Some(oldest) => {
                    self.seen.remove(&oldest);
                }
                None => break,
            }
        }
        true
    }
}
