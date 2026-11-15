//! Stage 3 (Phase C): DTI resolution, expanded rendering, indexed wire form and views.
//!
//! - [`render_expanded_period`]: expanded canonical form (D8): no `def` or
//!   portability blocks, explicit colon runs, variables and glyph anaphora
//!   substituted, `~~` kept as `~~`.
//! - [`render_indexed_period`]: written canonical form with DTI keys (D4, D5)
//!   and labels as `LVAAAA<form>` (v4.1), independent of the expanded form
//!   (every stage-2 valid period is transmissible).
//! - [`validate_indexed_input`]: checks a period received in indexed mode (D3),
//!   reporting labels, their subtypes and their homonymy with local glyphs;
//!   [`receive_indexed_input`] also registers new label subtypes and marks the
//!   label slots in the session.
//! - [`render_view`]: human views (`ViewDisplayMode`, `Layout`).

mod expand;
mod indexed;
mod label;
mod resolve;
mod view;

pub use expand::{render_expanded_period, ExpandError, WireMode};
pub use indexed::{render_indexed_period, render_wire, IndexError};
pub use label::{
    is_glyph_form, label_wire, parse_label_wire, LabelSubtype, MalformedLabel, WireLabel,
    LABEL_BASE_CODE, LABEL_BASE_TYPE, LABEL_PREFIX, LABEL_TYPE_LENGTH, MIN_WIRE_LABEL_LENGTH,
};
pub(crate) use label::{slot_content, SlotContent};
pub use resolve::{
    resolve_glyph_text, resolve_path, IndexedAtom, MorphFlag, PathResolution, ResolveError,
};
pub use view::{
    receive_indexed_input, render_view, validate_indexed_input, IndexedEntry, IndexedInputError,
    IndexedLabel, IndexedReport, Layout, NewLabelSubtype, ViewDisplayMode, ViewError,
};

#[cfg(test)]
mod tests;
