//! Portable output data including each guest's exact display occurrence.
//! A receiver first selects and validates its guests, then explicitly prepares
//! and renders its expected output. Decoding invokes no guest callbacks and
//! grants no namespace, asset, semantic-validation or HTML-serialization proof.
use super::*;

/// Encode the schema-checked data. This operation grants no rendering authority.
pub fn to_value<C: FoundationValueCodec>(
    output: &RenderedWithForeign,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = output.put(schema(registry)?, registry, codec, budget)?;
    check(&value, "RenderedWithForeign", registry, budget)?;
    Ok(value)
}

/// Compare all fields with the receiver's independently reconstructed output,
/// including options, origins and the ordered placements of shared guests.
/// `expected` is ordinary data: the caller owns its derivation and the validation
/// of guest meaning (including hidden variants). Equality grants no extra proof.
pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    expected: &RenderedWithForeign,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<RenderedWithForeign, PortableError<C::Error>> {
    check(input, "RenderedWithForeign", registry, budget)?;
    let expected = to_value(expected, registry, codec, budget)?;
    if !input.equal_with_budget(&expected, budget)? {
        return Err(PortableError::Mismatch);
    }
    RenderedWithForeign::read(input, schema(registry)?, registry, codec, budget)
}
