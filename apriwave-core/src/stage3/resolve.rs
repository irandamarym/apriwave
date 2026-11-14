//! Stage 3 glyph → DTI key resolution (B4 option 1, B9) with active validation
//! of the morphology flags.
//!
//! Canonical text admits only **primary forms** (synonyms and `display` forms are
//! normalized by the editor). DTI forms are single atoms, so a qualified glyph
//! `s1§…§sn` is always resolved segment by segment into `<K1>§…§<Kn>`: the `§`
//! structure is isomorphic between the lexical and the indexed form.
//!
//! - `[+UNION]`: in a qualified glyph every concept joined by `§` must carry
//!   `+UNION`;
//! - `[+AGGL]` / `[+CONTR]`: a record with both `base:` and `param:` whose form is
//!   a single atom is a compound form. When the form is the concatenation of a
//!   primary form of `base` and one of `param` it is an agglutination (both
//!   components must carry `+AGGL`), otherwise a contraction (both must carry
//!   `+CONTR`). Compound forms must be censused as records: unknown atoms are
//!   never segmented automatically (the wire could not tell them from a union);
//! - `[+AGGL_NUM]`: every digit concept of a numeral must carry `+AGGL_NUM`.

use crate::prelude::*;

use core::fmt;

use crate::ast::GlyphPath;
use crate::dti::{Dti, DtiKey, FormResolution};
use crate::printer::render_glyph_path;
use crate::symbol::Interner;

/// One `§` segment of an indexed glyph path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexedAtom {
    Key(DtiKey),
    /// ASCII numeral kept literal (B4 option 1).
    Numeral(String),
}

impl fmt::Display for IndexedAtom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(key) => f.write_str(key.as_str()),
            Self::Numeral(digits) => f.write_str(digits),
        }
    }
}

/// Resolution of a glyph path: one key for an atom, per-segment atoms otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathResolution {
    Record(DtiKey),
    Segments(Vec<IndexedAtom>),
}

impl PathResolution {
    /// Indexed textual form (`<key>` or `<atom>§<atom>…`).
    pub fn indexed_text(&self) -> String {
        match self {
            Self::Record(key) => key.as_str().to_owned(),
            Self::Segments(atoms) => atoms
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join("§"),
        }
    }

    /// Keys referenced by the resolution, in order.
    pub fn keys(&self) -> Vec<DtiKey> {
        match self {
            Self::Record(key) => vec![*key],
            Self::Segments(atoms) => atoms
                .iter()
                .filter_map(|atom| match atom {
                    IndexedAtom::Key(key) => Some(*key),
                    IndexedAtom::Numeral(_) => None,
                })
                .collect(),
        }
    }
}

/// Morphology flag required by a form (B9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MorphFlag {
    Union,
    Aggl,
    Contr,
    AgglNum,
}

impl MorphFlag {
    /// Surface of the flag in the `.awdti` format.
    pub const fn surface(self) -> &'static str {
        match self {
            Self::Union => "+UNION",
            Self::Aggl => "+AGGL",
            Self::Contr => "+CONTR",
            Self::AgglNum => "+AGGL_NUM",
        }
    }

    /// `true` when the concept carries the flag.
    pub fn allowed_by(self, dti: &Dti, key: &DtiKey) -> bool {
        dti.concept(key).is_some_and(|concept| match self {
            Self::Union => concept.morph.union,
            Self::Aggl => concept.morph.aggl,
            Self::Contr => concept.morph.contr,
            Self::AgglNum => concept.morph.aggl_num,
        })
    }
}

impl fmt::Display for MorphFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.surface())
    }
}

/// Errors of the glyph → key resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The form maps to several concepts: the user must choose (B3).
    Ambiguous {
        form: String,
        keys: Vec<DtiKey>,
    },
    /// The form is not in any lexicon.
    UnknownForm {
        form: String,
    },
    /// The concept exists but is not enabled locally (B5).
    NotEnabled {
        form: String,
        key: DtiKey,
    },
    /// A digit is outside the declared base, or has no digit concept (B4).
    InvalidNumeral {
        form: String,
    },
    /// The form needs a morphology permission the concept `key` lacks (B9).
    MorphViolation {
        form: String,
        key: DtiKey,
        flag: MorphFlag,
    },
    UnknownSymbol,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous { form, keys } => {
                write!(f, "ambiguous form '{form}' ({} concepts)", keys.len())
            }
            Self::UnknownForm { form } => write!(f, "form '{form}' is not in the DTI"),
            Self::NotEnabled { form, key } => {
                write!(
                    f,
                    "form '{form}' maps to {key}, which is not enabled (add `use {key}`)"
                )
            }
            Self::InvalidNumeral { form } => write!(f, "invalid numeral '{form}'"),
            Self::MorphViolation { form, key, flag } => {
                write!(f, "form '{form}' requires {flag} on concept {key}")
            }
            Self::UnknownSymbol => f.write_str("symbol not found in the interner"),
        }
    }
}

impl core::error::Error for ResolveError {}

/// Primary forms of a key in the local and in the exchange language.
fn primary_forms(dti: &Dti, key: DtiKey) -> impl Iterator<Item = &str> {
    [dti.overlay().lexicon(), dti.universal().lexicon()]
        .into_iter()
        .filter_map(move |lexicon| lexicon.entry(&key).map(|entry| &*entry.primary))
}

