//! A narrow host admission check over retained renderer termination metadata.
use super::LocalDocument;
use crate::doc::math::display::TexPreparation;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Debug, PartialEq, Eq)]
pub enum TerminationError {
    Stopped(StopReason),
    Failed { occurrence: u64 },
    Missing { occurrence: u64 },
}
impl From<StopReason> for TerminationError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl LocalDocument<'_, '_, '_, '_> {
    /// Require an explicit clean-termination observation for every occurrence
    /// that requested a renderer. A missing observation is never treated as
    /// false. MathML-only and structurally unsupported TeX need no renderer.
    ///
    /// This checks retained metadata only. The host must separately retain and
    /// reclaim its actual native Supervisor; using an unrelated empty supervisor
    /// cannot prove that work finished. This does not certify renderer identity,
    /// fidelity, resource isolation, accessibility, or an exported artifact.
    /// Ordinary materialize_bundle remains available for diagnostic inspection
    /// and does not implicitly perform this host success-policy check.
    pub fn check_math_termination(&self, b: &mut Budget) -> Result<(), TerminationError> {
        b.poll()?;
        for item in &self.math {
            b.charge(Resource::Work, 1)?;
            let requested = matches!(item.tex(), TexPreparation::Ready { .. });
            check(
                item.context().ordinal(),
                requested,
                item.termination_failure(),
            )?;
        }
        b.poll()?;
        Ok(())
    }
}
fn check(occurrence: u64, requested: bool, failure: Option<bool>) -> Result<(), TerminationError> {
    match failure {
        Some(true) => Err(TerminationError::Failed { occurrence }),
        None if requested => Err(TerminationError::Missing { occurrence }),
        _ => Ok(()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_observations_are_not_clean_termination() {
        for requested in [false, true] {
            assert_eq!(
                check(7, requested, Some(true)),
                Err(TerminationError::Failed { occurrence: 7 })
            );
            assert_eq!(check(7, requested, Some(false)), Ok(()));
        }
        assert_eq!(
            check(7, true, None),
            Err(TerminationError::Missing { occurrence: 7 })
        );
        assert_eq!(check(7, false, None), Ok(()));
    }
}
