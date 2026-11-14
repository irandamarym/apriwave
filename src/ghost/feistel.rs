//! Ephemeral keys (rule 5): keyed Feistel permutation of the key space minus
//! the universal snapshot.
//!
//! Bound symbols that are not universal concepts (ephemeral session keys and
//! local `new` keys, the set `K \ S`) cannot be permuted by Fisher–Yates over the
//! snapshot domain. They are encrypted with a deterministic Feistel network over
//! the **31 body characters** of the key; the 32nd (Luhn mod 36 check) character
//! is recomputed with [`DtiKey::from_random_body`], so every image is a valid key.
//!
//! # Construction
//!
//! - **Encoding.** The body is a mixed-radix digit vector: `d0 ∈ Z_26` (the
//!   first character, `[a-z]`), `d1..d30 ∈ Z_36` (`0-9` → 0–9, `a-z` → 10–35).
//!   The left half `L = d0·36^15 + (d1..d15)_36` lies in `[0, 26·36^15)`
//!   (`< 2^83`), the right half `R = (d16..d30)_36` in `[0, 36^15)` (`< 2^78`):
//!   both fit a `u128`, no big-number arithmetic is needed. The encoding is a
//!   bijection between bodies and `Z_{26·36^15} × Z_{36^15}` (≈ 2^160 elements).
//! - **Rounds.** [`FEISTEL_ROUNDS`] = 10 alternating (unbalanced) Feistel rounds,
//!   modular addition in each half's own ring: even rounds `L ← (L + F_i(R)) mod
//!   |L|`, odd rounds `R ← (R + F_i(L)) mod |R|`. Each round is invertible
//!   (subtraction), so the network is a permutation of the body space; decryption
//!   runs the rounds backwards.
//! - **Round function.** `F_i(x) = u128_LE(XOF(field(keystream label) || master
//!   key || nonce || version || field("ephemeral-key") || P || field(i || x_LE)))
//!   mod m`, i.e. the suite keystream of the token position `P` in class
//!   [`CLASS_EPHEMERAL`] with the round index and the half as extra input. The
//!   modular reduction of a 128-bit value has a bias below `2^-45`.
//! - **Cycle walking.** The Feistel image may be a universal key. The image is
//!   re-encrypted until it falls outside `S` (`τ(x) = F^k(x)` for the smallest
//!   `k ≥ 1` with `F^k(x) ∉ S`); decryption walks backwards until it leaves `S`.
//!   Cycle walking a permutation restricted to `K \ S` is a permutation of
//!   `K \ S`. Collisions with other active bound symbols are excluded by the
//!   chain walk of the `bijection` module, which uses this `τ` as base
//!   permutation.

use crate::dti::DtiKey;

use super::permutation::Direction;
use super::transform::Keystream;
use super::GhostError;

/// Class label of ephemeral keys (keys outside the universal snapshot).
pub const CLASS_EPHEMERAL: &[u8] = b"ephemeral-key";
/// Number of Feistel rounds (5 per half).
pub const FEISTEL_ROUNDS: u8 = 10;
/// Maximum number of cycle-walking steps (each step lands in the snapshot with
/// probability `|S| / 2^160`).
pub const MAX_CYCLE_WALK: u32 = 64;

const BODY: usize = crate::dti::KEY_BODY_LENGTH;
const RIGHT_DIGITS: u32 = 15;

/// `36^15`.
const RIGHT_MODULUS: u128 = 36_u128.pow(RIGHT_DIGITS);
/// `26 · 36^15`.
const LEFT_MODULUS: u128 = 26 * RIGHT_MODULUS;

fn radix36(byte: u8) -> Option<u128> {
    match byte {
        b'0'..=b'9' => Some(u128::from(byte - b'0')),
        b'a'..=b'z' => Some(u128::from(byte - b'a') + 10),
        _ => None,
    }
}

fn radix36_char(value: u128) -> Option<u8> {
    let value = u8::try_from(value).ok()?;
    match value {
        0..=9 => Some(b'0' + value),
        10..=35 => Some(b'a' + (value - 10)),
        _ => None,
    }
}

const fn encoding_error() -> GhostError {
    GhostError::Internal {
        context: "key body outside the Feistel encoding",
    }
}

