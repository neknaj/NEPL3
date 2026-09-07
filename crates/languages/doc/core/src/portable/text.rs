//! Request-owned source closure for plain-text data. Receiving data never creates
//! a PreparedText proof; execution prepares the received document independently.
use super::*;
use crate::text::{PlainTextOutcome, PlainTextReply, PlainTextRequest, TextIdentity};

pub(super) fn check_type<E>(
    v: &NdfValue,
    name: &str,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(Resource::AllocationUnits, (name.len() + 10) as u64)?;
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.doc".into(),
            revision: 1,
            name: name.into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
pub fn request_to_value<C: FoundationValueCodec>(
    request: &PlainTextRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = schema(r)?;
    let document = to_value(&request.document, r, c, b)?;
    let out = record(
        s,
        "PlainTextRequest",
        [
            document,
            request.sentence.put(s, c, b)?,
            request.policy.put(s, c, b)?,
            request.resolved.put(s, c, b)?,
        ],
        b,
    )?;
    check_type(&out, "PlainTextRequest", r, b)?;
    Ok(out)
}
pub fn request_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PlainTextRequest, PortableError<C::Error>> {
    check_type(input, "PlainTextRequest", r, b)?;
    let s = schema(r)?;
    let f = fields(input, s, "PlainTextRequest", 4)?;
    Ok(PlainTextRequest {
        document: from_value(&f[0], r, c, b)?,
        sentence: Value::read(&f[1], s, c, b)?,
        policy: Value::read(&f[2], s, c, b)?,
        resolved: Value::read(&f[3], s, c, b)?,
    })
}
pub fn identity_to_value<C: FoundationValueCodec>(
    input: &TextIdentity,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let out = input.put(schema(r)?, c, b)?;
    check_type(&out, "TextIdentity", r, b)?;
    Ok(out)
}
pub fn identity_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<TextIdentity, PortableError<C::Error>> {
    check_type(input, "TextIdentity", r, b)?;
    Value::read(input, schema(r)?, c, b)
}
fn report_tag<E>(reply: &PlainTextReply) -> Result<(), PortableError<E>> {
    if reply.report.trace_overflow.is_some()
        && !matches!(reply.outcome, PlainTextOutcome::Stopped { .. })
    {
        return Err(PortableError::Shape);
    }
    Ok(())
}
pub(super) fn sources<E>(
    document: &DocumentSyntax,
    b: &mut Budget,
) -> Result<SourceStore, PortableError<E>> {
    let mut store = SourceStore::default();
    for source in &document.sources {
        b.charge(Resource::Work, 1)?;
        store
            .insert(source.clone_with_budget(b)?)
            .map_err(StructureError::from)?;
    }
    Ok(store)
}
/// Report positions resolve only within the original request's document.
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &PlainTextReply,
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.poll()?;
    report_tag(reply)?;
    document.validate_structure(r, b, c.source_admission())?;
    let store = sources(document, b)?;
    let mut scoped = c.scoped(&store);
    let out = reply.put(schema(r)?, &mut scoped, b)?;
    check_type(&out, "PlainTextReply", r, b)?;
    Ok(out)
}
pub fn reply_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PlainTextReply, PortableError<C::Error>> {
    check_type(input, "PlainTextReply", r, b)?;
    document.validate_structure(r, b, c.source_admission())?;
    let store = sources(document, b)?;
    let mut scoped = c.scoped(&store);
    let reply = Value::read(input, schema(r)?, &mut scoped, b)?;
    report_tag(&reply)?;
    Ok(reply)
}
