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
        if value.policy != policy {
            return Err(PlainTextFailure::InvalidResolution {
                entry: index as u64,
                reason: ResolutionMismatch::Policy,
            }
            .into());
        }
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
    let Some(DocKind::Sentence { syntax }) = usize::try_from(sentence.0)
        .ok()
        .and_then(|index| doc.nodes.get(index))
        .map(|n| &n.kind)
    else {
        return Err(PlainTextFailure::ExpectedSentence { node: sentence.0 }.into());
    };
    let mut out = String::new();
    for value in resolved {
        b.charge(Resource::Work, 1)?;
        if value.embed == *syntax {
            append(&mut out, &value.text, b)?;
            return Ok(out);
        }
    }
    Err(PlainTextFailure::UnresolvedEmbed { embed: *syntax }.into())
}
