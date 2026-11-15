//! # ApriWave (GHLIP Protocol v4.1) — host crate (`std`)
//!
//! Re-exports the whole `no_std` core ([`apriwave_core`]) under the same module
//! paths and adds the host-only parts: the file-system API of the `.dti.bin`
//! cache ([`dti::load_or_compile_universal_path`], …), the assisted editor, the
//! JSON export, the CLI and the Ghost Protocol (frozen).

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

pub use apriwave_core::{
    analyze, ast, error, functions, lexer, parser, plan, policy, printer, session, shake256,
    shield, span, stage3, symbol, tiles, validator,
};
pub use apriwave_core::{
    Diagnostic, DiagnosticKind, ErrorCode, ParserConfig, PeriodId, Session, Severity, SlotKind,
    WarningCodes, DEFAULT_MAX_DSP_DEPTH, DEFAULT_MAX_LOOP_ITERATIONS,
};

/// Two-level DTI: the core model on buffers plus the host file-system cache API.
pub mod dti {
    pub use crate::dti_fs::{
        binary_cache_path, load_or_compile_local_path, load_or_compile_universal_path,
        DtiFileError, BINARY_SUFFIX, SOURCE_EXTENSION,
    };
    pub use apriwave_core::dti::*;
}

mod dti_fs;
pub mod editor;
pub mod export;
pub mod ghost;

#[cfg(test)]
mod tests_phase_a;
