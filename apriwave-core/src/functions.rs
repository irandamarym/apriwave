//! Contract of the Functions Dictionary (D12).
//!
//! The protocol **does not execute** computations: execution belongs to Apricot Logic.
//! This module only fixes the interface of the homoiconic rewriting:
//!
//! 1. an external evaluator reads an already analyzed clause (stage 2);
//! 2. it returns **canonical ApriWave text**;
//! 3. the text is submitted to `Session::ingest`, so it cannot escape the
//!    grammar nor alter the session if it is invalid (atomicity).
//!
//! Multiple operands are expressed with `absi`, never with `§`, which remains the
//! `root§attribute` qualifier. Example: `/^ ka somma nu sistema a 10 absi 20 ^/`.
//! Determinism, termination and resource limits are obligations of the evaluator.

use crate::prelude::*;

use core::fmt;

use crate::analyze::GlobalClause;
use crate::error::Diagnostic;
use crate::session::{PeriodId, Session};

pub trait FunctionEvaluator {
    /// Evaluator-specific error (unrelated to the 6 grammatical codes).
    type Error;

    /// Evaluates the given clause and returns canonical ApriWave text.
    fn evaluate(&self, session: &Session, clause: GlobalClause) -> Result<String, Self::Error>;
}

/// Failure outcome of `evaluate_and_ingest`.
#[derive(Debug, PartialEq, Eq)]
pub enum FunctionError<E> {
    /// The evaluator failed.
    Evaluation(E),
    /// The produced text is not valid ApriWave: the session is left unchanged.
    InvalidOutput(Vec<Diagnostic>),
}

impl<E: fmt::Display> fmt::Display for FunctionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evaluation(err) => write!(f, "evaluation failed: {err}"),
            Self::InvalidOutput(diagnostics) => write!(
                f,
                "function output rejected ({} diagnostics)",
                diagnostics.len()
            ),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for FunctionError<E> {}

impl Session {
    /// Evaluates `clause` with `evaluator` and ingests the output as new periods.
    pub fn evaluate_and_ingest<F: FunctionEvaluator>(
        &mut self,
        evaluator: &F,
        clause: GlobalClause,
    ) -> Result<Vec<PeriodId>, FunctionError<F::Error>> {
        let output = evaluator
            .evaluate(self, clause)
            .map_err(FunctionError::Evaluation)?;
        self.ingest(&output).map_err(FunctionError::InvalidOutput)
    }
}
