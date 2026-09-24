//! Checked record encoding over immutable existing NDF fields. Repeated callers
//! can borrow shared large fields without constructing another owned record.
//! Source admission and domain semantics remain the typed adapter's obligation.
use crate::{WireError, encode};
use alloc::vec::Vec;
use nepl3_core::{
    budget::Budget,
    schema::SchemaRegistry,
    source::Digest,
    value::{NdfValue, SchemaRef},
};

/// Encode the exact registered record using the ordinary canonical encoder.
/// All fields are revalidated in this call; borrowing conveys no stored proof.
pub fn record(
    schema: &SchemaRef,
    kind: &str,
    fields: &[&NdfValue],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    registry.validate_record_fields(schema, kind, fields, budget)?;
    encode::record_fields(schema, kind, fields, budget)
}

/// Hash `domain || canonical NDF/1(record)` without allocating the record or
/// its CBOR output. Encoding, hashing and every validation remain budgeted.
pub fn record_digest(
    domain: &[u8],
    schema: &SchemaRef,
    kind: &str,
    fields: &[&NdfValue],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Digest, WireError> {
    registry.validate_record_fields(schema, kind, fields, budget)?;
    encode::record_fields_digest(domain, schema, kind, fields, budget)
}
