//! Internal, zero-dependency SHAKE256 (FIPS 202).
//!
//! Keccak-f\[1600\] with 24 rounds, sponge rate 136 bytes (capacity 512 bits),
//! domain padding `0x1F … 0x80`. The implementation contains no slice indexing:
//! lanes are addressed through iterators, `as_chunks` and array rotations.
//!
//! A sponge is not subject to length-extension attacks, so `SHAKE256(key || msg)`
//! (with length-prefixed fields, see `primitives`) is a sound MAC/PRF.

use super::primitives::{Xof, XofReader};

/// Sponge rate of SHAKE256 in bytes (1600 - 2 * 256 bits).
pub const SHAKE256_RATE: usize = 136;

/// SHAKE domain-separation suffix (`1111` followed by the first padding bit).
const DOMAIN_PAD: u8 = 0x1F;
/// Last padding bit.
const FINAL_PAD: u8 = 0x80;

const ROUND_CONSTANTS: [u64; 24] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];

/// Lane visiting order of the combined ρ+π step (lane index `x + 5y`).
const PI_LANES: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

/// ρ rotation offsets matching `PI_LANES`.
const RHO_OFFSETS: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];

type State = [u64; 25];

/// Keccak-f\[1600\] permutation (24 rounds). Lane `(x, y)` is `state[x + 5y]`.
fn keccak_f1600(state: &mut State) {
    for round_constant in ROUND_CONSTANTS {
        // θ: column parities.
        let mut parity = [0_u64; 5];
        for row in state.as_chunks::<5>().0 {
            for (column, lane) in parity.iter_mut().zip(row) {
                *column ^= *lane;
            }
        }
        // D[x] = C[x - 1] ^ rot(C[x + 1], 1)
        let mut previous = parity;
        previous.rotate_right(1);
        let mut next = parity;
        next.rotate_left(1);
        let mut effect = [0_u64; 5];
        for ((d, p), n) in effect.iter_mut().zip(previous).zip(next) {
            *d = p ^ n.rotate_left(1);
        }
        for row in state.as_chunks_mut::<5>().0 {
            for (lane, d) in row.iter_mut().zip(effect) {
                *lane ^= d;
            }
        }

        // ρ and π: walk the single 24-lane cycle of π, rotating as we go.
        let mut carried = state.get(1).copied().unwrap_or(0);
        for (lane_index, offset) in PI_LANES.iter().zip(RHO_OFFSETS) {
            if let Some(lane) = state.get_mut(*lane_index) {
                let displaced = *lane;
                *lane = carried.rotate_left(offset);
                carried = displaced;
            }
        }

        // χ: row-wise non-linear step.
        for row in state.as_chunks_mut::<5>().0 {
            let original = *row;
            let mut plus_one = original;
            plus_one.rotate_left(1);
            let mut plus_two = original;
            plus_two.rotate_left(2);
            for (((lane, a), b), c) in row.iter_mut().zip(original).zip(plus_one).zip(plus_two) {
                *lane = a ^ (!b & c);
            }
        }

        // ι
        if let Some(first) = state.first_mut() {
            *first ^= round_constant;
        }
    }
}

/// XORs one full rate block (little-endian lanes) into the state.
fn absorb_block(state: &mut State, block: &[u8; SHAKE256_RATE]) {
    for (lane, chunk) in state.iter_mut().zip(block.as_chunks::<8>().0) {
        *lane ^= u64::from_le_bytes(*chunk);
    }
}

/// Serializes the rate part of the state (little-endian lanes).
fn squeeze_block(state: &State, out: &mut [u8; SHAKE256_RATE]) {
    for (chunk, lane) in out.as_chunks_mut::<8>().0.iter_mut().zip(state.iter()) {
        *chunk = lane.to_le_bytes();
    }
}

/// Best-effort wipe (see the note on `MasterKey`: without `unsafe`/volatile
/// writes the compiler may elide it).
fn wipe(state: &mut State, buffer: &mut [u8; SHAKE256_RATE]) {
    state.iter_mut().for_each(|lane| *lane = 0);
    buffer.iter_mut().for_each(|byte| *byte = 0);
    std::hint::black_box((state, buffer));
}

