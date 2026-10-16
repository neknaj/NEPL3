//! Explicit archive identity and link resolution; old resolve keeps its v1 identity.
use super::*;
use nepl3_core::value_codec::archive::FoundationGroupArchiveCodec;
pub const SET_DOMAIN: &[u8] = b"NEPL3.Doc.Archive.Pages.v1\0";
pub const DOCUMENT_DOMAIN: &[u8] = b"NEPL3.Doc.Archive.Document.v1\0";
pub const GUEST_DOMAIN: &[u8] = b"NEPL3.Doc.Archive.Guest.v1\0";
/// Resolve exactly the archived PageSet, retaining original native structure proofs.
/// Registrations are checked before encoding, and all work shares one Budget.
pub fn resolve<'a, C: FoundationGroupArchiveCodec>(
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<CheckedPages<'a>, PageError<'a, C::Error>> {
    super::resolve_using(
        set,
        r,
        c,
        b,
        PageIdentityProfile::ArchiveV1,
        portable::archive::pages::to_value_with_structures,
    )
    .map_err(|error| match error {
        PageError::Boundary(portable::PortableError::Stopped(reason)) => PageError::Stopped(reason),
        error => error,
    })
}
