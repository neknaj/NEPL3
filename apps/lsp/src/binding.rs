//! Analysis-wide binding status for a host presentation layer.
//! This is not an LSP transport, edit acceptance policy, or source-position map.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    facts::{OccurrenceRole, ReferenceResolution},
};
use nepl3_engine::{
    analysis::{AnalysisKey, BoundBindingReply},
    binding::{BindingAnalysis, BindingError, BindingOutcome, BindingProgress},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReferenceCounts {
    pub resolved: u64,
    pub unresolved: u64,
    pub ambiguous: u64,
    pub deferred: u64,
}
/// Complete still includes unresolved, ambiguous, or deferred references.
/// Invalid/stopped partial facts are never presented as completed analysis.
pub enum BindingState<'a> {
    Complete {
        analysis: &'a BindingAnalysis,
        references: ReferenceCounts,
    },
    Invalid {
        error: &'a BindingError,
        progress: &'a BindingProgress,
    },
    Stopped {
        reason: StopReason,
        progress: &'a BindingProgress,
    },
}
pub struct BindingStatus<'a> {
    pub key: AnalysisKey,
    pub state: BindingState<'a>,
    /// Original analysis diagnostics, events, and usage are borrowed unchanged.
    pub report: &'a Report,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusError {
    Stopped(StopReason),
    StaleAnalysis,
    CountOverflow,
}
impl From<StopReason> for StatusError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Inspect a native reply only after matching the host's current requested key.
/// Counts include Reference-role occurrences across all analyzed bundles/sources;
/// Definition, Import, and Export roles remain available in the borrowed facts.
/// A projection stop returns no status. Its cost stays in the caller's Budget,
/// separately from the original BindingReply report.
pub fn inspect<'a>(
    reply: &'a BoundBindingReply,
    expected: &AnalysisKey,
    b: &mut Budget,
) -> Result<BindingStatus<'a>, StatusError> {
    b.charge(Resource::Work, 128)?;
    if reply.key() != *expected {
        return Err(StatusError::StaleAnalysis);
    }
    let state = match &reply.reply().outcome {
        BindingOutcome::Complete(analysis) => {
            let mut references = ReferenceCounts::default();
            for occurrence in &analysis.facts().occurrences {
                b.charge(Resource::Work, 1)?;
                if occurrence.role != OccurrenceRole::Reference {
                    continue;
                }
                references.record(&occurrence.resolution)?;
            }
            BindingState::Complete {
                analysis,
                references,
            }
        }
        BindingOutcome::Invalid { error, progress } => BindingState::Invalid { error, progress },
        BindingOutcome::Stopped { reason, progress } => BindingState::Stopped {
            reason: *reason,
            progress,
        },
    };
    Ok(BindingStatus {
        key: reply.key(),
        state,
        report: &reply.reply().report,
    })
}

impl ReferenceCounts {
    fn record(&mut self, resolution: &ReferenceResolution) -> Result<(), StatusError> {
        let count = match resolution {
            ReferenceResolution::Resolved(_) => &mut self.resolved,
            ReferenceResolution::Unresolved(_) => &mut self.unresolved,
            ReferenceResolution::Ambiguous(_) => &mut self.ambiguous,
            ReferenceResolution::Deferred(_) => &mut self.deferred,
        };
        *count = count.checked_add(1).ok_or(StatusError::CountOverflow)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::facts::EntityId;
    #[test]
    fn resolution_classes_remain_distinct() {
        let mut counts = ReferenceCounts::default();
        for resolution in [
            ReferenceResolution::Resolved(EntityId(0)),
            ReferenceResolution::Unresolved("x".into()),
            ReferenceResolution::Ambiguous(vec![EntityId(0), EntityId(1)]),
            ReferenceResolution::Deferred(vec![]),
        ] {
            assert_eq!(counts.record(&resolution), Ok(()));
        }
        assert_eq!(
            counts,
            ReferenceCounts {
                resolved: 1,
                unresolved: 1,
                ambiguous: 1,
                deferred: 1
            }
        );
        counts.deferred = u64::MAX;
        assert_eq!(
            counts.record(&ReferenceResolution::Deferred(vec![])),
            Err(StatusError::CountOverflow)
        );
        assert_eq!(counts.deferred, u64::MAX);
    }
}
