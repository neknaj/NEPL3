//! Projected Head protocol. Initial decoding requires no sender object or full
//! snapshot. Authentication and execution authorization remain host duties.
use super::{PortableError, value::*};
use crate::{
    head::{HeadCall, HeadReply},
    profile::ResolvedParseProfile,
};
use nepl3_core::{budget::Budget, value::NdfValue, value_codec::FoundationValueCodec};
mod delegation;
mod windows;
pub use delegation::*;
pub use windows::WindowAdmission;

/// Encodes checked projected data. This does not admit full source snapshots.
pub fn call_to_value<C: FoundationValueCodec>(
    call: &HeadCall,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    call.validate_projection(profile, budget)?;
    let value = call.value(&Schemas::new(profile.registry())?, codec, budget)?;
    profile
        .registry()
        .validate(&expected("HeadCall", budget)?, &value, budget)?;
    Ok(value)
}
/// Restores an untrusted projection and charges only explicitly supplied byte
/// windows into this receiving operation's separate interval ledger. Payloads
/// never cause implicit source lookup or admission.
pub fn call_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    windows: &mut WindowAdmission,
    budget: &mut Budget,
) -> Result<HeadCall, PortableError<C::Error>> {
    profile
        .registry()
        .validate(&expected("HeadCall", budget)?, value, budget)?;
    let call = HeadCall::read(value, &Schemas::new(profile.registry())?, codec, budget)?;
    call.validate_projection(profile, budget)?;
    windows.admit(&call, budget)?;
    Ok(call)
}
/// Reply positions and operation identity are checked against the exact issued
/// call. Usage is still the provider's claim; no parent counters are changed.
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &HeadReply,
    call: &HeadCall,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    call.validate_reply(reply, profile, budget)?;
    let value = reply.value(&Schemas::new(profile.registry())?, codec, budget)?;
    profile
        .registry()
        .validate(&expected("HeadReply", budget)?, &value, budget)?;
    Ok(value)
}
pub fn reply_decode<C: FoundationValueCodec>(
    value: &NdfValue,
    issued: &HeadCall,
    profile: &ResolvedParseProfile<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<HeadReply, PortableError<C::Error>> {
    profile
        .registry()
        .validate(&expected("HeadReply", budget)?, value, budget)?;
    let reply = HeadReply::read(value, &Schemas::new(profile.registry())?, codec, budget)?;
    issued.validate_reply(&reply, profile, budget)?;
    Ok(reply)
}