/// SHAKE256 in the absorbing phase.
///
/// ```
/// use apriwave::ghost::Shake256;
/// let mut out = [0_u8; 32];
/// Shake256::digest(b"abc", &mut out);
/// assert_eq!(out.first(), Some(&0x48));
/// ```
pub struct Shake256 {
    state: State,
    buffer: [u8; SHAKE256_RATE],
    filled: usize,
}

impl Default for Shake256 {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Shake256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The state may contain key material.
        f.write_str("Shake256 { .. }")
    }
}

impl Shake256 {
    pub const fn new() -> Self {
        Self {
            state: [0; 25],
            buffer: [0; SHAKE256_RATE],
            filled: 0,
        }
    }

    /// Absorbs `data`; may be called any number of times with chunks of any size.
    pub fn absorb(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            let Some(space) = self.buffer.get_mut(self.filled..) else {
                return;
            };
            let take = space.len().min(data.len());
            let (head, rest) = data.split_at(take);
            if let Some(target) = space.get_mut(..take) {
                target.copy_from_slice(head);
            }
            self.filled = self.filled.saturating_add(take);
            data = rest;
            if self.filled == SHAKE256_RATE {
                absorb_block(&mut self.state, &self.buffer);
                keccak_f1600(&mut self.state);
                self.buffer = [0; SHAKE256_RATE];
                self.filled = 0;
            }
        }
    }

    /// Pads, switches to the squeezing phase and returns the reader.
    pub fn finalize(mut self) -> Shake256Reader {
        // `buffer` beyond `filled` is zero by construction.
        if let Some(byte) = self.buffer.get_mut(self.filled) {
            *byte ^= DOMAIN_PAD;
        }
        if let Some(byte) = self.buffer.last_mut() {
            *byte ^= FINAL_PAD;
        }
        absorb_block(&mut self.state, &self.buffer);
        keccak_f1600(&mut self.state);
        let mut reader = Shake256Reader {
            state: self.state,
            block: [0; SHAKE256_RATE],
            position: 0,
        };
        squeeze_block(&reader.state, &mut reader.block);
        reader
    }

    /// One-shot helper: `out = SHAKE256(data)` truncated to `out.len()` bytes.
    pub fn digest(data: &[u8], out: &mut [u8]) {
        let mut xof = Self::new();
        xof.absorb(data);
        xof.finalize().squeeze(out);
    }
}

impl Drop for Shake256 {
    fn drop(&mut self) {
        wipe(&mut self.state, &mut self.buffer);
    }
}

impl Xof for Shake256 {
    fn update(&mut self, data: &[u8]) {
        self.absorb(data);
    }

    fn finalize_xof(self: Box<Self>) -> Box<dyn XofReader> {
        Box::new((*self).finalize())
    }
}

/// SHAKE256 in the squeezing phase.
pub struct Shake256Reader {
    state: State,
    block: [u8; SHAKE256_RATE],
    position: usize,
}

impl std::fmt::Debug for Shake256Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Shake256Reader { .. }")
    }
}

impl Shake256Reader {
    /// Fills `out` with the next output bytes; may be called repeatedly.
    pub fn squeeze(&mut self, out: &mut [u8]) {
        let mut out = out;
        while !out.is_empty() {
            if self.position >= SHAKE256_RATE {
                keccak_f1600(&mut self.state);
                squeeze_block(&self.state, &mut self.block);
                self.position = 0;
            }
            let Some(available) = self.block.get(self.position..) else {
                return;
            };
            let take = available.len().min(out.len());
            let (head, rest) = out.split_at_mut(take);
            if let Some(source) = available.get(..take) {
                head.copy_from_slice(source);
            }
            self.position = self.position.saturating_add(take);
            out = rest;
        }
    }
}

impl Drop for Shake256Reader {
    fn drop(&mut self) {
        wipe(&mut self.state, &mut self.block);
    }
}

impl XofReader for Shake256Reader {
    fn squeeze(&mut self, out: &mut [u8]) {
        Shake256Reader::squeeze(self, out);
    }
}
