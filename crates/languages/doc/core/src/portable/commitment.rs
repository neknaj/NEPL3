//! Identity-only private preimages. These are never archive transport values.
use super::*;
use crate::pages::{PageLinkPlan, PageSet, push};
use alloc::vec::Vec;
use nepl3_core::{
    syntax::group::ForeignClosureGroup,
    value_codec::{GroupEncodingError, commitment::FoundationGroupCommitmentCodec},
};
struct DocumentCommitment<'a> {
    value: NdfValue,
    structure: crate::check::ValidatedDocumentSyntax<'a>,
    components: Vec<nepl3_core::source::Digest>,
}
fn document<'a, C: FoundationGroupCommitmentCodec>(
    doc: &'a DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocumentCommitment<'a>, PortableError<C::Error>> {
    let mut commitments = None;
    let (skeleton, structure) = encode_structure_using_group(doc, r, c, b, |c, depths, b| {
        let value = c
            .compact_group_commitments_at_depths(&doc.value.embeds.group, depths, r, b)
            .map_err(|error| match error {
                GroupEncodingError::Structure(e) => PortableError::from(StructureError::from(e)),
                GroupEncodingError::Codec(e) => boundary(e),
            })?;
        b.charge(Resource::Work, 1)?;
        if value.entry_digests.len() != doc.value.embeds.group.entries.len() {
            return Err(PortableError::Shape);
        }
        commitments = Some(value);
        c.encode_foreign_group(
            &ForeignClosureGroup {
                provenances: Vec::new(),
                environments: Vec::new(),
                entries: Vec::new(),
            },
            b,
        )
        .map_err(boundary)
    })?;
    check(r, &skeleton, b)?;
    let commitments = commitments.ok_or(PortableError::Shape)?;
    let mut preimage = Vec::new();
    push(&mut preimage, skeleton, b)?;
    b.charge(Resource::Work, 32)?;
    b.charge(Resource::AllocationUnits, 32)?;
    push(
        &mut preimage,
        NdfValue::Bytes(commitments.group_digest.0.to_vec()),
        b,
    )?;
    b.poll()?;
    Ok(DocumentCommitment {
        value: NdfValue::List(preimage),
        structure,
        components: commitments.entry_digests,
    })
}
/// Construct only native-checked private hash preimages, not a portable PageSet.
pub(crate) fn encode_pages<'a, C: FoundationGroupCommitmentCodec>(
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<super::pages::PageSetEncoding<'a>, PortableError<C::Error>> {
    let s = schema(r)?;
    let mut pages = Vec::new();
    let mut structures = Vec::new();
    let mut components = Vec::new();
    for page in &set.pages {
        b.charge(Resource::Work, 1)?;
        let registration = page.registration.put(s, c, b)?;
        text::check_type(&registration, "PageRegistration", r, b)?;
        let doc = document(&page.document, r, c, b)?;
        push(&mut structures, doc.structure, b)?;
        push(&mut components, doc.components, b)?;
        let mut fields = Vec::new();
        push(&mut fields, registration, b)?;
        push(&mut fields, doc.value, b)?;
        push(&mut pages, NdfValue::List(fields), b)?;
    }
    let files = set.files.put(s, c, b)?;
    let NdfValue::List(values) = &files else {
        return Err(PortableError::Shape);
    };
    for file in values {
        b.charge(Resource::Work, 1)?;
        text::check_type(file, "PageFile", r, b)?;
    }
    let mut fields = Vec::new();
    push(&mut fields, NdfValue::List(pages), b)?;
    push(&mut fields, files, b)?;
    b.poll()?;
    Ok(super::pages::PageSetEncoding {
        value: NdfValue::List(fields),
        structures,
        components,
    })
}
/// Only the private constructor above supplies these positional list preimages.
pub(crate) fn document_value<'a, E>(
    value: &'a NdfValue,
    page: usize,
    b: &mut Budget,
) -> Result<&'a NdfValue, PortableError<E>> {
    b.charge(Resource::Work, 1)?;
    let NdfValue::List(fields) = value else {
        return Err(PortableError::Shape);
    };
    let [NdfValue::List(pages), _] = fields.as_slice() else {
        return Err(PortableError::Shape);
    };
    let Some(NdfValue::List(fields)) = pages.get(page) else {
        return Err(PortableError::Shape);
    };
    let [_, document] = fields.as_slice() else {
        return Err(PortableError::Shape);
    };
    Ok(document)
}

pub fn plan_to_value<'a, C: FoundationGroupCommitmentCodec>(
    plan: &PageLinkPlan,
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, crate::pages::PageError<'a, C::Error>> {
    let checked = crate::pages::compact::resolve(set, r, c, b)?;
    let actual = checked.plan().put(crate::portable::schema(r)?, c, b)?;
    let supplied = plan.put(crate::portable::schema(r)?, c, b)?;
    let left = c
        .canonical_value_digest(b"NEPL3.Doc.Compact.PagePlan.v1\0", &actual, b)
        .map_err(boundary)?;
    let right = c
        .canonical_value_digest(b"NEPL3.Doc.Compact.PagePlan.v1\0", &supplied, b)
        .map_err(boundary)?;
    if left != right {
        return Err(PortableError::Shape.into());
    }
    text::check_type(&supplied, "PageLinkPlan", r, b)?;
    Ok(supplied)
}
pub fn plan_from_value<'a, C: FoundationGroupCommitmentCodec>(
    input: &NdfValue,
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PageLinkPlan, crate::pages::PageError<'a, C::Error>> {
    text::check_type(input, "PageLinkPlan", r, b)?;
    let plan = PageLinkPlan::read(input, crate::portable::schema(r)?, c, b)?;
    plan_to_value(&plan, set, r, c, b)?;
    Ok(plan)
}
