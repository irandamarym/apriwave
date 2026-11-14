//! Stage 3 (Phase C): DTI resolution, expanded rendering, indexed wire form and views.
//!
//! - [`render_expanded_period`]: expanded canonical form (D8): no `def` or
//!   portability blocks, explicit colon runs, variables and glyph anaphora
//!   substituted, `~~` written as `<glyph>~~`.
//! - [`render_indexed_period`]: written canonical form with DTI keys (D4, D5),
//!   independent of the expanded form (every stage-2 valid period is transmissible).
//! - [`validate_indexed_input`]: checks a period received in indexed mode (D3).
//! - [`render_view`]: human views (`ViewDisplayMode`, `Layout`).

mod expand;
mod indexed;
mod resolve;
mod view;

pub use expand::{render_expanded_period, ExpandError, WireMode};
pub use indexed::{render_indexed_period, render_wire, IndexError};
pub use resolve::{
    resolve_glyph_text, resolve_path, IndexedAtom, MorphFlag, PathResolution, ResolveError,
};
pub use view::{
    render_view, validate_indexed_input, IndexedEntry, IndexedInputError, IndexedReport, Layout,
    ViewDisplayMode, ViewError,
};

#[cfg(test)]
mod tests;
