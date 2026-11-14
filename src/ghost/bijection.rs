//! Binding-consistent bijections ("chain walking").
//!
//! At every position Ghost maps an atom with a bijection `σ` that must agree
//! with a small set of active bindings `B = {(p_j, c_j)}` (plaintext atom ↔
//! ciphertext atom, see the `walk` module): `σ(p_j) = c_j` for every binding,
//! and every other plaintext atom is mapped **outside** `{c_j}`. Given a base
//! permutation `τ` of the class (Fisher–Yates over the snapshot domain,
//! digit-wise permutation of numerals, cycle-walked Feistel of ephemeral keys)
//! `σ` is defined by chain walking:
//!
//! ```text
//! σ(x)  = c_j                          if x = p_j
//! σ(x)  = y_t  where y_0 = τ(x), y_{k+1} = τ(p_j) while y_k = c_j
//! σ⁻¹(y) = p_j                         if y = c_j
//! σ⁻¹(y) = x_t where x_0 = τ⁻¹(y), x_{k+1} = τ⁻¹(c_j) while x_k = p_j
//! ```
//!
//! **Bijectivity.** The forward chain visits pairwise distinct bindings: if it
//! returned to a cipher `c_j` already visited, injectivity of `τ` would force the
//! starting atom `x` to equal a bound plaintext, which is excluded. Hence the
//! walk stops after at most `|B| + 1` applications of `τ` on an atom outside
//! `{c_j}`. The inverse walk retraces the same chain backwards (`τ⁻¹(τ(p_j)) =
//! p_j`, `τ⁻¹(c_j) = τ⁻¹(τ(p_{j'}))` …) and ends on the starting atom, so `σ⁻¹ ∘ σ`
//! is the identity on the unbound atoms and `σ` is a bijection from the
//! unbound plaintext atoms onto the unbound ciphertext atoms. With `B = ∅`,
//! `σ = τ`.
//!
//! Both sides compute `B` from the already processed prefix only, so encryption
//! and decryption use the same `σ`.

use std::borrow::Borrow;

use super::GhostError;

/// Image of a chain walk.
#[derive(Debug, PartialEq, Eq)]
pub enum Image<T> {
    /// The input is the bound side of binding `j`: the image is the other side.
    Bound(usize),
    /// The input is unbound: the image (outside every bound value).
    Free(T),
}

/// `σ(x)` (forward) for bindings `(plain, cipher)` and base permutation `tau`.
pub fn forward<T, U>(
    x: &U,
    bindings: &[(&U, &U)],
    mut tau: impl FnMut(&U) -> Result<T, GhostError>,
) -> Result<Image<T>, GhostError>
where
    T: Borrow<U>,
    U: PartialEq + ?Sized,
{
    walk(x, bindings, &mut tau, false)
}

/// `σ⁻¹(y)` (inverse) for bindings `(plain, cipher)` and inverse base
/// permutation `tau_inverse`.
pub fn inverse<T, U>(
    y: &U,
    bindings: &[(&U, &U)],
    mut tau_inverse: impl FnMut(&U) -> Result<T, GhostError>,
) -> Result<Image<T>, GhostError>
where
    T: Borrow<U>,
    U: PartialEq + ?Sized,
{
    walk(y, bindings, &mut tau_inverse, true)
}

/// Plain side (`plain = true`) or cipher side of a binding.
fn side<'a, U: ?Sized>(pair: &(&'a U, &'a U), plain: bool) -> &'a U {
    if plain {
        pair.0
    } else {
        pair.1
    }
}

/// Shared walk: `swap = false` follows `plain → cipher`, `swap = true` follows
/// `cipher → plain` (the roles of the two sides of every binding exchange).
fn walk<T, U>(
    start: &U,
    bindings: &[(&U, &U)],
    step: &mut impl FnMut(&U) -> Result<T, GhostError>,
    swap: bool,
) -> Result<Image<T>, GhostError>
where
    T: Borrow<U>,
    U: PartialEq + ?Sized,
{
    if let Some(index) = bindings.iter().position(|pair| side(pair, !swap) == start) {
        return Ok(Image::Bound(index));
    }
    let mut current = step(start)?;
    // At most |B| + 1 applications of the base permutation (see module docs).
    for _ in 0..=bindings.len() {
        let hit = bindings
            .iter()
            .position(|pair| side(pair, swap) == current.borrow());
        match hit.and_then(|index| bindings.get(index)) {
            None => return Ok(Image::Free(current)),
            Some(pair) => current = step(side(pair, !swap))?,
        }
    }
    Err(GhostError::Internal {
        context: "chain walk did not terminate",
    })
}
