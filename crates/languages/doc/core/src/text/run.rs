use super::*;
use alloc::string::String;
use nepl3_core::budget::StopReason;
pub(super) enum Failure {
    Stopped(StopReason),
    Invalid(PlainTextFailure),
}
impl From<StopReason> for Failure {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<PlainTextFailure> for Failure {
    fn from(e: PlainTextFailure) -> Self {
        Self::Invalid(e)
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
) -> Result<(), Failure> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Action<'a>>() as u64,
    )?;
    pending.push(action);
    Ok(())
}
fn append(out: &mut String, text: &str, b: &mut Budget) -> Result<(), Failure> {
    b.charge(Resource::Work, text.len() as u64)?;
    b.charge(Resource::OutputBytes, text.len() as u64)?;
    b.charge(Resource::AllocationUnits, text.len() as u64)?;
    out.push_str(text);
    Ok(())
}
pub(super) fn execute<'a>(
    prepared: &'a PreparedText<'a>,
    sentence: SentenceRef,
    policy: AnnotationPolicy,
    resolved: &'a [ResolvedInlineText],
    b: &mut Budget,
) -> Result<String, Failure> {
    b.poll()?;
    for (index, value) in resolved.iter().enumerate() {
        b.charge(Resource::Work, 33)?;
        if value.document_digest != prepared.identity.document_digest {
            return Err(PlainTextFailure::InvalidResolution {
                entry: index as u64,
                reason: ResolutionMismatch::Document,
            }
            .into());
        }
        let mut found = None;
        for target in &prepared.identity.embeds {
            b.charge(Resource::Work, 1)?;
            if target.embed == value.embed {
                found = Some(target);
                break;
            }
        }
        let target = found.ok_or(PlainTextFailure::InvalidResolution {
            entry: index as u64,
            reason: ResolutionMismatch::Embed,
        })?;
        b.charge(Resource::Work, 32)?;
        if target.guest_digest != value.guest_digest {
            return Err(PlainTextFailure::InvalidResolution {
                entry: index as u64,
                reason: ResolutionMismatch::Guest,
            }
            .into());
        }
        for prior in &resolved[..index] {
            b.charge(Resource::Work, 1)?;
            if prior.embed == value.embed {
                return Err(PlainTextFailure::InvalidResolution {
                    entry: index as u64,
                    reason: ResolutionMismatch::Duplicate,
                }
                .into());
            }
        }
    }
    let doc = &prepared.document.value;
    if !matches!(
        usize::try_from(sentence.0)
            .ok()
            .and_then(|index| doc.nodes.get(index))
            .map(|n| &n.kind),
        Some(DocKind::Sentence { .. })
    ) {
        return Err(PlainTextFailure::ExpectedSentence { node: sentence.0 }.into());
    }
    let base = b.current_depth();
    let mut pending = Vec::new();
    push(&mut pending, Action::Node(sentence.0, 1), b)?;
    let mut out = String::new();
    while let Some(action) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        match action {
            Action::Text(text) => append(&mut out, text, b)?,
            Action::Node(index, depth) => {
                b.with_depth_at_least(base.saturating_add(depth), |b| -> Result<(), Failure> {
                    let next = depth.saturating_add(1);
                    let node = usize::try_from(index)
                        .ok()
                        .and_then(|index| doc.nodes.get(index))
                        .ok_or(PlainTextFailure::ExpectedSentence { node: index })?;
                    match &node.kind {
                        DocKind::Sentence { inlines } | DocKind::Concat { inlines } => {
                            for child in inlines.iter().rev() {
                                push(&mut pending, Action::Node(child.0, next), b)?;
                            }
                        }
                        DocKind::Text { text } | DocKind::InlineCode { text } => {
                            append(&mut out, text, b)?
                        }
                        DocKind::Ruby { base, reading } => {
                            if policy != AnnotationPolicy::BaseOnly {
                                push(&mut pending, Action::Text("]"), b)?;
                                push(&mut pending, Action::Node(reading.0, next), b)?;
                                push(&mut pending, Action::Text("["), b)?;
                            }
                            push(&mut pending, Action::Node(base.0, next), b)?;
                        }
                        DocKind::Anno { base, notes } => {
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
                        DocKind::Break => append(&mut out, "\n", b)?,
                        DocKind::Strong { inline }
                        | DocKind::Emphasis { inline }
                        | DocKind::Anchor { label: inline, .. }
                        | DocKind::Reference { label: inline, .. }
                        | DocKind::Link { label: inline, .. } => {
                            push(&mut pending, Action::Node(inline.0, next), b)?
                        }
                        DocKind::InlineImage { alt, .. } => {
                            push(&mut pending, Action::Node(alt.0, next), b)?
                        }
                        DocKind::InlineMath { syntax } => {
                            let mut found = None;
                            for value in resolved {
                                b.charge(Resource::Work, 1)?;
                                if value.embed == *syntax {
                                    found = Some(value.text.as_str());
                                    break;
                                }
                            }
                            append(
                                &mut out,
                                found
                                    .ok_or(PlainTextFailure::UnresolvedEmbed { embed: *syntax })?,
                                b,
                            )?;
                        }
                        _ => return Err(PlainTextFailure::ExpectedSentence { node: index }.into()),
                    }
                    Ok(())
                })?
            }
        }
    }
    Ok(out)
}
