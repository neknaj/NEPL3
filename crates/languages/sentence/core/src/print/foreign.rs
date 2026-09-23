use super::{Error, emit_prefix};
use crate::{
    check,
    model::{EmbedRef, SentenceValue},
};
use alloc::{string::String, vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
};

/// Immutable Sentence and validated foreign closures selected for prefix output.
/// This proof covers the input. The host owns guest surface selection and printing.
pub struct PreparedPrint<'a> {
    shape: check::CheckedShape<'a>,
    _registry: &'a SchemaRegistry,
}

/// Source for a complete foreign Inline form, bound to one immutable input.
/// Its grammar and semantic correspondence are checked by the host's selected
/// adapter; constructing this value does not certify the supplied source.
pub struct ResolvedInlineSource<'a, 'source> {
    value: &'a SentenceValue,
    embed: EmbedRef,
    source: &'source str,
}

pub fn prepare<'a>(
    value: &'a SentenceValue,
    registry: &'a SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<PreparedPrint<'a>, Error> {
    let shape = value.validate_shape(b)?;
    shape.validate_foreign(registry, b, admission)?;
    Ok(PreparedPrint {
        shape,
        _registry: registry,
    })
}

impl<'a> PreparedPrint<'a> {
    pub fn resolve<'source>(
        &self,
        embed: EmbedRef,
        source: &'source str,
        b: &mut Budget,
    ) -> Result<ResolvedInlineSource<'a, 'source>, Error> {
        b.charge(Resource::Work, 1)?;
        let value = self.shape.value();
        if usize::try_from(embed.0)
            .ok()
            .and_then(|i| value.embeds.get(i))
            .is_none()
        {
            return Err(Error::Embed(embed));
        }
        Ok(ResolvedInlineSource {
            value,
            embed,
            source,
        })
    }

    /// Print the standard Sentence tree and host-prepared foreign forms in
    /// occurrence order. Output copying is charged even when a guest printer
    /// already charged for producing its source. Stops return no partial output.
    pub fn render(
        &self,
        resolved: &[ResolvedInlineSource<'_, '_>],
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
        let mut sources = vec![None; count];
        for supplied in resolved {
            b.charge(Resource::Work, 1)?;
            if !core::ptr::eq(value, supplied.value) {
                return Err(Error::WrongScope);
            }
            let slot = usize::try_from(supplied.embed.0)
                .ok()
                .and_then(|i| sources.get_mut(i))
                .ok_or(Error::Embed(supplied.embed))?;
            if slot.is_some() {
                return Err(Error::Duplicate(supplied.embed));
            }
            *slot = Some(supplied.source);
        }
        // The source and all graph edges remain borrowed from the checked arena.
        emit_prefix(value, Some(&sources), b)
    }
}
