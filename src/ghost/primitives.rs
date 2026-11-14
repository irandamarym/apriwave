//! G1: primitive traits, crypto suites and suite adapters.
//!
//! Every suite is exposed through the same object-safe pair of traits:
//! [`Xof`] (absorbing phase) and [`XofReader`] (squeezing phase). The type-state
//! split makes "absorb after squeeze" unrepresentable.
//!
//! | Suite      | Feature        | Construction                                        |
//! |------------|----------------|-----------------------------------------------------|
//! | `Shake256` | (always)       | internal FIPS 202 SHAKE256                          |
//! | `Blake3`   | `ghost-blake3` | BLAKE3 XOF; keyed mode for keyed use                |
//! | `Ascon`    | `ghost-ascon`  | Ascon-XOF128 (`ascon-hash`)                         |
//! | `Skein512` | `ghost-skein`  | Skein-512-512 in counter mode (see [`CryptoSuite`]) |

use std::fmt;

use super::keccak::Shake256;
use super::GhostError;
use super::MasterKey;

/// Extendable-output function in the absorbing phase (object safe).
pub trait Xof {
    /// Absorbs more input.
    fn update(&mut self, data: &[u8]);
    /// Ends absorption and returns the output stream.
    fn finalize_xof(self: Box<Self>) -> Box<dyn XofReader>;
}

/// Output stream of an [`Xof`].
pub trait XofReader {
    /// Fills `out` with the next bytes of the stream.
    fn squeeze(&mut self, out: &mut [u8]);
}

/// Absorbs a field with an unambiguous encoding: `len (u64 LE) || data`.
pub fn absorb_field(xof: &mut dyn Xof, data: &[u8]) {
    let length = u64::try_from(data.len()).unwrap_or(u64::MAX);
    xof.update(&length.to_le_bytes());
    xof.update(data);
}

/// Cryptographic suite of a Ghost session.
///
/// `Skein512` is used in counter mode: the absorbed input is compressed to a
/// 64-byte seed `S = Skein-512-512(input)` and the stream is
/// `Skein-512-512(S || 0u64 LE) || Skein-512-512(S || 1u64 LE) || …`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CryptoSuite {
    Ascon,
    Skein512,
    Blake3,
    Shake256,
}

impl CryptoSuite {
    pub const ALL: [Self; 4] = [Self::Ascon, Self::Skein512, Self::Blake3, Self::Shake256];

    /// Stable identifier bound into the MAC.
    pub const fn id(self) -> u8 {
        match self {
            Self::Ascon => 1,
            Self::Skein512 => 2,
            Self::Blake3 => 3,
            Self::Shake256 => 4,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Ascon => "Ascon-XOF128",
            Self::Skein512 => "Skein-512 (counter mode)",
            Self::Blake3 => "BLAKE3",
            Self::Shake256 => "SHAKE256",
        }
    }

    /// `true` if the suite is compiled in (its Cargo feature is enabled).
    pub const fn is_available(self) -> bool {
        match self {
            Self::Ascon => cfg!(feature = "ghost-ascon"),
            Self::Skein512 => cfg!(feature = "ghost-skein"),
            Self::Blake3 => cfg!(feature = "ghost-blake3"),
            Self::Shake256 => true,
        }
    }
}

impl fmt::Display for CryptoSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Unkeyed XOF of `suite`, or [`GhostError::SuiteUnavailable`].
pub fn new_xof(suite: CryptoSuite) -> Result<Box<dyn Xof>, GhostError> {
    match suite {
        CryptoSuite::Shake256 => Ok(Box::new(Shake256::new())),
        #[cfg(feature = "ghost-blake3")]
        CryptoSuite::Blake3 => Ok(Box::new(blake3_suite::Blake3Xof(blake3::Hasher::new()))),
        #[cfg(feature = "ghost-ascon")]
        CryptoSuite::Ascon => Ok(Box::new(ascon_suite::AsconXof::default())),
        #[cfg(feature = "ghost-skein")]
        CryptoSuite::Skein512 => Ok(Box::new(skein_suite::SkeinCounterXof::default())),
        #[allow(unreachable_patterns)]
        unavailable => Err(GhostError::SuiteUnavailable { suite: unavailable }),
    }
}

