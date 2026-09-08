use crate::WireError;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    source::Digest,
    value::{Integer, NdfValue, SchemaRef},
};

use sha2::{Digest as _, Sha256};

// Both destinations consume the same canonical encoding traversal. Only the
// byte-vector destination materializes the encoded output.
trait Sink {
    fn write(&mut self, value: &[u8], budget: &mut Budget) -> Result<(), WireError>;
}
impl Sink for Vec<u8> {
    fn write(&mut self, value: &[u8], budget: &mut Budget) -> Result<(), WireError> {
        let length = u64::try_from(value.len()).map_err(|_| WireError::InvalidLength)?;
        budget.charge(Resource::OutputBytes, length)?;
        budget.charge(Resource::AllocationUnits, length)?;
        budget.charge(Resource::Work, length)?;
        self.extend_from_slice(value);
        Ok(())
    }
}
impl Sink for Sha256 {
    fn write(&mut self, value: &[u8], budget: &mut Budget) -> Result<(), WireError> {
        let length = u64::try_from(value.len()).map_err(|_| WireError::InvalidLength)?;
        // Retain both encoding and hashing work, without allocating or returning
        // a temporary CBOR byte vector. Each charge precedes processing the bytes.
        budget.charge(Resource::Work, length)?;
        budget.charge(Resource::Work, length)?;
        self.update(value);
        Ok(())
    }
}
fn bytes(out: &mut impl Sink, value: &[u8], budget: &mut Budget) -> Result<(), WireError> {
    out.write(value, budget)
}

fn head(out: &mut impl Sink, major: u8, value: u64, budget: &mut Budget) -> Result<(), WireError> {
    let mut data = [0; 9];
    let length;
    if value < 24 {
        data[0] = (major << 5) | value as u8;
        length = 1;
    } else {
        let (additional, size) = if value <= u8::MAX.into() {
            (24, 1)
        } else if value <= u16::MAX.into() {
            (25, 2)
        } else if value <= u32::MAX.into() {
            (26, 4)
        } else {
            (27, 8)
        };
        data[0] = (major << 5) | additional;
        data[1..=size].copy_from_slice(&value.to_be_bytes()[8 - size..]);
        length = size + 1;
    }
    bytes(out, &data[..length], budget)
}

fn raw(out: &mut impl Sink, major: u8, value: &[u8], budget: &mut Budget) -> Result<(), WireError> {
    head(out, major, value.len() as u64, budget)?;
    bytes(out, value, budget)
}

fn integer(out: &mut impl Sink, value: &Integer, budget: &mut Budget) -> Result<(), WireError> {
    let magnitude_length = value.as_bigint().bits().div_ceil(8);
    budget.charge(Resource::AllocationUnits, magnitude_length)?;
    budget.charge(Resource::Work, magnitude_length)?;
    let (negative, magnitude) = value.canonical_parts();
    head(out, 4, 3, budget)?;
    head(out, 0, 3, budget)?;
    bytes(out, &[if negative { 0xf5 } else { 0xf4 }], budget)?;
    raw(out, 2, &magnitude, budget)
}

fn schema(out: &mut impl Sink, schema: &SchemaRef, budget: &mut Budget) -> Result<(), WireError> {
    head(out, 4, 3, budget)?;
    raw(out, 3, schema.package.as_bytes(), budget)?;
    head(out, 0, schema.revision, budget)?;
    raw(out, 2, &schema.digest.0, budget)
}

fn push<'a>(
    stack: &mut Vec<(&'a NdfValue, u64)>,
    value: &'a NdfValue,
    depth: u64,
    budget: &mut Budget,
) -> Result<(), WireError> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<(&NdfValue, u64)>() as u64,
    )?;
    stack.push((value, depth));
    Ok(())
}

