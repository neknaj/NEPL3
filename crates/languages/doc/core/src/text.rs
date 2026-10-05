//! Explicit plain-text projection. This is neither a reparsable sentence literal
//! nor evidence that an embedded language has passed its semantic checks.
mod model;
mod run;
use crate::{
    model::*,
    portable::{self, PortableError},
};
use alloc::vec::Vec;
pub use model::*;
use nepl3_core::{
    budget::{Budget, Limits, Resource},
    schema::SchemaRegistry,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

pub const DOCUMENT_DOMAIN: &[u8] = b"NEPL3.Doc.PlainText.Document.v1\0";
pub const GUEST_DOMAIN: &[u8] = b"NEPL3.Doc.PlainText.Guest.v1\0";
/// Execute one request with the same Budget and SourceAdmission for document
/// validation, identity computation and output. Malformed input is a boundary
/// error; a resource stop during preparation is a formal stopped reply.
pub fn plain_text<C: FoundationValueCodec>(
    request: &PlainTextRequest,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PlainTextReply, PortableError<C::Error>> {
    match prepare(&request.document, registry, codec, b) {
        Ok(prepared) => {
            Ok(prepared.execute(request.sentence, request.policy, &request.resolved, b))
        }
        Err(PortableError::Stopped(reason)) => {
            let reason = b.stop(reason);
            Ok(PlainTextReply {
                outcome: PlainTextOutcome::Stopped { reason },
                report: nepl3_core::diagnostic::Report {
                    usage: b.usage(),
                    ..Default::default()
                },
            })
        }
        Err(error) => Err(error),
    }
}
/// Borrowed input projection. Admission, identity and projection share one Budget.
pub fn plain_text_borrowed<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    sentence: SentenceRef,
    policy: AnnotationPolicy,
    resolved: &[ResolvedInlineText],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PlainTextReply, PortableError<C::Error>> {
    let prepared = prepare(document, registry, codec, b)?;
    Ok(prepared.execute(sentence, policy, resolved, b))
}
/// Host-visible identities are data, not transferable validation or paid-work
/// proofs. `plain_text` always validates and computes identities under its own
/// operation Budget and SourceAdmission; this value cannot bypass that work.
pub struct PreparedText<'a> {
    document: &'a DocumentSyntax,
    identity: TextIdentity,
    limits: Limits,
}
/// Native prepared projection error, separate from the portable reply schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedTextError {
    LimitsMismatch,
}
impl<'a> PreparedText<'a> {
    pub fn identity(&self) -> &TextIdentity {
        &self.identity
    }
    /// Project another sentence from this immutable, validated document.
    /// Continue the preparation operation's cumulative Budget. Equal limits
    /// are checked, but do not prove that the caller preserved paid usage.
    /// Entry, resolution, output and traversal checks still run on every call;
    /// only document admission and identity computation are reused. Serialized
    /// identities cannot construct this value or bypass document validation.
    pub fn project(
        &self,
        sentence: SentenceRef,
        policy: AnnotationPolicy,
        resolved: &[ResolvedInlineText],
        b: &mut Budget,
    ) -> Result<PlainTextReply, PreparedTextError> {
        if b.limits() != self.limits {
            return Err(PreparedTextError::LimitsMismatch);
        }
        Ok(self.execute(sentence, policy, resolved, b))
    }
    fn execute(
        &self,
        sentence: SentenceRef,
        policy: AnnotationPolicy,
        resolved: &[ResolvedInlineText],
        b: &mut Budget,
    ) -> PlainTextReply {
        let result = run::execute(self, sentence, policy, resolved, b);
        let outcome = match result {
            Ok(text) => PlainTextOutcome::Complete { text },
            Err(run::Failure::Invalid(error)) => PlainTextOutcome::Invalid { error },
            Err(run::Failure::Stopped(reason)) => {
                let reason = b.stop(reason);
                PlainTextOutcome::Stopped { reason }
            }
        };
        PlainTextReply {
            outcome,
            report: nepl3_core::diagnostic::Report {
                usage: b.usage(),
                ..Default::default()
            },
        }
    }
}
fn boundary<E: FoundationCodecError>(e: E) -> PortableError<E> {
    match e.stop_reason() {
        Some(s) => PortableError::Stopped(s),
        None => PortableError::Foundation(e),
    }
}
/// Compute identities for host-supplied text. This is a separate preparation
/// operation, not a promise that a future execution has already paid its costs.
pub fn prepare<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PreparedText<'a>, PortableError<C::Error>> {
    let value = portable::to_value(document, registry, codec, b)?;
    let document_digest = codec
        .canonical_value_digest(DOCUMENT_DOMAIN, &value, b)
        .map_err(boundary)?;
    let mut embeds = Vec::new();
    for (index, embed) in document.value.embeds.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if embed.kind != EmbedKind::InlineMath {
            continue;
        }
        let value = codec
            .encode_foreign_closure(&embed.closure, b)
            .map_err(boundary)?;
        let guest_digest = codec
            .canonical_value_digest(GUEST_DOMAIN, &value, b)
            .map_err(boundary)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<InlineTextTarget>() as u64,
        )?;
        embeds.push(InlineTextTarget {
            embed: EmbedRef(index as u64),
            guest_digest,
        });
    }
    Ok(PreparedText {
        document,
        limits: b.limits(),
        identity: TextIdentity {
            document_digest,
            embeds,
        },
    })
}
