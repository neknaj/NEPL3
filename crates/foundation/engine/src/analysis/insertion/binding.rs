//! Execute binding for the exact checked candidate, preserving every outcome.
//! Completion means plan execution finished, not that all references resolved.
use super::*;
use crate::{
    analysis::BoundBindingReply,
    binding::BindingHost,
    portable::{PortableError, analysis},
};
use nepl3_core::value_codec::FoundationValueCodec;

#[derive(Debug)]
pub enum BindingError<E> {
    Preparation(PortableError<E>),
    Access(BindingAccessError),
}

/// Reuse the candidate analysis ID passed to checked::check. The same codec
/// admission ledger and cumulative Budget continue through preparation/execution.
pub fn execute<C: FoundationValueCodec>(
    candidate: &checked::CheckedInsertion<'_, '_>,
    analysis_id: &str,
    codec: &mut C,
    b: &mut Budget,
) -> Result<BoundBindingReply, BindingError<C::Error>> {
    execute_inner(candidate, analysis_id, codec, None, b)
}

/// Host dispatch and Custom authority remain explicit, as in prepared binding.
pub fn execute_with_host<C: FoundationValueCodec>(
    candidate: &checked::CheckedInsertion<'_, '_>,
    analysis_id: &str,
    codec: &mut C,
    host: &mut dyn BindingHost,
    b: &mut Budget,
) -> Result<BoundBindingReply, BindingError<C::Error>> {
    execute_inner(candidate, analysis_id, codec, Some(host), b)
}

fn execute_inner<C: FoundationValueCodec>(
    candidate: &checked::CheckedInsertion<'_, '_>,
    analysis_id: &str,
    codec: &mut C,
    host: Option<&mut dyn BindingHost>,
    b: &mut Budget,
) -> Result<BoundBindingReply, BindingError<C::Error>> {
    if b.limits() != candidate.limits() {
        return Err(BindingError::Access(BindingAccessError::LimitsMismatch));
    }
    let prepared = analysis::prepare(
        analysis_id,
        candidate.candidate().execution().tree(),
        crate::analysis::BindingOptions,
        b.limits(),
        candidate.candidate().seed().profile(),
        codec,
        b,
    )
    .map_err(BindingError::Preparation)?;
    b.charge(nepl3_core::budget::Resource::Work, 128)
        .map_err(|reason| BindingError::Access(BindingAccessError::Stopped(reason)))?;
    if prepared.key() != candidate.keys().1 {
        return Err(BindingError::Access(BindingAccessError::StaleAnalysis));
    }
    match host {
        Some(host) => prepared.execute_with_host(host, b, codec.source_admission()),
        None => prepared.execute(b, codec.source_admission()),
    }
    .map_err(BindingError::Access)
}
