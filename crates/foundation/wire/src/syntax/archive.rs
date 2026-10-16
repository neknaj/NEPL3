//! Opt-in direct-foreign syntax transport. Opaque token payloads stay unchanged.
use super::*;
use nepl3_core::{
    schema::archive::syntax as contract,
    source::{SourceRef, SourceSnapshot},
};

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
    bundle.validate_with_sources(registry, budget, admission)?;
    let mut union = SourceStore::default();
    let mut memberships: Vec<Vec<SourceRef>> = Vec::new();
    let skeleton = bundle_value_with_sources(
        bundle,
        foundation,
        registry,
        admission,
        budget,
        &mut |sources, _, admission, b| {
            let ordered = ordered_sources(sources, b)?;
            let mut members = Vec::new();
            for (reference, source) in ordered {
                admission.admit_existing(source, b)?;
                union.insert_ref_with_budget(source, b)?;
                // The owned reference was charged when ordered_sources built it;
                // pay for its new container slot before moving it.
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<SourceRef>() as u64,
                )?;
                members.push(reference);
            }
            push(&mut memberships, members, b)?;
            Ok(NdfValue::List(Vec::new()))
        },
    )?;
    // Checked structural validation accepts empty declaration lists. Semantic
    // validation was performed on the original native graph above.
    let skeleton = crate::encode_checked(&skeleton, &expected("SyntaxBundle"), registry, budget)?;
    let ordered = ordered_sources(union.snapshots(), budget)?;
    let mut sources = Vec::<SourceSnapshot>::new();
    for (_, source) in ordered {
        sources.push(source.clone_with_budget(budget)?);
    }
    let sources = crate::source::archive::to_value(
        &sources,
        &memberships,
        source_archive,
        foundation,
        admission,
        budget,
    )?;
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
    let value = crate::decode_checked(bytes, &archive_type(), registry, budget)?;
    let root = fields(value.value(), syntax_archive, contract::ROOT, 2)?;
    let sources = crate::source::archive::from_value(
        &root[0],
        source_archive,
        foundation,
        admission,
        budget,
    )?;
    let NdfValue::Bytes(skeleton) = &root[1] else {
        return Err(WireError::InvalidType);
    };
    let skeleton = crate::decode_checked(skeleton, &expected("SyntaxBundle"), registry, budget)?;
    let mut members = sources.into_iter();
    let bundle = bundle_from_with_sources(
        skeleton.value(),
        foundation,
        registry,
        admission,
        budget,
        &mut |value, _, _, b| {
            b.charge(Resource::Work, 1)?;
            if !list(value)?.is_empty() {
                return Err(WireError::NonCanonical);
            }
            let snapshots = members.next().ok_or(WireError::InvalidLength)?;
            let mut local = SourceStore::default();
            for source in &snapshots {
                local.insert_ref_with_budget(source, b)?;
            }
            Ok((snapshots, local))
        },
    )?;
    if members.next().is_some() {
        return Err(WireError::InvalidLength);
    }
    bundle.validate_with_sources(registry, budget, admission)?;
    budget.poll()?;
    Ok(bundle)
}
