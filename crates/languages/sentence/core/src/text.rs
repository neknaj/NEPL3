//! Plain-text projection of Sentence-owned content. Foreign text is supplied by
//! an explicit host adapter and bound to this immutable input. No guest executes.
use crate::{check, model::*};
use alloc::{string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationPolicy {
    BaseOnly,
    WithReadings,
    WithAllNotes,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Shape(check::Error),
    Embed(EmbedRef),
    WrongScope,
    Duplicate(EmbedRef),
    Unresolved(EmbedRef),
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<check::Error> for Error {
    fn from(error: check::Error) -> Self {
        match error {
            check::Error::Stopped(reason) => Self::Stopped(reason),
            error => Self::Shape(error),
        }
    }
}

/// The value, its foreign source closures and their registry stay immutable.
/// This native proof establishes structure, not the meaning of supplied text.
pub struct PreparedText<'a> {
    shape: check::CheckedShape<'a>,
    _registry: &'a SchemaRegistry,
}

/// Host-supplied foreign text bound to one input arena and one embed. Construction
/// is explicit; it does not authenticate a provider or certify guest semantics.
pub struct ResolvedInline<'a, 'text> {
    value: &'a SentenceValue,
    embed: EmbedRef,
    text: &'text str,
}

pub fn prepare<'a>(
    value: &'a SentenceValue,
    registry: &'a SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<PreparedText<'a>, Error> {
    let shape = value.validate_shape(b)?;
    shape.validate_foreign(registry, b, admission)?;
    Ok(PreparedText {
        shape,
        _registry: registry,
    })
}

impl<'a> PreparedText<'a> {
    pub fn resolve<'text>(
        &self,
        embed: EmbedRef,
        text: &'text str,
        b: &mut Budget,
    ) -> Result<ResolvedInline<'a, 'text>, Error> {
        b.charge(Resource::Work, 1)?;
        let value = self.shape.value();
        if usize::try_from(embed.0)
            .ok()
            .and_then(|i| value.embeds.get(i))
            .is_none()
        {
            return Err(Error::Embed(embed));
        }
        Ok(ResolvedInline { value, embed, text })
    }

    /// Each shared-node occurrence contributes its text. Notes are selected by
    /// policy; foreign text is required only for occurrences selected for output.
    /// The complete output is returned only on success. A stop retains no prefix.
    pub fn render(
        &self,
        policy: AnnotationPolicy,
        resolved: &[ResolvedInline<'_, '_>],
        b: &mut Budget,
    ) -> Result<String, Error> {
        b.poll()?;
        let value = self.shape.value();
        let count = value.embeds.len();
        let bytes = count
            .checked_mul(core::mem::size_of::<Option<&str>>())
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::Work, count as u64)?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut texts = vec![None; count];
        for supplied in resolved {
            b.charge(Resource::Work, 1)?;
            if !core::ptr::eq(value, supplied.value) {
                return Err(Error::WrongScope);
            }
            let slot = usize::try_from(supplied.embed.0)
                .ok()
                .and_then(|i| texts.get_mut(i))
                .ok_or(Error::Embed(supplied.embed))?;
            if slot.is_some() {
                return Err(Error::Duplicate(supplied.embed));
            }
            *slot = Some(supplied.text);
        }
        let root = match value.root {
            Root::Sentence(r) => r.0,
            Root::Inline(r) => r.0,
        };
        let mut pending = Vec::new();
        push(&mut pending, Action::Node(root, 1), b)?;
        let mut output = String::new();
        let base = b.current_depth();
        while let Some(action) = pending.pop() {
            b.charge(Resource::Work, 1)?;
            match action {
                Action::Text(text) => append(&mut output, text, b)?,
                Action::Node(id, depth) => {
                    b.with_depth_at_least::<_, Error>(base.saturating_add(depth), |b| {
                        let kind = usize::try_from(id)
                            .ok()
                            .and_then(|i| value.nodes.get(i))
                            .ok_or(check::Error::Reference(id))?;
                        let next = depth.saturating_add(1);
                        match kind {
                            Kind::Sentence { inlines } | Kind::Concat { inlines } => {
                                for child in inlines.iter().rev() {
                                    push(&mut pending, Action::Node(child.0, next), b)?;
                                }
                            }
                            Kind::Text { text } | Kind::Code { text } => {
                                append(&mut output, text, b)?
                            }
                            Kind::Ruby { base, reading } => {
                                if policy != AnnotationPolicy::BaseOnly {
                                    push(&mut pending, Action::Text("]"), b)?;
                                    push(&mut pending, Action::Node(reading.0, next), b)?;
                                    push(&mut pending, Action::Text("["), b)?;
                                }
                                push(&mut pending, Action::Node(base.0, next), b)?;
                            }
                            Kind::InlineAnno { base, notes } => {
                                if policy == AnnotationPolicy::WithAllNotes {
                                    push(&mut pending, Action::Text("}"), b)?;
                                    for (i, note) in notes.iter().enumerate().rev() {
                                        push(&mut pending, Action::Node(note.0, next), b)?;
                                        if i > 0 {
                                            push(&mut pending, Action::Text("/"), b)?;
                                        }
                                    }
                                    push(&mut pending, Action::Text("{"), b)?;
                                }
                                push(&mut pending, Action::Node(base.0, next), b)?;
                            }
                            Kind::Emphasis { inline }
                            | Kind::Strong { inline }
                            | Kind::ExternalLink { label: inline, .. } => {
                                push(&mut pending, Action::Node(inline.0, next), b)?;
                            }
                            Kind::Break => append(&mut output, "\n", b)?,
                            Kind::ForeignInline { syntax } => {
                                let text = usize::try_from(syntax.0)
                                    .ok()
                                    .and_then(|i| texts.get(i))
                                    .and_then(|text| *text)
                                    .ok_or(Error::Unresolved(*syntax))?;
                                append(&mut output, text, b)?;
                            }
                        }
                        Ok(())
                    })?
                }
            }
        }
        Ok(output)
    }
}

enum Action<'a> {
    Node(u64, u64),
    Text(&'a str),
}
fn push<'a>(
    pending: &mut Vec<Action<'a>>,
    action: Action<'a>,
    b: &mut Budget,
) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Action<'a>>() as u64,
    )?;
    pending.push(action);
    Ok(())
}
fn append(out: &mut String, text: &str, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, text.len() as u64)?;
    b.charge(Resource::OutputBytes, text.len() as u64)?;
    b.charge(Resource::AllocationUnits, text.len() as u64)?;
    out.push_str(text);
    Ok(())
}
