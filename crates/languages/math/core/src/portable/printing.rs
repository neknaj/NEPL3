//! Source artifacts are data. Only local regeneration compares them with a
//! requested immutable input and selected host; decoding alone proves neither
//! input identity nor guest semantic equivalence.
use super::*;
use crate::model::{MathPrintIdentity, MathPrintRequest, MathPrintResult};
use crate::{check::ValidatedMathShape, model::MathSourceArtifact, print};

/// Encode an explicit source-closed request. Guest assertions remain untrusted
/// until execution checks their exact identities and selected schema.
pub fn request_to_value<C: FoundationValueCodec>(
    request: &MathPrintRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let syntax = super::to_value(&request.syntax, r, c, b)?;
    let s = schema(r)?;
    let raw = record(
        s,
        "MathPrintRequest",
        [
            syntax,
            request.doc_schema.put(s, c, b)?,
            request.guests.put(s, c, b)?,
        ],
        b,
    )?;
    check_report(r, &raw, "MathPrintRequest", b)?;
    Ok(raw)
}
/// Decode data and revalidate its explicit source closure; no print proof.
pub fn request_from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathPrintRequest, PortableError<C::Error>> {
    check_report(r, raw, "MathPrintRequest", b)?;
    let s = schema(r)?;
    let f = fields(raw, s, "MathPrintRequest", 3)?;
    Ok(MathPrintRequest {
        syntax: super::from_value(&f[0], r, c, b)?,
        doc_schema: Value::read(&f[1], s, c, b)?,
        guests: Value::read(&f[2], s, c, b)?,
    })
}
pub fn identity_to_value<C: FoundationValueCodec>(
    identity: &MathPrintIdentity,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let raw = identity.put(schema(r)?, c, b)?;
    check_report(r, &raw, "MathPrintIdentity", b)?;
    Ok(raw)
}
pub fn identity_from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathPrintIdentity, PortableError<C::Error>> {
    check_report(r, raw, "MathPrintIdentity", b)?;
    MathPrintIdentity::read(raw, schema(r)?, c, b)
}
pub fn result_to_value<C: FoundationValueCodec>(
    result: &MathPrintResult,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let raw = result.put(schema(r)?, c, b)?;
    check_report(r, &raw, "MathPrintResult", b)?;
    Ok(raw)
}
/// Raw result data, not an execution proof. In particular, a remote Stopped
/// record does not authenticate usage or mutate the local Budget's stop state.
pub fn result_from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathPrintResult, PortableError<C::Error>> {
    check_report(r, raw, "MathPrintResult", b)?;
    MathPrintResult::read(raw, schema(r)?, c, b)
}

#[derive(Debug)]
pub enum Error<C, G> {
    Boundary(PortableError<C>),
    Print(print::PrintError<G>),
    Mismatch,
}

pub fn to_value<C: FoundationValueCodec>(
    artifact: &MathSourceArtifact,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let raw = artifact.put(schema(registry)?, codec, budget)?;
    check_report(registry, &raw, "MathSourceArtifact", budget)?;
    Ok(raw)
}

/// Decode schema-valid data, without certifying how its text was generated.
pub fn from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<MathSourceArtifact, PortableError<C::Error>> {
    check_report(registry, raw, "MathSourceArtifact", budget)?;
    MathSourceArtifact::read(raw, schema(registry)?, codec, budget)
}

/// Regenerate canonical prefix source using the caller's selected guest host
/// and current Budget. Return the local artifact only after exact entry/text
/// equality. Guest semantic equivalence and provider request identity remain
/// separate contracts. Comparison is O(text bytes), in addition to generation.
pub fn verify<C: FoundationValueCodec, G: print::GuestPrinter>(
    raw: &NdfValue,
    input: &ValidatedMathShape<'_>,
    guests: &mut G,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<MathSourceArtifact, Error<C::Error, G::Error>> {
    let received = from_value(raw, registry, codec, budget).map_err(Error::Boundary)?;
    let expected = print::prefix(input, guests, budget).map_err(Error::Print)?;
    budget
        .charge(
            Resource::Work,
            (received.text.len() as u64)
                .saturating_add(expected.text.len() as u64)
                .saturating_add(1),
        )
        .map_err(|s| Error::Boundary(PortableError::Stopped(s)))?;
    if received != expected {
        return Err(Error::Mismatch);
    }
    Ok(expected)
}