/// `[+AGGL]` / `[+CONTR]` on a compound record (`base:` + `param:`, atomic form).
fn check_compound(dti: &Dti, form: &str, key: &DtiKey) -> Result<(), ResolveError> {
    let Some(concept) = dti.concept(key) else {
        return Ok(());
    };
    let (Some(base), Some(param)) = (concept.base, concept.param) else {
        return Ok(());
    };
    if form.contains(crate::ast::QUALIFIER_SEPARATOR) {
        return Ok(());
    }
    let agglutinated = primary_forms(dti, base).any(|head| {
        form.strip_prefix(head)
            .is_some_and(|tail| primary_forms(dti, param).any(|p| p == tail))
    });
    let flag = if agglutinated {
        MorphFlag::Aggl
    } else {
        MorphFlag::Contr
    };
    match [base, param]
        .into_iter()
        .find(|part| !flag.allowed_by(dti, part))
    {
        Some(part) => Err(ResolveError::MorphViolation {
            form: form.to_owned(),
            key: part,
            flag,
        }),
        None => Ok(()),
    }
}

/// Resolves a single primary form to an enabled key.
fn resolve_form(dti: &Dti, form: &str) -> Result<DtiKey, ResolveError> {
    match dti.resolve_primary(form) {
        FormResolution::Unique(key) if dti.is_enabled(&key) => {
            check_compound(dti, form, &key)?;
            Ok(key)
        }
        FormResolution::Unique(key) => Err(ResolveError::NotEnabled {
            form: form.to_owned(),
            key,
        }),
        FormResolution::Ambiguous(keys) => Err(ResolveError::Ambiguous {
            form: form.to_owned(),
            keys,
        }),
        FormResolution::Unknown => Err(ResolveError::UnknownForm {
            form: form.to_owned(),
        }),
    }
}

/// B4 option 1 + B9: when the snapshot declares a numeral system, an all-digit
/// atom is a numeral; every digit must be below the base (`InvalidNumeral`) and
/// its concept must carry `[+AGGL_NUM]` (`MorphViolation`).
pub(crate) fn numeral(dti: &Dti, atom: &str) -> Option<Result<IndexedAtom, ResolveError>> {
    let system = dti.universal().numeral()?;
    if atom.is_empty() || !atom.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    for byte in atom.bytes() {
        let digit = usize::from(byte.saturating_sub(b'0'));
        let key = system
            .digits
            .get(digit)
            .filter(|_| digit < usize::from(system.base));
        let Some(key) = key else {
            return Some(Err(ResolveError::InvalidNumeral {
                form: atom.to_owned(),
            }));
        };
        if !MorphFlag::AgglNum.allowed_by(dti, key) {
            return Some(Err(ResolveError::MorphViolation {
                form: atom.to_owned(),
                key: *key,
                flag: MorphFlag::AgglNum,
            }));
        }
    }
    Some(Ok(IndexedAtom::Numeral(atom.to_owned())))
}

/// Resolves one atom (numeral or lexical form).
pub(crate) fn resolve_atom(dti: &Dti, atom: &str) -> Result<IndexedAtom, ResolveError> {
    match numeral(dti, atom) {
        Some(result) => result,
        None => resolve_form(dti, atom).map(IndexedAtom::Key),
    }
}

/// Every `§` segment is resolved on its own (DTI forms are atoms); every concept
/// joined by `§` must carry `[+UNION]` (numerals are not concepts: exempt).
pub fn resolve_path(
    path: &GlyphPath,
    interner: &Interner,
    dti: &Dti,
) -> Result<PathResolution, ResolveError> {
    // Atoms never contain `§` (the parser splits on it): the text form is exact.
    let text = render_glyph_path(path, interner).map_err(|_| ResolveError::UnknownSymbol)?;
    resolve_glyph_text(&text, dti)
}

/// First concept of a `§` union lacking `[+UNION]` (numerals are exempt).
pub(crate) fn union_violation(dti: &Dti, atoms: &[IndexedAtom]) -> Option<DtiKey> {
    if atoms.len() < 2 {
        return None;
    }
    atoms.iter().find_map(|atom| match atom {
        IndexedAtom::Key(key) if !MorphFlag::Union.allowed_by(dti, key) => Some(*key),
        IndexedAtom::Key(_) | IndexedAtom::Numeral(_) => None,
    })
}

/// Resolves the text of a glyph path (`a§b`): same rules as [`resolve_path`]
/// (segment by segment, primary forms, morphology flags).
pub fn resolve_glyph_text(text: &str, dti: &Dti) -> Result<PathResolution, ResolveError> {
    let atoms = text
        .split(crate::ast::QUALIFIER_SEPARATOR)
        .map(|segment| resolve_atom(dti, segment))
        .collect::<Result<Vec<IndexedAtom>, ResolveError>>()?;
    if let [IndexedAtom::Key(key)] = atoms.as_slice() {
        return Ok(PathResolution::Record(*key));
    }
    if let Some(key) = union_violation(dti, &atoms) {
        return Err(ResolveError::MorphViolation {
            form: text.to_owned(),
            key,
            flag: MorphFlag::Union,
        });
    }
    Ok(PathResolution::Segments(atoms))
}
