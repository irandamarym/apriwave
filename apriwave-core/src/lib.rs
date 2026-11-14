//! # ApriWave core (ApricotWave Protocol v4.0) — `no_std` + `alloc`
//!
//! Stage 1: segmentation-only lexer, positional `[Tile] [Glyph]` parser, AST that
//! mirrors the grammar, block-local checks, canonical rendering.
//!
//! Stage 2: DSP tree with anchors, determiner inheritance, glyph and sema
//! anaphora, `def` variable lifecycle, coordination and the 3-level close
//! semantics.
//!
//! Stage 3, two-level DTI (on buffers), execution plan, policy manager and the
//! Legacy Shield filter.
//!
//! The crate has no external dependency and no operating-system dependency: it
//! builds for bare-metal targets (e.g. `thumbv6m-none-eabi`,
//! `thumbv7em-none-eabihf`) with a global allocator. File-system access, the
//! editor, JSON export and the CLI live in the `apriwave` crate. Maps and sets are
//! `BTreeMap` / `BTreeSet`: deterministic iteration and O(log n) worst-case
//! lookups.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::indexing_slicing,
    clippy::redundant_clone,
    clippy::clone_on_ref_ptr
)]

extern crate alloc;

/// `alloc` items used throughout the crate (the `std` prelude is not available).
mod prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
}

pub mod analyze;
pub mod ast;
pub mod dti;
pub mod error;
pub mod functions;
mod glyph;
pub mod lexer;
pub mod parser;
pub mod plan;
pub mod policy;
pub mod printer;
pub mod session;
pub mod shake256;
pub mod shield;
pub mod span;
pub mod stage3;
pub mod symbol;
pub mod tiles;
mod validate;
pub mod validator;

pub use error::{Diagnostic, DiagnosticKind, ErrorCode};
pub use parser::{ParserConfig, DEFAULT_MAX_DSP_DEPTH, DEFAULT_MAX_LOOP_ITERATIONS};
pub use session::{PeriodId, Session};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_stage2;
