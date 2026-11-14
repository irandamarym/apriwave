//! Hook point for the future DTI validation (Q26: DTI out of scope).
//!
//! No implementation: the trait only fixes the contract, so that stage 2
//! can delegate lexical validation without depending on a dictionary format.

use crate::ast::GlyphPath;
use crate::symbol::Interner;

pub trait GlyphValidator {
    /// Validator-specific error (does not belong to `ErrorCode`, which contains
    /// only the 6 grammatical codes of M§7).
    type Error;

    fn validate(&self, glyph: &GlyphPath, interner: &Interner) -> Result<(), Self::Error>;
}
