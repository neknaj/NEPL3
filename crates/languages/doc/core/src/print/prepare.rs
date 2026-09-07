use super::*;
pub(super) struct Guest<'a> {
    pub language: GuestLanguage,
    pub text: &'a str,
}
pub(super) fn category(language: GuestLanguage) -> &'static str {
    match language {
        GuestLanguage::Math => "Expr",
        GuestLanguage::Circuit => "Design",
        GuestLanguage::Grammar => "Root",
        GuestLanguage::Doc => "Article",
    }
}
pub(super) fn check<'a>(
    request: &'a PrintRequest,
    identity: &PrintIdentity,
    b: &mut Budget,
) -> Result<Vec<Guest<'a>>, Failure> {
    for (index, binding) in request.bindings.iter().enumerate() {
        b.charge(Resource::Work, (binding.category.len() + 8) as u64)?;
        if binding.category != category(binding.language) {
            return Err(PrintFailure::InvalidBinding {
                binding: index as u64,
            }
            .into());
        }
        for prior in &request.bindings[..index] {
            b.charge(
                Resource::Work,
                (binding.schema.package.len() + prior.schema.package.len() + 33) as u64,
            )?;
            if binding.schema == prior.schema || binding.language == prior.language {
                return Err(PrintFailure::ConflictingBinding {
                    binding: index as u64,
                }
                .into());
            }
        }
    }
    for (index, guest) in request.guests.iter().enumerate() {
        b.charge(Resource::Work, 32)?;
        let failure = |reason| PrintFailure::InvalidGuest {
            entry: index as u64,
            reason,
        };
        if guest.document_digest != identity.document_digest {
            return Err(failure(PrintMismatch::Document).into());
        }
        let target = usize::try_from(guest.embed.0)
            .ok()
            .and_then(|i| identity.guests.get(i))
            .ok_or(failure(PrintMismatch::Embed))?;
        b.charge(Resource::Work, 32)?;
        if target.guest_digest != guest.guest_digest {
            return Err(failure(PrintMismatch::Guest).into());
        }
        for prior in &request.guests[..index] {
            b.charge(Resource::Work, 1)?;
            if prior.embed == guest.embed {
                return Err(failure(PrintMismatch::Duplicate).into());
            }
        }
    }
    let mut out = Vec::new();
    for (index, embed) in request.document.value.embeds.iter().enumerate() {
        let id = EmbedRef(index as u64);
        let mut found = None;
        for binding in &request.bindings {
            b.charge(
                Resource::Work,
                (binding.schema.package.len() + embed.closure.syntax.schema.package.len() + 33)
                    as u64,
            )?;
            if binding.schema == embed.closure.syntax.schema {
                found = Some(binding);
                break;
            }
        }
        let binding = found.ok_or(PrintFailure::MissingBinding { embed: id })?;
        b.charge(
            Resource::Work,
            (binding.category.len() + embed.closure.syntax.category.len()) as u64,
        )?;
        let compatible = match embed.kind {
            EmbedKind::InlineMath | EmbedKind::DisplayMath => {
                binding.language == GuestLanguage::Math
            }
            EmbedKind::CircuitFigure => binding.language == GuestLanguage::Circuit,
            EmbedKind::Code | EmbedKind::Guest => true,
        };
        if !compatible || binding.category != embed.closure.syntax.category {
            return Err(PrintFailure::GuestCategory { embed: id }.into());
        }
        let mut text = None;
        for guest in &request.guests {
            b.charge(Resource::Work, 1)?;
            if guest.embed == id {
                text = Some(guest.text.as_str());
                break;
            }
        }
        let text = text.ok_or(PrintFailure::UnresolvedGuest { embed: id })?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Guest<'a>>() as u64,
        )?;
        out.push(Guest {
            language: binding.language,
            text,
        });
    }
    Ok(out)
}
