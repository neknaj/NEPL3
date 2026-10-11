//! Opt-in source admission between structural framing and common Report decoding.
use super::*;
use core::convert::Infallible;
use nepl3_core::{budget::Resource, operation::ProviderFrame};

/// Terminal payload available to the caller-selected source policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplyPayloadKind {
    Complete,
    InvalidPartial,
    StoppedPartial,
}

/// An immutable payload checked against the saved operation's output type.
/// This is not domain acceptance or evidence that the request was issued.
pub struct ReplyPayload<'a> {
    pub kind: ReplyPayloadKind,
    pub value: &'a NdfValue,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ReplyAdmissionError<E> {
    Wire(WireError),
    RequestId { expected: u64, actual: u64 },
    UnknownOperation,
    Policy(E),
}

/// Framing failures remain distinguishable without parsing the frame again.
#[derive(Debug, Eq, PartialEq)]
pub enum ReplyFrameError {
    Wire(WireError),
    PeerClose,
    ExpectedReply,
}
impl From<WireError> for ReplyFrameError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

impl<E> From<WireError> for ReplyAdmissionError<E> {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

/// Sealed structural frame retaining the original conversion ledger and budget.
/// Dropping this value does not undo consumed resources or admitted identities.
pub struct PendingReplyFrame<'a> {
    value: crate::StructuralValue,
    request_id: u64,
    registry: &'a SchemaRegistry,
    foundation: &'a SchemaRef,
    admission: &'a mut SourceAdmission,
    budget: &'a mut Budget,
}

/// Check one frame without resolving its outer Report source references.
/// Only Reply is accepted. Subsequent frames remain in the returned suffix.
pub fn decode_pending_reply_frame<'input, 'context>(
    input: &'input [u8],
    final_input: bool,
    registry: &'context SchemaRegistry,
    admission: &'context mut SourceAdmission,
    budget: &'context mut Budget,
) -> Result<Option<(PendingReplyFrame<'context>, &'input [u8])>, ReplyFrameError> {
    let s = schema(registry)?;
    let Some((value, rest)) = crate::frame::decode_checked(
        input,
        final_input,
        &expected("ProviderFrame"),
        registry,
        budget,
    )?
    else {
        return Ok(None);
    };
    let id = match variant_parts(value.value(), s, "ProviderFrame")? {
        ("Reply", [id, _]) => id,
        ("Close", []) => return Err(ReplyFrameError::PeerClose),
        _ => return Err(ReplyFrameError::ExpectedReply),
    };
    let request_id = as_u64(id)?;
    Ok(Some((
        PendingReplyFrame {
            value,
            request_id,
            registry,
            foundation: s,
            admission,
            budget,
        },
        rest,
    )))
}