fn children<'a>(
    out: &mut impl Sink,
    stack: &mut Vec<(&'a NdfValue, u64)>,
    values: &'a [NdfValue],
    depth: u64,
    budget: &mut Budget,
) -> Result<(), WireError> {
    head(out, 4, values.len() as u64, budget)?;
    let next = depth
        .checked_add(1)
        .ok_or(nepl3_core::budget::StopReason::DepthLimit)?;
    for value in values.iter().rev() {
        push(stack, value, next, budget)?;
    }
    Ok(())
}

fn emit(item: &NdfValue, out: &mut impl Sink, budget: &mut Budget) -> Result<(), WireError> {
    let mut pending = Vec::new();
    push(&mut pending, item, 1, budget)?;
    while let Some((item, depth)) = pending.pop() {
        budget.observe_depth(depth)?;
        budget.charge(Resource::Nodes, 1)?;
        match item {
            NdfValue::Unit => {
                head(out, 4, 1, budget)?;
                head(out, 0, 0, budget)?;
            }
            NdfValue::Bool(v) => {
                head(out, 4, 2, budget)?;
                head(out, 0, 1, budget)?;
                bytes(out, &[if *v { 0xf5 } else { 0xf4 }], budget)?;
            }
            NdfValue::U64(v) => {
                head(out, 4, 2, budget)?;
                head(out, 0, 2, budget)?;
                head(out, 0, *v, budget)?;
            }
            NdfValue::Integer(v) => integer(out, v, budget)?,
            NdfValue::Rational(v) => {
                head(out, 4, 3, budget)?;
                head(out, 0, 4, budget)?;
                integer(out, v.numerator(), budget)?;
                budget.charge(Resource::Work, v.denominator().bits().div_ceil(8))?;
                budget.charge(
                    Resource::AllocationUnits,
                    v.denominator().bits().div_ceil(8),
                )?;
                raw(out, 2, &v.denominator_bytes(), budget)?;
            }
            NdfValue::Text(v) => {
                head(out, 4, 2, budget)?;
                head(out, 0, 5, budget)?;
                raw(out, 3, v.as_bytes(), budget)?;
            }
            NdfValue::Bytes(v) => {
                head(out, 4, 2, budget)?;
                head(out, 0, 6, budget)?;
                raw(out, 2, v, budget)?;
            }
            NdfValue::List(v) => {
                head(out, 4, 2, budget)?;
                head(out, 0, 7, budget)?;
                children(out, &mut pending, v, depth, budget)?;
            }
            NdfValue::None => {
                head(out, 4, 1, budget)?;
                head(out, 0, 8, budget)?;
            }
            NdfValue::Some(v) => {
                head(out, 4, 2, budget)?;
                head(out, 0, 9, budget)?;
                push(
                    &mut pending,
                    v,
                    depth
                        .checked_add(1)
                        .ok_or(nepl3_core::budget::StopReason::DepthLimit)?,
                    budget,
                )?;
            }
            NdfValue::Record(v) => {
                head(out, 4, 4, budget)?;
                head(out, 0, 10, budget)?;
                schema(out, &v.schema, budget)?;
                raw(out, 3, v.kind.as_bytes(), budget)?;
                children(out, &mut pending, &v.fields, depth, budget)?;
            }
            NdfValue::Variant(v) => {
                head(out, 4, 5, budget)?;
                head(out, 0, 11, budget)?;
                schema(out, &v.schema, budget)?;
                raw(out, 3, v.type_name.as_bytes(), budget)?;
                raw(out, 3, v.variant.as_bytes(), budget)?;
                children(out, &mut pending, &v.fields, depth, budget)?;
            }
        }
    }
    Ok(())
}

pub(super) fn encode(item: &NdfValue, budget: &mut Budget) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    emit(item, &mut out, budget)?;
    Ok(out)
}

pub(super) fn digest(
    domain: &[u8],
    item: &NdfValue,
    budget: &mut Budget,
) -> Result<Digest, WireError> {
    budget.charge(Resource::OutputBytes, 32)?;
    budget.charge(Resource::Work, domain.len() as u64)?;
    let mut hash = Sha256::new();
    hash.update(domain);
    emit(item, &mut hash, budget)?;
    Ok(Digest(hash.finalize().into()))
}
