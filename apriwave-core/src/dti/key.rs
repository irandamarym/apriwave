//! DTI keys (D2, B1, B2).
//!
//! A key is exactly 32 characters from `[a-z0-9]`: 31 body characters followed by
//! one Luhn mod 36 check character. The first character is always a letter
//! `[a-z]`, so a token starting with a digit is never a key: in the indexed form
//! it is a numeral (B4). Keys carry no meaning (no prefixes, no
//! hierarchy), are immutable, are never reused and are never deleted. Random
//! generation happens outside the crate (B2): the crate only validates keys and
//! computes the check character.

use core::fmt;

/// Number of characters in a key.
pub const KEY_LENGTH: usize = 32;
/// Number of body characters (the last character is the check character).
pub const KEY_BODY_LENGTH: usize = 31;
/// Domain separation label of ephemeral session keys ([`DtiKey::session_symbol`]).
pub const SESSION_SYMBOL_DOMAIN: &str = "apriwave-session-symbol-v1";
const RADIX: u32 = 36;

/// Validated DTI key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DtiKey([u8; KEY_LENGTH]);

/// Reasons why a text is not a valid key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyError {
    /// The text does not have exactly 32 characters (or 31 for a body).
    Length { found: usize },
    /// A character outside `[a-z0-9]`.
    Character { ch: char },
    /// The first character is not a letter `[a-z]`.
    FirstCharacter { ch: char },
    /// The Luhn mod 36 check character does not match.
    CheckCharacter,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(f, "invalid key length {found}"),
            Self::Character { ch } => write!(f, "invalid key character '{ch}' (allowed: a-z, 0-9)"),
            Self::FirstCharacter { ch } => {
                write!(f, "invalid first key character '{ch}' (allowed: a-z)")
            }
            Self::CheckCharacter => f.write_str("invalid key check character (Luhn mod 36)"),
        }
    }
}

impl core::error::Error for KeyError {}

/// Code point of a key character in the Luhn mod 36 alphabet (`0-9` then `a-z`).
const fn code_point(byte: u8) -> Option<u32> {
    match byte {
        b'0'..=b'9' => Some((byte - b'0') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 10),
        _ => None,
    }
}

/// Character of a code point (`value < 36`).
fn character(value: u32) -> Option<u8> {
    let value = u8::try_from(value).ok()?;
    match value {
        0..=9 => Some(b'0' + value),
        10..=35 => Some(b'a' + (value - 10)),
        _ => None,
    }
}

/// Luhn mod N sum over `bytes` read from right to left, starting with `first_factor`.
fn luhn_sum(bytes: &[u8], first_factor: u32) -> Option<u32> {
    let mut factor = first_factor;
    let mut sum: u32 = 0;
    for byte in bytes.iter().rev() {
        let addend = factor.checked_mul(code_point(*byte)?)?;
        let addend = addend / RADIX + addend % RADIX;
        sum = sum.checked_add(addend)?;
        factor = if factor == 2 { 1 } else { 2 };
    }
    Some(sum)
}

fn check_alphabet(bytes: &[u8]) -> Result<(), KeyError> {
    if let Some(byte) = bytes.iter().find(|b| code_point(**b).is_none()) {
        return Err(KeyError::Character {
            ch: char::from(*byte),
        });
    }
    match bytes.first() {
        Some(first) if !first.is_ascii_lowercase() => Err(KeyError::FirstCharacter {
            ch: char::from(*first),
        }),
        _ => Ok(()),
    }
}

impl DtiKey {
    /// Parses and validates a key (length, alphabet, first letter, check character).
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        if let Some(ch) = text.chars().find(|c| !c.is_ascii()) {
            return Err(KeyError::Character { ch });
        }
        let bytes: [u8; KEY_LENGTH] = text.as_bytes().try_into().map_err(|_| KeyError::Length {
            found: text.chars().count(),
        })?;
        check_alphabet(&bytes)?;
        match luhn_sum(&bytes, 1) {
            Some(sum) if sum % RADIX == 0 => Ok(Self(bytes)),
            _ => Err(KeyError::CheckCharacter),
        }
    }

    /// Builds a key from 31 body characters (the first in `[a-z]`, the others in
    /// `[a-z0-9]`, typically produced by an external random generator) by
    /// appending the check character.
    pub fn from_random_body(body: [u8; KEY_BODY_LENGTH]) -> Result<Self, KeyError> {
        check_alphabet(&body)?;
        let sum = luhn_sum(&body, 2).ok_or(KeyError::CheckCharacter)?;
        let check = character((RADIX - sum % RADIX) % RADIX).ok_or(KeyError::CheckCharacter)?;
        let mut bytes = [0_u8; KEY_LENGTH];
        for (target, source) in bytes
            .iter_mut()
            .zip(body.iter().chain(core::iter::once(&check)))
        {
            *target = *source;
        }
        Ok(Self(bytes))
    }

    /// Ephemeral session key of a symbol that is not an enabled primary form of
    /// the DTI (`def` variable names, anchor-id atoms) in the indexed wire form.
    ///
    /// Deterministic: `SHAKE256(len(domain) u64 LE || domain || name)` with domain
    /// [`SESSION_SYMBOL_DOMAIN`], read as a byte stream with unbiased rejection
    /// sampling: the first body character from `[a-z]` (bytes < 234, value mod 26),
    /// the next 30 from `[a-z0-9]` (bytes < 252, value mod 36, Luhn alphabet order
    /// `0-9a-z`), then the Luhn mod 36 check character.
    pub fn session_symbol(name: &str) -> Result<Self, KeyError> {
        let mut xof = crate::shake256::Shake256::default();
        let domain = SESSION_SYMBOL_DOMAIN.as_bytes();
        xof.absorb(
            &u64::try_from(domain.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        xof.absorb(domain);
        xof.absorb(name.as_bytes());
        let mut reader = xof.finalize();
        let mut body = [0_u8; KEY_BODY_LENGTH];
        let mut filled = 0_usize;
        let mut block = [0_u8; 64];
        while filled < KEY_BODY_LENGTH {
            reader.squeeze(&mut block);
            for byte in block {
                let Some(slot) = body.get_mut(filled) else {
                    break;
                };
                let value = if filled == 0 {
                    (byte < 234).then(|| b'a' + byte % 26)
                } else {
                    (byte < 252)
                        .then(|| u32::from(byte % 36))
                        .and_then(character)
                };
                if let Some(value) = value {
                    *slot = value;
                    filled = filled.saturating_add(1);
                }
            }
        }
        // The body is valid by construction: the error is never produced.
        Self::from_random_body(body)
    }

    /// Textual form of the key.
    pub fn as_str(&self) -> &str {
        // Every byte is ASCII by construction, so the conversion cannot fail;
        // the fallback keeps the function total without panicking.
        core::str::from_utf8(&self.0).unwrap_or("")
    }
}

impl fmt::Debug for DtiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DtiKey({})", self.as_str())
    }
}

impl fmt::Display for DtiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