/// Mixed-radix encoding of a body into `(L, R)`.
fn encode(body: &[u8; BODY]) -> Result<(u128, u128), GhostError> {
    let mut bytes = body.iter();
    let first = bytes.next().ok_or_else(encoding_error)?;
    let mut left = match first {
        b'a'..=b'z' => u128::from(first - b'a'),
        _ => return Err(encoding_error()),
    };
    for _ in 0..RIGHT_DIGITS {
        let digit = bytes
            .next()
            .and_then(|b| radix36(*b))
            .ok_or_else(encoding_error)?;
        left = left * 36 + digit;
    }
    let mut right = 0_u128;
    for byte in bytes {
        right = right * 36 + radix36(*byte).ok_or_else(encoding_error)?;
    }
    Ok((left, right))
}

/// Inverse of [`encode`].
fn decode(left: u128, right: u128) -> Result<[u8; BODY], GhostError> {
    let mut body = [0_u8; BODY];
    let (head, tail) = body.split_at_mut(BODY - RIGHT_DIGITS as usize);
    let mut value = right;
    for slot in tail.iter_mut().rev() {
        *slot = radix36_char(value % 36).ok_or_else(encoding_error)?;
        value /= 36;
    }
    let mut value = left;
    let (first, middle) = head.split_first_mut().ok_or_else(encoding_error)?;
    for slot in middle.iter_mut().rev() {
        *slot = radix36_char(value % 36).ok_or_else(encoding_error)?;
        value /= 36;
    }
    let letter = u8::try_from(value)
        .ok()
        .filter(|v| *v < 26)
        .ok_or_else(encoding_error)?;
    *first = b'a' + letter;
    Ok(body)
}

fn body_of(key: &DtiKey) -> Result<[u8; BODY], GhostError> {
    key.as_str()
        .as_bytes()
        .get(..BODY)
        .and_then(|bytes| <[u8; BODY]>::try_from(bytes).ok())
        .ok_or_else(encoding_error)
}

/// Feistel network of one token position.
pub struct Feistel<'a, 'k> {
    keystream: &'a Keystream<'k>,
    position: u64,
}

impl<'a, 'k> Feistel<'a, 'k> {
    pub const fn new(keystream: &'a Keystream<'k>, position: u64) -> Self {
        Self {
            keystream,
            position,
        }
    }

    fn round(&self, round: u8, input: u128, modulus: u128) -> Result<u128, GhostError> {
        let mut extra = [0_u8; 17];
        let (index, half) = extra.split_at_mut(1);
        index.fill(round);
        half.copy_from_slice(&input.to_le_bytes());
        let mut reader = self.keystream.prf(CLASS_EPHEMERAL, self.position, &extra)?;
        let mut out = [0_u8; 16];
        reader.squeeze(&mut out);
        Ok(u128::from_le_bytes(out) % modulus)
    }

    /// One application of the Feistel permutation to a key (check character
    /// recomputed).
    pub fn apply(&self, key: &DtiKey, direction: Direction) -> Result<DtiKey, GhostError> {
        let (mut left, mut right) = encode(&body_of(key)?)?;
        let rounds: Vec<u8> = match direction {
            Direction::Forward => (0..FEISTEL_ROUNDS).collect(),
            Direction::Inverse => (0..FEISTEL_ROUNDS).rev().collect(),
        };
        for round in rounds {
            if round % 2 == 0 {
                let f = self.round(round, right, LEFT_MODULUS)?;
                left = match direction {
                    Direction::Forward => (left + f) % LEFT_MODULUS,
                    Direction::Inverse => (left + LEFT_MODULUS - f) % LEFT_MODULUS,
                };
            } else {
                let f = self.round(round, left, RIGHT_MODULUS)?;
                right = match direction {
                    Direction::Forward => (right + f) % RIGHT_MODULUS,
                    Direction::Inverse => (right + RIGHT_MODULUS - f) % RIGHT_MODULUS,
                };
            }
        }
        DtiKey::from_random_body(decode(left, right)?).map_err(|_| encoding_error())
    }

    /// Cycle-walked permutation `τ` of `K \ S` (`in_snapshot` tells `S`).
    pub fn permute(
        &self,
        key: &DtiKey,
        direction: Direction,
        in_snapshot: &impl Fn(&DtiKey) -> bool,
    ) -> Result<DtiKey, GhostError> {
        let mut current = self.apply(key, direction)?;
        for _ in 0..MAX_CYCLE_WALK {
            if !in_snapshot(&current) {
                return Ok(current);
            }
            current = self.apply(&current, direction)?;
        }
        Err(GhostError::KeystreamExhausted)
    }
}

#[cfg(test)]
pub(super) fn encoding_round_trip(key: &DtiKey) -> Result<bool, GhostError> {
    let body = body_of(key)?;
    let (left, right) = encode(&body)?;
    Ok(left < LEFT_MODULUS && right < RIGHT_MODULUS && decode(left, right)? == body)
}
