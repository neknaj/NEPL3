//! Source artifacts are data. Only local regeneration compares them with a
//! requested immutable input and selected host; decoding alone proves neither
//! input identity nor guest semantic equivalence.
use super::*;
use crate::{check::ValidatedMathShape, model::MathSourceArtifact, print};

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
