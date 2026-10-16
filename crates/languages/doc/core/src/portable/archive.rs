//! Explicit Doc archive transport. V1 preparation/rendering routes stay unchanged.
use super::*;
use crate::{
    model::{DocEmbeds, DocValue},
    schema::archive as contract,
};
use nepl3_core::{
    syntax::group::ForeignClosureGroup,
    value_codec::{GroupEncodingError, archive::FoundationGroupArchiveCodec},
};

/// Generated transport and compact guest commitments, not an externally reusable proof.
pub struct DocumentArchiveEncoding {
    pub value: NdfValue,
    pub guest_components: alloc::vec::Vec<nepl3_core::source::Digest>,
}
fn archive_schema<'a, E>(
    r: &'a SchemaRegistry,
    b: &mut Budget,
) -> Result<&'a SchemaRef, PortableError<E>> {
    r.selected_descriptor_with_budget(contract::PACKAGE, contract::REVISION, b)?
        .map(|(reference, _)| reference)
        .ok_or_else(|| SchemaError::UnknownSchema.into())
}
fn check_archive<E>(
    v: &NdfValue,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        (contract::PACKAGE.len() + contract::ROOT.len()) as u64,
    )?;
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: contract::PACKAGE.into(),
            revision: contract::REVISION,
            name: contract::ROOT.into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
/// Encode one native document with shared group archive bytes and v2 components.
/// The skeleton is structurally checked Doc v1 data whose group is empty; it is
/// not independently valid native Doc syntax. Original structure and guest depths
/// are checked before the archive is returned.
pub fn to_value<C: FoundationGroupArchiveCodec>(
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocumentArchiveEncoding, PortableError<C::Error>> {
    to_value_with_structure(document, r, c, b).map(|(encoding, _)| encoding)
}

/// Retain the native operation-local structure proof for consuming page routes.
/// The public archive transport remains data and carries no reusable proof.
pub(crate) fn to_value_with_structure<'a, C: FoundationGroupArchiveCodec>(
    document: &'a DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<
    (
        DocumentArchiveEncoding,
        crate::check::ValidatedDocumentSyntax<'a>,
    ),
    PortableError<C::Error>,
> {
    let mut group_archive = None;
    let (skeleton, structure) = encode_structure_using_group(document, r, c, b, |c, depths, b| {
        let encoded = c
            .encode_compact_group_at_depths(&document.value.embeds.group, depths, r, b)
            .map_err(|error| match error {
                GroupEncodingError::Structure(error) => {
                    PortableError::from(StructureError::from(error))
                }
                GroupEncodingError::Codec(error) => boundary(error),
            })?;
        b.charge(Resource::Work, 1)?;
        if encoded.entry_digests.len() != document.value.embeds.group.entries.len() {
            return Err(PortableError::Shape);
        }
        group_archive = Some(encoded);
        c.encode_foreign_group(
            &ForeignClosureGroup {
                provenances: alloc::vec![],
                environments: alloc::vec![],
                entries: alloc::vec![],
            },
            b,
        )
        .map_err(boundary)
    })?;
    // Validate unchanged fields under their selected descriptor before wrapping.
    check(r, &skeleton, b)?;
    let group_archive = group_archive.ok_or(PortableError::Shape)?;
    let value = record(
        archive_schema(r, b)?,
        contract::ROOT,
        [skeleton, NdfValue::Bytes(group_archive.archive)],
        b,
    )?;
    check_archive(&value, r, b)?;
    b.poll()?;
    Ok((
        DocumentArchiveEncoding {
            value,
            guest_components: group_archive.entry_digests,
        },
        structure,
    ))
}
/// Decode only this explicit archive envelope. Restore the group without expanding
/// it back into v1 NDF, retain each declaration scope, then check consuming Doc depths.
pub fn from_value<C: FoundationGroupArchiveCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocumentSyntax, PortableError<C::Error>> {
    check_archive(input, r, b)?;
    let root = fields(input, archive_schema(r, b)?, contract::ROOT, 2)?;
    check(r, &root[0], b)?;
    let NdfValue::Bytes(group_bytes) = &root[1] else {
        return Err(PortableError::Shape);
    };
    let group = c.decode_group_archive(group_bytes, b).map_err(boundary)?;
    let s = schema(r)?;
    let f = fields(&root[0], s, "DocumentSyntax", 5)?;
    let sources = c.decode_sources(&f[1], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    for source in &sources {
        store
            .insert_with_budget(source.clone_with_budget(b)?, b)
            .map_err(StructureError::from)?;
    }
    let mut scoped = c.scoped(&store);
    let value = DocValue::read_with_embeds(&f[0], s, &mut scoped, b, |v, c, b| {
        DocEmbeds::read_with_group(v, s, c, b, |placeholder, c, b| {
            let empty = c.decode_foreign_group(placeholder, b).map_err(boundary)?;
            if !empty.provenances.is_empty()
                || !empty.environments.is_empty()
                || !empty.entries.is_empty()
            {
                return Err(PortableError::Shape);
            }
            Ok(group)
        })
    })?;
    let origins = scoped.decode_origins(&f[2], b).map_err(boundary)?;
    let source_maps: alloc::vec::Vec<nepl3_core::origin::Mapping> =
        Value::read(&f[4], s, &mut scoped, b)?;
    let views = {
        let mut mapped = scoped.scoped_with_mappings(&store, &source_maps);
        Value::read(&f[3], s, &mut mapped, b)?
    };
    let output = DocumentSyntax {
        value,
        sources,
        origins,
        views,
        source_maps,
    };
    output.validate_structure(r, b, scoped.source_admission())?;
    b.poll()?;
    Ok(output)
}

pub mod pages;
