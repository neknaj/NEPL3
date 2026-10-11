//! Faithful TeX preparation for GitHub Markdown, before page-set integration.
//! The native handle borrows the exact Doc input; it is not a portable proof.
use super::MathDisplayHost;
use super::display::{Preference, TexPreparation};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::{DocKind, DocumentSyntax};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Placement {
    Inline,
    TableCell,
    DisplayBlock,
}

#[derive(Debug)]
pub enum PrepareError<E> {
    Host(super::Error<E>),
    Unsupported {
        node: u64,
        reason: nepl3_math_tex::Unsupported,
    },
    MissingTex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmitError {
    Stopped(StopReason),
    InputMismatch,
    LimitsMismatch,
    Placement,
    UnsafeDelimiter,
    EmptyExpression,
}
impl From<StopReason> for EmitError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Retains source identity by an immutable borrow, without duplicating the Doc.
/// Keep the original cumulative Budget for preparation and emission. Matching
/// Limits alone does not attest that the caller preserved cumulative usage.
pub struct PreparedMarkdown<'a> {
    document: &'a DocumentSyntax,
    node: u64,
    display: bool,
    tex: String,
    limits: Limits,
}

impl<C: FoundationValueCodec> MathDisplayHost<'_, C> {
    /// Uses the existing checked structural TeX path, never Math evaluation.
    /// MathML-only content (including foreign annotations) is an explicit error;
    /// a caller must not replace it with an empty or annotation-free expression.
    pub fn prepare_markdown_node<'a>(
        &mut self,
        document: &'a DocumentSyntax,
        node: u64,
        budget: &mut Budget,
    ) -> Result<PreparedMarkdown<'a>, PrepareError<C::Error>> {
        let prepared = self
            .prepare_node(document, node, Preference::KaTeXPreferred, budget)
            .map_err(PrepareError::Host)?;
        Self::markdown_from_display(document, node, prepared, budget)
    }

    pub(crate) fn prepare_markdown_validated_node<'a>(
        &mut self,
        checked: &nepl3_doc_core::check::ValidatedDocumentSyntax<'a>,
        node: u64,
        budget: &mut Budget,
    ) -> Result<PreparedMarkdown<'a>, PrepareError<C::Error>> {
        let prepared = self
            .prepare_validated_node(checked, node, Preference::KaTeXPreferred, budget)
            .map_err(PrepareError::Host)?;
        Self::markdown_from_display(checked.document(), node, prepared, budget)
    }

    fn markdown_from_display<'a>(
        document: &'a DocumentSyntax,
        node: u64,
        prepared: super::display::PreparedDisplay,
        budget: &mut Budget,
    ) -> Result<PreparedMarkdown<'a>, PrepareError<C::Error>> {
        let display = matches!(
            usize::try_from(node)
                .ok()
                .and_then(|n| document.value.nodes.get(n))
                .map(|n| &n.kind),
            Some(DocKind::DisplayMath { .. })
        );
        let (_, tex) = prepared.into_parts();
        let tex = match tex {
            TexPreparation::Ready { tex, .. } => tex,
            TexPreparation::Unsupported { node, reason } => {
                return Err(PrepareError::Unsupported { node, reason });
            }
            TexPreparation::MathMLOnly => return Err(PrepareError::MissingTex),
        };
        Ok(PreparedMarkdown {
            document,
            node,
            display,
            tex,
            limits: budget.limits(),
        })
    }
}

impl PreparedMarkdown<'_> {
    /// Emits a standalone math fragment, not a rendered GitHub page. The caller
    /// remains responsible for structural placement and surrounding Markdown.
    /// Table pipes are rejected rather than rewritten into different TeX.
    pub fn emit(
        &self,
        document: &DocumentSyntax,
        node: u64,
        placement: Placement,
        budget: &mut Budget,
    ) -> Result<String, EmitError> {
        budget.poll()?;
        if !core::ptr::eq(self.document, document) || node != self.node {
            return Err(EmitError::InputMismatch);
        }
        if budget.limits() != self.limits {
            return Err(EmitError::LimitsMismatch);
        }
        if self.display != matches!(placement, Placement::DisplayBlock) {
            return Err(EmitError::Placement);
        }
        budget.charge(Resource::Work, self.tex.len() as u64)?;
        if self.tex.is_empty() {
            return Err(EmitError::EmptyExpression);
        }
        if self.tex.contains(['`', '\r', '\n'])
            || (placement == Placement::TableCell && self.tex.contains('|'))
        {
            return Err(EmitError::UnsafeDelimiter);
        }
        let (prefix, suffix) = if self.display {
            ("```math\n", "\n```")
        } else {
            ("$`", "`$")
        };
        let length = self
            .tex
            .len()
            .checked_add(prefix.len())
            .and_then(|n| n.checked_add(suffix.len()))
            .ok_or_else(|| EmitError::Stopped(budget.stop(StopReason::OutputLimit)))?;
        budget.charge(Resource::Work, length as u64)?;
        budget.charge(Resource::OutputBytes, length as u64)?;
        budget.charge(Resource::AllocationUnits, length as u64)?;
        let mut output = String::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| EmitError::Stopped(budget.stop(StopReason::AllocationLimit)))?;
        output.push_str(prefix);
        output.push_str(&self.tex);
        output.push_str(suffix);
        Ok(output)
    }
}
