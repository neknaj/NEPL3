//! Native collector recovery. Raw continuations cannot manufacture prefix marks.
use super::{
    AcceptedTokenizationReply, AcceptedTokenizationReport, TokenizationHostReply,
    TokenizationOutcome, TokenizationScope,
};
use crate::runtime::{AcceptedReport, ReaderError};
use alloc::rc::Rc;
use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    diagnostic::TraceOverflow,
};
#[cfg(test)]
mod tests;

/// Failure of a fresh native token read, distinct from a normal Stopped reply.
#[derive(Debug)]
// Recovery cannot allocate a box after the operation has exhausted its budget.
#[allow(clippy::large_enum_variant)]
pub enum AcceptedTokenizationFailure {
    /// The original accepted prefix is returned; consumed budget is not refunded.
    Recoverable {
        error: ReaderError,
        accepted: AcceptedTokenizationReport,
    },
    /// An internal append-only invariant failed. No accepted proof is issued.
    /// The tokenizer is closed; the enclosing operation must terminate.
    BrokenPrefix {
        original_error: ReaderError,
        observed_stop: Option<StopReason>,
    },
}

/// A host reply and its private entry mark. The mark cannot be applied to
/// another reply. Callers either accept the owned boundary or reject its error.
pub struct RecoverableHostReply {
    pub(super) inner: TokenizationHostReply,
    pub(super) prefix: Prefix,
}
impl RecoverableHostReply {
    pub fn has_host_error(&self) -> bool {
        self.inner.host_error.is_some()
    }
    pub fn accept(self) -> TokenizationHostReply {
        self.inner
    }
    /// Reject a reader-side host error. The caller must discard the tokenizer's
    /// pending slot; BrokenPrefix requires closing the enclosing session.
    /// A valid resource stop retains its live collector instead of rolling back.
    #[allow(clippy::result_large_err)]
    pub fn reject(
        self,
        budget: &Budget,
    ) -> Result<AcceptedTokenizationReply, AcceptedTokenizationFailure> {
        let TokenizationHostReply { reply, host_error } = self.inner;
        let Some(error) = host_error else {
            return Ok(reply);
        };
        if self.prefix.limits != budget.limits()
            || !crate::runtime::usage_at_least(budget.usage(), reply.accepted.report.usage)
        {
            return Err(AcceptedTokenizationFailure::BrokenPrefix {
                original_error: error,
                observed_stop: budget.poll().err(),
            });
        }
        if matches!(reply.outcome, TokenizationOutcome::Stopped { .. }) {
            return Ok(reply);
        }
        let (report, sources, source_maps) = reply.accepted.into_parts();
        match self.prefix.restore(AcceptedReport {
            report,
            sources,
            source_maps,
        }) {
            Ok(accepted) => Err(AcceptedTokenizationFailure::Recoverable { error, accepted }),
            Err(()) => Err(AcceptedTokenizationFailure::BrokenPrefix {
                original_error: error,
                observed_stop: budget.poll().err(),
            }),
        }
    }
}
impl AcceptedTokenizationFailure {
    pub fn into_error(self) -> ReaderError {
        match self {
            Self::Recoverable { error, .. } => error,
            Self::BrokenPrefix { .. } => ReaderError::Continuation,
        }
    }
}

pub(super) struct Prefix {
    scope: Rc<TokenizationScope>,
    limits: Limits,
    lengths: [usize; 4],
    overflow: Option<TraceOverflow>,
}
impl Prefix {
    pub fn capture(accepted: &AcceptedTokenizationReport) -> Self {
        Self {
            scope: Rc::clone(&accepted.scope),
            limits: accepted.limits,
            lengths: [
                accepted.report.diagnostics.len(),
                accepted.report.events.len(),
                accepted.sources.len(),
                accepted.source_maps.len(),
            ],
            overflow: accepted.report.trace_overflow.clone(),
        }
    }
    // No allocation is possible or needed on the recovery path.
    pub fn restore(self, mut live: AcceptedReport) -> Result<AcceptedTokenizationReport, ()> {
        let actual = [
            live.report.diagnostics.len(),
            live.report.events.len(),
            live.sources.len(),
            live.source_maps.len(),
        ];
        if self
            .lengths
            .iter()
            .zip(actual)
            .any(|(saved, actual)| *saved > actual)
        {
            return Err(());
        }
        live.report.diagnostics.truncate(self.lengths[0]);
        live.report.events.truncate(self.lengths[1]);
        live.sources.truncate(self.lengths[2]);
        live.source_maps.truncate(self.lengths[3]);
        live.report.trace_overflow = self.overflow;
        Ok(AcceptedTokenizationReport {
            scope: self.scope,
            limits: self.limits,
            report: live.report,
            sources: live.sources,
            source_maps: live.source_maps,
        })
    }
}

pub(super) fn rejected(
    error: ReaderError,
    mut accepted: AcceptedTokenizationReport,
    budget: &Budget,
) -> AcceptedTokenizationFailure {
    if accepted.limits == budget.limits()
        && crate::runtime::usage_at_least(budget.usage(), accepted.report.usage)
    {
        accepted.report.usage = budget.usage();
    }
    AcceptedTokenizationFailure::Recoverable { error, accepted }
}
