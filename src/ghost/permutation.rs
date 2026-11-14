//! G2: keyed permutations of finite domains.
//!
//! Each permuted token gets its own Fisher–Yates permutation of the whole
//! domain, driven by a keystream. Indices are drawn with **unbiased rejection
//! sampling**: a `u64` draw is rejected when it is ≥ the largest multiple of the
//! bound representable in 2^64, so every accepted value is uniform modulo the
//! bound. Building a permutation costs O(N) time and memory per token, which is
//! accepted by design (B4/G2).

use super::primitives::{next_u64, XofReader};
use super::GhostError;

/// Maximum number of rejected draws before giving up. Each draw is rejected
/// with probability < 1/2, so reaching this bound with a sound XOF has
/// probability < 2^-128; it only guards against degenerate streams.
pub const MAX_REJECTIONS: u32 = 128;

/// Uniform index in `0..bound` (`bound ≥ 1`) by rejection sampling.
pub fn uniform_index(reader: &mut dyn XofReader, bound: usize) -> Result<usize, GhostError> {
    let bound_u64 = u64::try_from(bound).map_err(|_| GhostError::KeystreamExhausted)?;
    if bound_u64 == 0 {
        return Err(GhostError::KeystreamExhausted);
    }
    // 2^64 mod bound; the acceptance zone is [0, 2^64 - remainder).
    let remainder = (u64::MAX % bound_u64).wrapping_add(1) % bound_u64;
    let limit = 0_u64.wrapping_sub(remainder);
    for _ in 0..MAX_REJECTIONS {
        let draw = next_u64(reader);
        if remainder == 0 || draw < limit {
            return usize::try_from(draw % bound_u64).map_err(|_| GhostError::KeystreamExhausted);
        }
    }
    Err(GhostError::KeystreamExhausted)
}

/// Fisher–Yates permutation of `0..size`: `result[i]` is the image of `i`.
pub fn permutation(reader: &mut dyn XofReader, size: usize) -> Result<Vec<usize>, GhostError> {
    let mut images: Vec<usize> = (0..size).collect();
    for upper in (1..size).rev() {
        let bound = upper.checked_add(1).ok_or(GhostError::KeystreamExhausted)?;
        let other = uniform_index(reader, bound)?;
        // `other <= upper < size`: `swap` cannot go out of bounds.
        images.swap(upper, other);
    }
    Ok(images)
}

/// Direction of the mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `π(index)`
    Forward,
    /// `π⁻¹(index)`
    Inverse,
}

/// Applies `π` or `π⁻¹` to `index`; `None` if `index` is out of range.
pub fn apply(images: &[usize], index: usize, direction: Direction) -> Option<usize> {
    match direction {
        Direction::Forward => images.get(index).copied(),
        Direction::Inverse => images.iter().position(|image| *image == index),
    }
}
