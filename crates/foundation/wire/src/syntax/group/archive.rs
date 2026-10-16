//! Explicit grouped ownership transport with one source table.
use super::*;
use crate::source::archive::tables::{Capture, Restore};
use nepl3_core::schema::archive::group as contract;
fn archive_type() -> nepl3_core::schema::TypeDescriptor {
    nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
        package: contract::PACKAGE.into(),
        revision: contract::REVISION,
        name: contract::ROOT.into(),
    })
}
/// Preserve owner/provenance/entry order; share only checked source storage.
/// The resulting transport bytes are never a replacement for v1 guest identity.
pub fn encode(
    group: &ForeignClosureGroup,
    group_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let mut capture = Capture::default();
    let skeleton = to_value_with_sources(
        group,
        foundation,
        registry,
        admission,
        b,
        &mut |sources, _, admission, b| capture.take(sources, admission, b),
    )?;
    let skeleton = crate::encode_checked(&skeleton, &expected("ForeignClosureGroup"), registry, b)?;
    let sources = capture.finish(source_archive, foundation, admission, b)?;
    let value = record(
        group_archive,
        contract::ROOT,
        [sources, NdfValue::Bytes(skeleton)],
        b,
    )?;
    crate::encode_checked(&value, &archive_type(), registry, b)
}
/// Restore provenance sets first and then each entry's child-first bundle sets.
/// Every component resolves against its own declarations before full validation.
pub fn decode(
    bytes: &[u8],
    group_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ForeignClosureGroup, WireError> {
    let value = crate::decode_checked(bytes, &archive_type(), registry, b)?;
    let root = fields(value.value(), group_archive, contract::ROOT, 2)?;
    let mut restore = Restore::new(&root[0], source_archive, foundation, admission, b)?;
    let NdfValue::Bytes(skeleton) = &root[1] else {
        return Err(WireError::InvalidType);
    };
    let skeleton = crate::decode_checked(skeleton, &expected("ForeignClosureGroup"), registry, b)?;
    let group = from_value_with_sources(
        skeleton.value(),
        foundation,
        registry,
        admission,
        b,
        &mut |value, _, _, b| restore.take(value, b),
    )?;
    restore.finish(b)?;
    b.poll()?;
    Ok(group)
}
