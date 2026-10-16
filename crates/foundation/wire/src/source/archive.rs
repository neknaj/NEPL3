//! Opt-in source-set archive. No syntax graph or opaque payload is rewritten.
pub(crate) mod fields;
pub(crate) mod tables;
use super::*;
use crate::{
    boundary::sequence,
    view::{collect, list},
};
use nepl3_core::{
    schema::archive as contract,
    source::archive::{ArchiveSourceError, ArchiveSources},
};

fn expected_archive() -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: contract::PACKAGE.into(),
        revision: contract::REVISION,
        name: contract::ROOT.into(),
    })
}
fn archive_error(error: ArchiveSourceError) -> WireError {
    match error {
        ArchiveSourceError::Source(source) => source.into(),
        ArchiveSourceError::NonCanonical | ArchiveSourceError::UnusedSource => {
            WireError::NonCanonical
        }
        ArchiveSourceError::MissingMembership => SourceError::MissingSnapshot.into(),
    }
}

/// Encode one canonical table and ordered local membership lists. The order of
/// the lists is semantic (caller-assigned source-set positions); every individual
/// list and the table must be canonical. All table entries must be referenced.
/// This is not a SyntaxBundle or ForeignClosureGroup codec.
pub fn encode(
    sources: &[SourceSnapshot],
    memberships: &[Vec<SourceRef>],
    archive_schema: &SchemaRef,
    foundation_schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let value = to_value(
        sources,
        memberships,
        archive_schema,
        foundation_schema,
        admission,
        budget,
    )?;
    encode_checked(&value, &expected_archive(), registry, budget)
}
pub(crate) fn to_value(
    sources: &[SourceSnapshot],
    memberships: &[Vec<SourceRef>],
    archive_schema: &SchemaRef,
    foundation_schema: &SchemaRef,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let mut table = ArchiveSources::new(sources, admission, budget).map_err(archive_error)?;
    let members = sequence(memberships, budget, |members, budget| {
        // Resolve through the table to reject dangling, wrong-digest, duplicate
        // or reordered memberships; a global admission ledger grants no scope.
        table.resolve(members, budget).map_err(archive_error)?;
        sequence(members, budget, |member, budget| {
            source_ref(member, foundation_schema, budget)
        })
    })?;
    table.finish(budget).map_err(archive_error)?;
    let sources = sources_value(sources, foundation_schema, admission, budget)?;
    let value = record(archive_schema, contract::ROOT, [sources, members], budget)?;
    Ok(value)
}

/// Decode the table once and resolve checked native handles for each local set.
/// Returned sets are self-contained sources only. The caller must validate its
/// graph, origins and mappings against that set, never against the whole table.
pub fn decode(
    bytes: &[u8],
    archive_schema: &SchemaRef,
    foundation_schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<Vec<SourceSnapshot>>, WireError> {
    let value = decode_checked(bytes, &expected_archive(), registry, budget)?;
    from_value(
        value.value(),
        archive_schema,
        foundation_schema,
        admission,
        budget,
    )
}
pub(crate) fn from_value(
    value: &NdfValue,
    archive_schema: &SchemaRef,
    foundation_schema: &SchemaRef,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<Vec<SourceSnapshot>>, WireError> {
    let root = fields(value, archive_schema, contract::ROOT, 2)?;
    let sources = sources_from(&root[0], foundation_schema, admission, budget)?;
    let mut table = ArchiveSources::new(&sources, admission, budget).map_err(archive_error)?;
    let result = collect(list(&root[1])?, budget, |members, budget| {
        let references = collect(list(members)?, budget, |v, budget| {
            reference(v, foundation_schema, budget)
        })?;
        table.resolve(&references, budget).map_err(archive_error)
    })?;
    table.finish(budget).map_err(archive_error)?;
    budget.poll()?;
    Ok(result)
}
