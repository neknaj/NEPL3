//! Native occurrence context selected by the actual prepared Doc traversal.
use crate::RenderOptions;
use nepl3_core::source::Digest;
use nepl3_doc_core::model::{DocEmbed, DocumentSyntax, EmbedRef};

/// An occurrence in one render traversal, not a global request or artifact ID.
/// The document, options and embed are immutable references to preparation input.
/// Shared nodes/embeds can appear with different ordinals in the same render.
/// Receiving a context does not prove that its import or the whole render
/// succeeded; a callback may retain it even when later validation fails.
/// ```compile_fail
/// fn mutate(c: &mut nepl3_doc_html::guests::Context<'_>) { c.node = 0; }
/// ```
/// ```compile_fail
/// fn clone(c: nepl3_doc_html::guests::Context<'_>) { c.clone(); }
/// ```
/// ```compile_fail
/// use nepl3_doc_html::guests::Context;
/// fn pair(c: Context<'_>) { let _ = Context::new(c.document(), c.options(), c.document_digest(), c.node(), c.reference(), c.embed(), c.ordinal()); }
/// ```
/// ```compile_fail
/// fn mutate_owner(c: nepl3_doc_html::guests::Context<'_>) { c.document().value.nodes.clear(); }
/// ```
#[must_use]
pub struct Context<'a> {
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    document_digest: Digest,
    node: u64,
    reference: EmbedRef,
    embed: &'a DocEmbed,
    ordinal: u64,
}
impl<'a> Context<'a> {
    pub(crate) fn new(
        document: &'a DocumentSyntax,
        options: &'a RenderOptions,
        document_digest: Digest,
        node: u64,
        reference: EmbedRef,
        embed: &'a DocEmbed,
        ordinal: u64,
    ) -> Self {
        Self {
            document,
            options,
            document_digest,
            node,
            reference,
            embed,
            ordinal,
        }
    }
    pub fn document(&self) -> &'a DocumentSyntax {
        self.document
    }
    pub fn options(&self) -> &'a RenderOptions {
        self.options
    }
    pub fn document_digest(&self) -> Digest {
        self.document_digest
    }
    pub fn node(&self) -> u64 {
        self.node
    }
    pub fn reference(&self) -> EmbedRef {
        self.reference
    }
    pub fn embed(&self) -> &'a DocEmbed {
        self.embed
    }
    /// Zero-based index in the successful render's foreign placement vector.
    /// Includes Code, InlineMath and DisplayMath and resets on every render;
    /// this is neither a preparation-global ID nor a one-shot authorization.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
}
