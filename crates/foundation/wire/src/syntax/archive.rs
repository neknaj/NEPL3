//! Opt-in direct-foreign syntax transport. Opaque token payloads stay unchanged.
use super::*;
use crate::source::archive::tables::{Capture, Restore};
use nepl3_core::schema::archive::syntax as contract;

fn archive_type() -> nepl3_core::schema::TypeDescriptor {
    nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
        package: contract::PACKAGE.into(),
        revision: contract::REVISION,
        name: contract::ROOT.into(),
    })
}
/// Encode native SyntaxBundle sources through an explicit archive. Only direct
/// ForeignSyntax fields participate; opaque payloads are not rewritten. Existing
/// semantic identities must still use the legacy native encoding, not these bytes.
pub fn encode(
    bundle: &SyntaxBundle,
    syntax_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    crate::source::archive::fields::check(foundation, registry, false, budget)?;
    bundle.validate_with_sources(registry, budget, admission)?;
    let mut capture = Capture::default();
    let skeleton = bundle_value_with_sources(
        bundle,
        foundation,
        registry,
        admission,
        budget,
        &mut |sources, _, admission, b| capture.take(sources, admission, b),
    )?;
    // Checked structural validation accepts empty declaration lists. Semantic
    // validation was performed on the original native graph above.
    let skeleton = crate::encode_checked(&skeleton, &expected("SyntaxBundle"), registry, budget)?;
    let sources = capture.finish(source_archive, foundation, admission, budget)?;
    let value = record(
        syntax_archive,
        contract::ROOT,
        [sources, NdfValue::Bytes(skeleton)],
        budget,
    )?;
    crate::encode_checked(&value, &archive_type(), registry, budget)
}

/// Restore each bundle's own declarations before final semantic validation.
/// Neither source admission nor a source used in another bundle supplies scope.
pub fn decode(
    bytes: &[u8],
    syntax_archive: &SchemaRef,
    source_archive: &SchemaRef,
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<SyntaxBundle, WireError> {
    crate::source::archive::fields::check(foundation, registry, false, budget)?;
    let value = crate::decode_checked(bytes, &archive_type(), registry, budget)?;
    let root = fields(value.value(), syntax_archive, contract::ROOT, 2)?;
    let mut restore = Restore::new(&root[0], source_archive, foundation, admission, budget)?;
    let NdfValue::Bytes(skeleton) = &root[1] else {
        return Err(WireError::InvalidType);
    };
    let skeleton = crate::decode_checked(skeleton, &expected("SyntaxBundle"), registry, budget)?;
    let bundle = bundle_from_with_sources(
        skeleton.value(),
        foundation,
        registry,
        admission,
        budget,
        &mut |value, _, _, b| restore.take(value, b),
    )?;
    restore.finish(budget)?;
    bundle.validate_with_sources(registry, budget, admission)?;
    budget.poll()?;
    Ok(bundle)
}
