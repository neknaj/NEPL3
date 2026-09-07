//! Pure source printing. Guest text is explicit host input; retained source
//! spans alone are not proof that a guest's current syntax matches its bytes.
mod model;
mod output;
mod prepare;
mod run;
use crate::{
    model::*,
    portable::{self, PortableError},
};
use alloc::vec::Vec;
pub use model::*;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

pub const DOCUMENT_DOMAIN: &[u8] = b"NEPL3.Doc.Print.Document.v1\0";
pub const GUEST_DOMAIN: &[u8] = b"NEPL3.Doc.Print.Guest.v1\0";

/// Retrieve retained bytes only. This does not certify correspondence between
/// the current guest syntax and that source, and `print` never calls it as a
/// substitute for a host guest printer. A source-less root returns None.
pub fn original_guest_source<'a>(
    document: &'a DocumentSyntax,
    embed: EmbedRef,
    r: &SchemaRegistry,
    b: &mut Budget,
    a: &mut nepl3_core::source::SourceAdmission,
) -> Result<Option<&'a str>, crate::check::StructureError> {
    document.validate_structure(r, b, a)?;
    let guest = usize::try_from(embed.0)
        .ok()
        .and_then(|i| document.value.embeds.get(i))
        .ok_or(crate::check::ShapeError::Embed(embed.0))?;
    let bundle = &guest.closure.syntax.bundle;
    let root = bundle.node(bundle.root)?;
    let Some(cover) = &root.cover else {
        return Ok(None);
    };
    for source in &bundle.sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + cover.snapshot_ref().source.0.len() + 33) as u64,
        )?;
        if source.identity() == cover.snapshot_ref() {
            return Ok(Some(source.slice(cover)?));
        }
    }
    Err(nepl3_core::source::SourceError::MissingSnapshot.into())
}

/// Identity data for requesting a host guest printer. Actual print recomputes
/// it under its own operation Budget; this does not transfer paid validation.
pub fn identity<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PrintIdentity, PortableError<C::Error>> {
    let value = portable::to_value(document, r, c, b)?;
    let document_digest = c
        .canonical_value_digest(DOCUMENT_DOMAIN, &value, b)
        .map_err(boundary)?;
    let mut guests = Vec::new();
    for (index, embed) in document.value.embeds.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let value = c
            .encode_foreign_closure(&embed.closure, b)
            .map_err(boundary)?;
        let guest_digest = c
            .canonical_value_digest(GUEST_DOMAIN, &value, b)
            .map_err(boundary)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<PrintGuestTarget>() as u64,
        )?;
        guests.push(PrintGuestTarget {
            embed: EmbedRef(index as u64),
            guest_digest,
        });
    }
    Ok(PrintIdentity {
        document_digest,
        guests,
    })
}
fn boundary<E: FoundationCodecError>(e: E) -> PortableError<E> {
    match e.stop_reason() {
        Some(s) => PortableError::Stopped(s),
        None => PortableError::Foundation(e),
    }
}
enum Failure {
    Stopped(StopReason),
    Invalid(PrintFailure),
    Shape(crate::check::ShapeError),
}
impl From<crate::check::ShapeError> for Failure {
    fn from(error: crate::check::ShapeError) -> Self {
        match error {
            crate::check::ShapeError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Shape(error),
        }
    }
}
impl From<StopReason> for Failure {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<PrintFailure> for Failure {
    fn from(e: PrintFailure) -> Self {
        Self::Invalid(e)
    }
}

pub fn print<C: FoundationValueCodec>(
    request: &PrintRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PrintReply, PortableError<C::Error>> {
    let result = match identity(&request.document, r, c, b) {
        Ok(identity) => {
            prepare::check(request, &identity, b).and_then(|guests| run::print(request, &guests, b))
        }
        Err(PortableError::Stopped(s)) => Err(Failure::Stopped(s)),
        Err(e) => return Err(e),
    };
    let outcome = match result {
        Ok(artifact) => PrintOutcome::Complete { artifact },
        Err(Failure::Invalid(error)) => PrintOutcome::Invalid { error },
        Err(Failure::Shape(error)) => {
            return Err(PortableError::Structure(
                crate::check::StructureError::Shape(error),
            ));
        }
        Err(Failure::Stopped(reason)) => PrintOutcome::Stopped {
            reason: b.stop(reason),
        },
    };
    Ok(PrintReply {
        outcome,
        report: nepl3_core::diagnostic::Report {
            usage: b.usage(),
            ..Default::default()
        },
    })
}
