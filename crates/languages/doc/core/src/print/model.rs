use crate::model::{DocField, DocumentSyntax, EmbedRef, GuestLanguage};
use alloc::{string::String, vec::Vec};
use nepl3_core::{budget::StopReason, diagnostic::Report, source::Digest, value::SchemaRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintMode {
    Prefix,
    Compact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestBinding {
    pub schema: SchemaRef,
    pub category: String,
    pub language: GuestLanguage,
}

/// Host-supplied source for one selected guest. Its identity binds the data to
/// the request; this is not a guest semantic-check or parse proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintedGuest {
    pub document_digest: Digest,
    pub embed: EmbedRef,
    pub guest_digest: Digest,
    pub text: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintGuestTarget {
    pub embed: EmbedRef,
    pub guest_digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintIdentity {
    pub document_digest: Digest,
    pub guests: Vec<PrintGuestTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintRequest {
    pub document: DocumentSyntax,
    pub mode: PrintMode,
    pub bindings: Vec<GuestBinding>,
    pub guests: Vec<PrintedGuest>,
}

/// Exact surface entry for reparsing, including otherwise similar fragments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintEntry {
    Article,
    Body,
    Block,
    Flow,
    Sentence,
    Inline,
    Variant,
    Row,
    ListItem,
    Alignment,
    ListStyle,
    Check,
    LinkTarget,
    Asset,
    OptionalRow,
    OptionalSentence,
    OptionalText,
    MathGuest,
    CircuitGuest,
    Guest,
}

/// Text has no invented snapshot identity. The host assigns source ID, revision
/// and URI when saving or parsing this artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArtifact {
    pub text: String,
    pub entry: PrintEntry,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrintMismatch {
    Document,
    Guest,
    Embed,
    Duplicate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrintFailure {
    InvalidBinding { binding: u64 },
    ConflictingBinding { binding: u64 },
    MissingBinding { embed: EmbedRef },
    GuestCategory { embed: EmbedRef },
    InvalidGuest { entry: u64, reason: PrintMismatch },
    UnresolvedGuest { embed: EmbedRef },
    UnprintableName { node: u64, field: DocField },
    UnprintableLanguage { node: u64 },
    UnprintableLiteral { node: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrintOutcome {
    Complete { artifact: SourceArtifact },
    Invalid { error: PrintFailure },
    Stopped { reason: StopReason },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrintReply {
    pub outcome: PrintOutcome,
    pub report: Report,
}