/// Keyed XOF (PRF) of `suite`, domain-separated by `label`.
///
/// - BLAKE3: native keyed mode with the master key, then `field(label)`.
/// - Other suites: `field(label) || field(key)` absorbed into the XOF. These
///   constructions are not subject to length extension (sponges; Skein's
///   output transform), so prefix keying is sound.
pub fn keyed_xof(
    suite: CryptoSuite,
    label: &[u8],
    key: &MasterKey,
) -> Result<Box<dyn Xof>, GhostError> {
    #[cfg(feature = "ghost-blake3")]
    if suite == CryptoSuite::Blake3 {
        let mut xof: Box<dyn Xof> = Box::new(blake3_suite::Blake3Xof(blake3::Hasher::new_keyed(
            key.bytes(),
        )));
        absorb_field(xof.as_mut(), label);
        return Ok(xof);
    }
    let mut xof = new_xof(suite)?;
    absorb_field(xof.as_mut(), label);
    absorb_field(xof.as_mut(), key.bytes());
    Ok(xof)
}

/// Reads the next little-endian `u64` from a stream.
pub fn next_u64(reader: &mut dyn XofReader) -> u64 {
    let mut bytes = [0_u8; 8];
    reader.squeeze(&mut bytes);
    u64::from_le_bytes(bytes)
}

#[cfg(feature = "ghost-blake3")]
mod blake3_suite {
    use super::{Xof, XofReader};

    pub struct Blake3Xof(pub blake3::Hasher);

    impl Xof for Blake3Xof {
        fn update(&mut self, data: &[u8]) {
            self.0.update(data);
        }

        fn finalize_xof(self: Box<Self>) -> Box<dyn XofReader> {
            Box::new(Blake3Reader(self.0.finalize_xof()))
        }
    }

    pub struct Blake3Reader(blake3::OutputReader);

    impl XofReader for Blake3Reader {
        fn squeeze(&mut self, out: &mut [u8]) {
            self.0.fill(out);
        }
    }
}

#[cfg(feature = "ghost-ascon")]
mod ascon_suite {
    use ascon_hash::{AsconXof128, AsconXof128Reader, ExtendableOutput, Update, XofReader as _};

    use super::{Xof, XofReader};

    #[derive(Default)]
    pub struct AsconXof(AsconXof128);

    impl Xof for AsconXof {
        fn update(&mut self, data: &[u8]) {
            Update::update(&mut self.0, data);
        }

        fn finalize_xof(self: Box<Self>) -> Box<dyn XofReader> {
            Box::new(AsconReader(self.0.finalize_xof()))
        }
    }

    pub struct AsconReader(AsconXof128Reader);

    impl XofReader for AsconReader {
        fn squeeze(&mut self, out: &mut [u8]) {
            self.0.read(out);
        }
    }
}

#[cfg(feature = "ghost-skein")]
mod skein_suite {
    use skein::{Digest, Skein512_512};

    use super::{Xof, XofReader};

    const BLOCK: usize = 64;

    /// Skein-512-512 used as a stream generator in counter mode.
    #[derive(Default)]
    pub struct SkeinCounterXof(Skein512_512);

    impl Xof for SkeinCounterXof {
        fn update(&mut self, data: &[u8]) {
            Digest::update(&mut self.0, data);
        }

        fn finalize_xof(self: Box<Self>) -> Box<dyn XofReader> {
            let mut seed = [0_u8; BLOCK];
            seed.copy_from_slice(self.0.finalize().as_slice());
            Box::new(SkeinCounterReader {
                seed,
                counter: 0,
                block: [0; BLOCK],
                position: BLOCK,
            })
        }
    }

    pub struct SkeinCounterReader {
        seed: [u8; BLOCK],
        counter: u64,
        block: [u8; BLOCK],
        position: usize,
    }

    impl SkeinCounterReader {
        fn refill(&mut self) {
            let mut hasher = Skein512_512::new();
            Digest::update(&mut hasher, self.seed);
            Digest::update(&mut hasher, self.counter.to_le_bytes());
            self.block.copy_from_slice(hasher.finalize().as_slice());
            self.counter = self.counter.wrapping_add(1);
            self.position = 0;
        }
    }

    impl XofReader for SkeinCounterReader {
        fn squeeze(&mut self, out: &mut [u8]) {
            let mut out = out;
            while !out.is_empty() {
                if self.position >= BLOCK {
                    self.refill();
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
}
