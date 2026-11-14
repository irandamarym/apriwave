//! G7: key derivation.
//!
//! - [`Argon2idKdf`]: password hashing (feature `ghost-argon2`), defaults to the
//!   RFC 9106 *second recommended* option.
//! - [`Shake256KeyDerivation`]: internal, dependency-free derivation for
//!   **high-entropy secrets only**.

use super::keccak::Shake256;
use super::primitives::absorb_field;
use super::GhostError;

/// Key-derivation function (object safe).
pub trait Kdf {
    /// Derives `out.len()` bytes from `secret` and `salt`.
    fn derive(&self, secret: &[u8], salt: &[u8], out: &mut [u8]) -> Result<(), GhostError>;
}

/// Domain-separation label of [`Shake256KeyDerivation`].
const SHAKE_KDF_LABEL: &[u8] = b"ApriWave-Ghost-v2/kdf-shake256";

/// `SHAKE256(field(label) || field(secret) || field(salt))`.
///
/// **NOT a password hash**: it is fast and has no memory hardness, so it must
/// only be used with secrets that already carry at least 128 bits of entropy
/// (e.g. a random key exchanged out of band). For passphrases use
/// [`Argon2idKdf`]. It exists so that the default build (no features) can derive
/// session keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shake256KeyDerivation;

impl Kdf for Shake256KeyDerivation {
    fn derive(&self, secret: &[u8], salt: &[u8], out: &mut [u8]) -> Result<(), GhostError> {
        let mut xof = Shake256::new();
        absorb_field(&mut xof, SHAKE_KDF_LABEL);
        absorb_field(&mut xof, secret);
        absorb_field(&mut xof, salt);
        xof.finalize().squeeze(out);
        Ok(())
    }
}

/// Argon2id (RFC 9106) adapter. Requires the `ghost-argon2` feature; without it
/// [`Kdf::derive`] returns [`GhostError::KdfUnavailable`].
///
/// The salt must be at least [`Argon2idKdf::SALT_LENGTH`] bytes (RFC 9106
/// recommends 16 random bytes); the recommended tag length is
/// [`Argon2idKdf::TAG_LENGTH`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2idKdf {
    t_cost: u32,
    p_cost: u32,
    m_cost_kib: u32,
}

impl Default for Argon2idKdf {
    /// RFC 9106 second recommended option.
    fn default() -> Self {
        Self::rfc9106_second_recommended()
    }
}

impl Argon2idKdf {
    /// Recommended salt length (bytes); shorter salts are rejected.
    pub const SALT_LENGTH: usize = 16;
    /// Recommended tag length (bytes).
    pub const TAG_LENGTH: usize = 32;

    /// RFC 9106 §4, first recommended option: t=1, p=4, m=2 GiB.
    pub const fn rfc9106_first_recommended() -> Self {
        Self {
            t_cost: 1,
            p_cost: 4,
            m_cost_kib: 2 * 1024 * 1024,
        }
    }

    /// RFC 9106 §4, second recommended option: t=3, p=4, m=64 MiB.
    pub const fn rfc9106_second_recommended() -> Self {
        Self {
            t_cost: 3,
            p_cost: 4,
            m_cost_kib: 64 * 1024,
        }
    }

    /// Custom parameters (validated when deriving).
    pub const fn with_params(t_cost: u32, p_cost: u32, m_cost_kib: u32) -> Self {
        Self {
            t_cost,
            p_cost,
            m_cost_kib,
        }
    }

    pub const fn t_cost(&self) -> u32 {
        self.t_cost
    }

    pub const fn p_cost(&self) -> u32 {
        self.p_cost
    }

    pub const fn m_cost_kib(&self) -> u32 {
        self.m_cost_kib
    }
}

impl Kdf for Argon2idKdf {
    #[cfg(feature = "ghost-argon2")]
    fn derive(&self, secret: &[u8], salt: &[u8], out: &mut [u8]) -> Result<(), GhostError> {
        use argon2::{Algorithm, Argon2, Params, Version};

        if salt.len() < Self::SALT_LENGTH {
            return Err(GhostError::SaltTooShort {
                minimum: Self::SALT_LENGTH,
                found: salt.len(),
            });
        }
        let params = Params::new(self.m_cost_kib, self.t_cost, self.p_cost, Some(out.len()))
            .map_err(|error| GhostError::Kdf {
                reason: error.to_string(),
            })?;
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(secret, salt, out)
            .map_err(|error| GhostError::Kdf {
                reason: error.to_string(),
            })
    }

    #[cfg(not(feature = "ghost-argon2"))]
    fn derive(&self, _secret: &[u8], _salt: &[u8], _out: &mut [u8]) -> Result<(), GhostError> {
        Err(GhostError::KdfUnavailable)
    }
}
