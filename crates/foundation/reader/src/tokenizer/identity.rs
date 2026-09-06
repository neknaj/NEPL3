//! Concrete tokenizer configuration identity; mode names are maps, rule order is semantic.
use super::{ReaderMode, TokenReader};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{CanonicalWriter, SchemaError},
    source::Digest,
    value::SchemaRef,
};

pub(super) fn digest(
    modes: &[ReaderMode],
    plan: Digest,
    budget: &mut Budget,
) -> Result<Digest, SchemaError> {
    budget.charge(
        Resource::AllocationUnits,
        modes.len() as u64 * core::mem::size_of::<&ReaderMode>() as u64,
    )?;
    let mut sorted: Vec<_> = modes.iter().collect();
    let cost = modes.iter().map(|m| m.name.len() as u64 + 1).sum::<u64>();
    budget.charge(Resource::Work, cost.saturating_mul(modes.len() as u64))?;
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    if sorted.windows(2).any(|m| m[0].name == m[1].name) {
        return Err(SchemaError::DuplicateName);
    }
    let mut out = CanonicalWriter::new(budget);
    out.push("{\"modes\":{")?;
    for (i, mode) in sorted.iter().enumerate() {
        if i != 0 {
            out.push(",")?;
        }
        out.quoted(&mode.name)?;
        out.push(":{\"skip\":[")?;
        for (i, skip) in mode.skip.iter().enumerate() {
            if i != 0 {
                out.push(",")?;
            }
            reader(&skip.reader, &mut out)?;
        }
        out.push("],\"take\":[")?;
        for (i, take) in mode.take.iter().enumerate() {
            if i != 0 {
                out.push(",")?;
            }
            out.push("{\"kind\":{\"localKind\":")?;
            out.number(take.kind.local_kind)?;
            out.push(",\"schema\":")?;
            schema(&take.kind.schema, &mut out)?;
            out.push("},\"reader\":")?;
            reader(&take.reader, &mut out)?;
            out.push("}")?;
        }
        out.push("]}")?;
    }
    out.push("},\"readerPlan\":[")?;
    for (i, b) in plan.0.iter().enumerate() {
        if i != 0 {
            out.push(",")?;
        }
        out.number(u64::from(*b))?;
    }
    out.push("]}")?;
    Ok(Digest::domain(b"NEPL3-TOKENIZER-1\0", &out.finish()))
}
fn schema(schema: &SchemaRef, out: &mut CanonicalWriter<'_>) -> Result<(), SchemaError> {
    out.push("{\"digest\":[")?;
    for (i, b) in schema.digest.0.iter().enumerate() {
        if i != 0 {
            out.push(",")?;
        }
        out.number(u64::from(*b))?;
    }
    out.push("],\"package\":")?;
    out.quoted(&schema.package)?;
    out.push(",\"revision\":")?;
    out.number(schema.revision)?;
    out.push("}")
}
fn reader(reader: &TokenReader, out: &mut CanonicalWriter<'_>) -> Result<(), SchemaError> {
    match reader {
        TokenReader::Rule(name) => {
            out.push("{\"rule\":")?;
            out.quoted(name)?;
            out.push("}")
        }
        TokenReader::Builtin(kind) => {
            out.push("{\"builtin\":")?;
            out.quoted(match kind {
                crate::builtin::BuiltinReader::Name => "Name",
                crate::builtin::BuiltinReader::Nat => "Nat",
                crate::builtin::BuiltinReader::Number => "Number",
                crate::builtin::BuiltinReader::Text => "Text",
                crate::builtin::BuiltinReader::Lang => "Lang",
                crate::builtin::BuiltinReader::Trivia => "Trivia",
            })?;
            out.push("}")
        }
    }
}
