//! First reception validates the explicit document closure. A raw printed guest
//! is host data, not a newly minted syntax/meaning correspondence proof.
use super::text::{check_type, sources};
use super::*;
use crate::print::{PrintIdentity, PrintOutcome, PrintReply, PrintRequest};

pub fn request_to_value<C: FoundationValueCodec>(
    request: &PrintRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = schema(r)?;
    let document = to_value(&request.document, r, c, b)?;
    let out = record(
        s,
        "PrintRequest",
        [
            document,
            request.mode.put(s, c, b)?,
            request.bindings.put(s, c, b)?,
            request.guests.put(s, c, b)?,
        ],
        b,
    )?;
    check_type(&out, "PrintRequest", r, b)?;
    Ok(out)
}
pub fn request_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PrintRequest, PortableError<C::Error>> {
    check_type(input, "PrintRequest", r, b)?;
    let s = schema(r)?;
    let f = fields(input, s, "PrintRequest", 4)?;
    Ok(PrintRequest {
        document: from_value(&f[0], r, c, b)?,
        mode: Value::read(&f[1], s, c, b)?,
        bindings: Value::read(&f[2], s, c, b)?,
        guests: Value::read(&f[3], s, c, b)?,
    })
}
pub fn identity_to_value<C: FoundationValueCodec>(
    identity: &PrintIdentity,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let out = identity.put(schema(r)?, c, b)?;
    check_type(&out, "PrintIdentity", r, b)?;
    Ok(out)
}
pub fn identity_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PrintIdentity, PortableError<C::Error>> {
    check_type(input, "PrintIdentity", r, b)?;
    Value::read(input, schema(r)?, c, b)
}
fn tag<E>(reply: &PrintReply) -> Result<(), PortableError<E>> {
    if reply.report.trace_overflow.is_some()
        && !matches!(reply.outcome, PrintOutcome::Stopped { .. })
    {
        return Err(PortableError::Shape);
    }
    Ok(())
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &PrintReply,
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.poll()?;
    tag(reply)?;
    document.validate_structure(r, b, c.source_admission())?;
    let store = sources(document, b)?;
    let mut scoped = c.scoped(&store);
    let out = reply.put(schema(r)?, &mut scoped, b)?;
    check_type(&out, "PrintReply", r, b)?;
    Ok(out)
}
pub fn reply_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PrintReply, PortableError<C::Error>> {
    check_type(input, "PrintReply", r, b)?;
    document.validate_structure(r, b, c.source_admission())?;
    let store = sources(document, b)?;
    let mut scoped = c.scoped(&store);
    let reply = Value::read(input, schema(r)?, &mut scoped, b)?;
    tag(&reply)?;
    Ok(reply)
}
