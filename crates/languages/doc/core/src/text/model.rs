use crate::model::{DocumentSyntax, EmbedRef, SentenceRef};
use alloc::{string::String, vec::Vec};
use nepl3_core::{budget::StopReason, diagnostic::Report, source::Digest};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationPolicy {
    BaseOnly,
    WithReadings,
    WithAllNotes,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlineTextTarget {
    pub embed: EmbedRef,
    pub guest_digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextIdentity {
    pub document_digest: Digest,
    pub embeds: Vec<InlineTextTarget>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInlineText {
    pub document_digest: Digest,
    pub embed: EmbedRef,
    pub guest_digest: Digest,
    pub text: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainTextRequest {
    pub document: DocumentSyntax,
    pub sentence: SentenceRef,
    pub policy: AnnotationPolicy,
    pub resolved: Vec<ResolvedInlineText>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionMismatch {
    Document,
    Guest,
    Embed,
    Duplicate,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlainTextFailure {
    ExpectedSentence {
        node: u64,
    },
    InvalidResolution {
        entry: u64,
        reason: ResolutionMismatch,
    },
    UnresolvedEmbed {
        embed: EmbedRef,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlainTextOutcome {
    Complete { text: String },
    Invalid { error: PlainTextFailure },
    Stopped { reason: StopReason },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainTextReply {
    pub outcome: PlainTextOutcome,
    pub report: Report,
}
