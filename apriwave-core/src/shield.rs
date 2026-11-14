//! Apricot Legacy Shield (B5): semantic sidecar filter, no network I/O.
//!
//! Every input runs through stage 1, stage 2, stage 3 (DTI resolution) and the
//! [`DtiPolicyManager`] with Enhanced Security forced on. A blocked input leaves
//! the shield session untouched (transactional ingestion); an accepted input is
//! re-emitted as the **reconstructed canonical form**, never as the original
//! text, so that parser-differential tricks cannot reach the protected device.
//! Virtual patching = replacing the `.awdti` / `.awpolicy` files.

use crate::prelude::*;

use core::fmt;

use crate::dti::Dti;
use crate::error::Diagnostic;
use crate::policy::{DtiPolicyManager, PolicyViolation};
use crate::printer::render_period;
use crate::session::{CheckedIngestError, Session};

/// Why an input was blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShieldViolation {
    /// Stage 1 / stage 2 diagnostic.
    Grammar(Diagnostic),
    /// Policy violation (stage 3 resolution failures are reported as
    /// `PolicyRule::UnresolvedGlyph`).
    Policy(PolicyViolation),
    /// The accepted periods could not be rendered back (internal inconsistency).
    Render,
}

impl fmt::Display for ShieldViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grammar(diagnostic) => write!(f, "{diagnostic}"),
            Self::Policy(violation) => write!(f, "{violation}"),
            Self::Render => f.write_str("canonical reconstruction failed"),
        }
    }
}

impl core::error::Error for ShieldViolation {}

/// Stateful filter in front of a protected device.
#[derive(Debug)]
pub struct LegacyShield {
    session: Session,
    dti: Dti,
    policy: DtiPolicyManager,
}

impl LegacyShield {
    /// Builds a shield; Enhanced Security is always enabled (B5).
    pub fn new(dti: Dti, policy: DtiPolicyManager) -> Self {
        Self {
            session: Session::new(),
            dti,
            policy: policy.with_enhanced_security(true),
        }
    }

    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// Filters one input. `Ok` carries the reconstructed canonical periods.
    pub fn filter(&mut self, input: &str) -> Result<String, Vec<ShieldViolation>> {
        let dti = &self.dti;
        let policy = &self.policy;
        let ids = self
            .session
            .ingest_checked(input, |session, ids| {
                let mut violations = Vec::new();
                for id in ids {
                    if let Err(found) = policy.check_period(session, *id, dti) {
                        violations.extend(found);
                    }
                }
                if violations.is_empty() {
                    Ok(())
                } else {
                    Err(violations)
                }
            })
            .map_err(|error| -> Vec<ShieldViolation> {
                match error {
                    CheckedIngestError::Grammar(diagnostics) => diagnostics
                        .into_iter()
                        .map(ShieldViolation::Grammar)
                        .collect(),
                    CheckedIngestError::Check(violations) => violations
                        .into_iter()
                        .map(ShieldViolation::Policy)
                        .collect(),
                }
            })?;
        let mut rendered = Vec::with_capacity(ids.len());
        for id in ids {
            let period = self
                .session
                .period(id)
                .ok_or_else(|| vec![ShieldViolation::Render])?;
            rendered.push(
                render_period(period, self.session.interner())
                    .map_err(|_| vec![ShieldViolation::Render])?,
            );
        }
        Ok(rendered.join(" "))
    }
}
