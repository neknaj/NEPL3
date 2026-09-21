//! Schema-checked Profile exchange against independently supplied host registrations.
//! This adapter serializes requirements. Executable providers and resource bytes
//! remain owned by the RuntimeCatalog and are never manufactured from the wire.
use super::{PortableError, value::*};
use crate::profile::{ParseProfile, ResolvedParseProfile, RuntimeCatalog};
use nepl3_core::{
    budget::Budget, schema::SchemaRegistry, value::NdfValue, value_codec::FoundationValueCodec,
};

/// Encode every field of an already resolved parsing profile.
pub fn to_value<C: FoundationValueCodec>(
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    budget.poll()?;
    let registry = profile.registry();
    let value = profile
        .profile()
        .value(&Schemas::new(registry)?, codec, budget)?;
    registry.validate(&expected("ParseProfile", budget)?, &value, budget)?;
    Ok(value)
}

/// Decode a complete profile and resolve all its requirements against the host.
/// The returned owned definition retains its declared limits. Decoding uses the
/// caller's budget; limits supplied by the document never enlarge that budget.
/// Call `resolve` to obtain a proof borrowing the definition before execution.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    catalog: &RuntimeCatalog<'_>,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<ParseProfile, PortableError<C::Error>> {
    budget.poll()?;
    registry.validate(&expected("ParseProfile", budget)?, value, budget)?;
    let profile = ParseProfile::read(value, &Schemas::new(registry)?, codec, budget)?;
    profile.resolve(catalog, registry, budget)?;
    Ok(profile)
}