impl PendingReplyFrame<'_> {
    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Finish using only the original explicit source closure.
    pub fn finish(
        self,
        saved_request: &Invoke,
        original_sources: &SourceStore,
    ) -> Result<ProviderFrame, ReplyAdmissionError<Infallible>> {
        self.finish_inner(saved_request, original_sources, |_, _, _, _, _| Ok(None))
    }

    /// Invoke a trusted, caller-selected source policy at most once, after the
    /// ID and selected terminal output type checks. The returned store is codec
    /// context only; no execution grants or request lifetimes are changed.
    ///
    /// The policy must use, not replace, the supplied budget and admission ledger.
    /// Await and report-only results never invoke this terminal-payload policy.
    pub fn finish_with<E>(
        self,
        saved_request: &Invoke,
        original_sources: &SourceStore,
        policy: impl FnOnce(
            ReplyPayload<'_>,
            &SchemaRegistry,
            &SourceStore,
            &mut SourceAdmission,
            &mut Budget,
        ) -> Result<SourceStore, E>,
    ) -> Result<ProviderFrame, ReplyAdmissionError<E>> {
        self.finish_inner(saved_request, original_sources, |v, r, s, a, b| {
            policy(v, r, s, a, b).map(Some)
        })
    }

    fn finish_inner<E>(
        self,
        saved_request: &Invoke,
        original_sources: &SourceStore,
        policy: impl FnOnce(
            ReplyPayload<'_>,
            &SchemaRegistry,
            &SourceStore,
            &mut SourceAdmission,
            &mut Budget,
        ) -> Result<Option<SourceStore>, E>,
    ) -> Result<ProviderFrame, ReplyAdmissionError<E>> {
        self.budget.poll().map_err(WireError::from)?;
        if self.request_id != saved_request.request_id {
            return Err(ReplyAdmissionError::RequestId {
                expected: saved_request.request_id,
                actual: self.request_id,
            });
        }
        let operation = &saved_request.operation;
        let (reference, descriptor) = self
            .registry
            .selected_descriptor_with_budget(
                &operation.schema.package,
                operation.schema.revision,
                self.budget,
            )
            .map_err(WireError::from)?
            .ok_or(ReplyAdmissionError::UnknownOperation)?;
        if reference != &operation.schema {
            return Err(ReplyAdmissionError::UnknownOperation);
        }
        let mut selected = None;
        for candidate in &descriptor.operations {
            self.budget
                .charge(
                    Resource::Work,
                    (candidate.name.len() as u64)
                        .saturating_add(operation.name.len() as u64)
                        .saturating_add(1),
                )
                .map_err(WireError::from)?;
            if candidate.name == operation.name {
                selected = Some(candidate);
                break;
            }
        }
        let selected = selected.ok_or(ReplyAdmissionError::UnknownOperation)?;
        // The registry stays immutably borrowed across both stages. Reuse its
        // initially selected identity instead of performing another linear scan.
        let s = self.foundation;
        let (_, [_, response]) = variant_parts(self.value.value(), s, "ProviderFrame")? else {
            return Err(WireError::InvalidType.into());
        };
        let payload = terminal_payload(response, s)?;
        let response_sources = if let Some(payload) = payload {
            self.registry
                .validate(&selected.output, payload.value, self.budget)
                .map_err(WireError::from)?;
            let result = policy(
                payload,
                self.registry,
                original_sources,
                self.admission,
                self.budget,
            )
            .map_err(ReplyAdmissionError::Policy)?;
            // A policy returning Ok cannot erase a sticky stop.
            self.budget.poll().map_err(WireError::from)?;
            result
        } else {
            None
        };
        let sources = response_sources.as_ref().unwrap_or(original_sources);
        reply::admit(sources, self.admission, self.budget)?;
        let reply = reply::reply_from(
            response,
            s,
            self.registry,
            sources,
            self.admission,
            self.budget,
        )?;
        Ok(ProviderFrame::Reply {
            request_id: self.request_id,
            reply,
        })
    }
}

fn terminal_payload<'a>(
    value: &'a NdfValue,
    s: &SchemaRef,
) -> Result<Option<ReplyPayload<'a>>, WireError> {
    let (case, fields) = variant_parts(value, s, "OperationReply")?;
    let (kind, value) = match (case, fields) {
        ("Complete", [value, _, _, _, _]) => (ReplyPayloadKind::Complete, value),
        ("Invalid", [NdfValue::Some(value), _, _, _, _]) => {
            (ReplyPayloadKind::InvalidPartial, value.as_ref())
        }
        ("Stopped", [_, NdfValue::Some(value), _, _, _, _]) => {
            (ReplyPayloadKind::StoppedPartial, value.as_ref())
        }
        ("Invalid", [NdfValue::None, _, _, _, _])
        | ("Stopped", [_, NdfValue::None, _, _, _, _])
        | ("Await", [_, _, _, _, _, _]) => return Ok(None),
        _ => return Err(WireError::InvalidType),
    };
    Ok(Some(ReplyPayload { kind, value }))
}
