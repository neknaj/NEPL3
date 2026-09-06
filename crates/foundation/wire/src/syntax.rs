//! Iterative bundle conversion preserves foreign syntax and token sidecars.
mod order;
use crate::{WireError, boundary::*, environment::*, origin::*, source::*, view::*};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::OriginId,
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
    syntax::*,
    value::{NdfScalar, NdfValue, SchemaRef},
};
use order::{mapped, order};

fn push<T>(values: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), WireError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    values.push(value);
    Ok(())
}
fn scalar_value(value: &NdfScalar, budget: &mut Budget) -> Result<NdfValue, WireError> {
    let size = match value {
        NdfScalar::Text(v) => v.len() as u64,
        NdfScalar::Bytes(v) => v.len() as u64,
        NdfScalar::Integer(v) => v.as_bigint().bits().div_ceil(8),
        NdfScalar::Rational(v) => {
            v.numerator().as_bigint().bits().div_ceil(8) + v.denominator().bits().div_ceil(8)
        }
        _ => 0,
    };
    budget.charge(Resource::AllocationUnits, size)?;
    Ok(match value {
        NdfScalar::Unit => NdfValue::Unit,
        NdfScalar::Bool(v) => NdfValue::Bool(*v),
        NdfScalar::U64(v) => NdfValue::U64(*v),
        NdfScalar::Integer(v) => NdfValue::Integer(v.clone()),
        NdfScalar::Rational(v) => NdfValue::Rational(v.clone()),
        NdfScalar::Text(v) => NdfValue::Text(v.clone()),
        NdfScalar::Bytes(v) => NdfValue::Bytes(v.clone()),
    })
}
fn scalar_from(value: &NdfValue, budget: &mut Budget) -> Result<NdfScalar, WireError> {
    let mut value = value.clone_with_budget(budget)?;
    Ok(match &mut value {
        NdfValue::Unit => NdfScalar::Unit,
        NdfValue::Bool(v) => NdfScalar::Bool(*v),
        NdfValue::U64(v) => NdfScalar::U64(*v),
        NdfValue::Integer(v) => NdfScalar::Integer(core::mem::replace(
            v,
            nepl3_core::value::Integer::from(0_i64),
        )),
        NdfValue::Rational(v) => {
            budget.charge(
                Resource::AllocationUnits,
                v.numerator().as_bigint().bits().div_ceil(8) + v.denominator().bits().div_ceil(8),
            )?;
            NdfScalar::Rational(v.clone())
        }
        NdfValue::Text(v) => NdfScalar::Text(core::mem::take(v)),
        NdfValue::Bytes(v) => NdfScalar::Bytes(core::mem::take(v)),
        _ => return Err(WireError::InvalidType),
    })
}
fn environment_ref_value(
    value: &EnvironmentRef,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    record(
        schema,
        "EnvironmentRef",
        [NdfValue::U64(value.id), bytes(&value.digest.0, budget)?],
        budget,
    )
}
fn environment_ref_from(value: &NdfValue, schema: &SchemaRef) -> Result<EnvironmentRef, WireError> {
    let f = fields(value, schema, "EnvironmentRef", 2)?;
    Ok(EnvironmentRef {
        id: as_u64(&f[0])?,
        digest: as_digest(&f[1])?,
    })
}

enum Encode<'a> {
    Enter(&'a SyntaxBundle, u64),
    Finish(&'a SyntaxBundle, usize),
}
fn bundle_value(
    bundle: &SyntaxBundle,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let mut pending = Vec::new();
    let mut results = Vec::new();
    push(&mut pending, Encode::Enter(bundle, 1), budget)?;
    while let Some(step) = pending.pop() {
        budget.charge(Resource::Work, 1)?;
        match step {
            Encode::Enter(bundle, depth) => {
                budget.observe_depth(depth)?;
                let mut guests = Vec::new();
                for index in order(bundle, budget)?.0 {
                    let node = &bundle.nodes[index];
                    for field in &node.fields {
                        if let FieldValue::Foreign(guest) = field {
                            push(&mut guests, &guest.bundle, budget)?;
                        }
                    }
                }
                push(&mut pending, Encode::Finish(bundle, guests.len()), budget)?;
                for guest in guests.into_iter().rev() {
                    push(
                        &mut pending,
                        Encode::Enter(guest, depth.checked_add(1).ok_or(StopReason::DepthLimit)?),
                        budget,
                    )?;
                }
            }
            Encode::Finish(bundle, count) => {
                let (indices, mapping) = order(bundle, budget)?;
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or(WireError::InvalidType)?;
                let mut guests = results.drain(start..);
                let nodes = sequence(&indices, budget, |index, budget| {
                    let node = &bundle.nodes[*index];
                    let values = sequence(&node.fields, budget, |field, budget| match field {
                        FieldValue::Atom(value) => variant(
                            schema,
                            "FieldValue",
                            "Atom",
                            [scalar_value(value, budget)?],
                            budget,
                        ),
                        FieldValue::Child(id) => variant(
                            schema,
                            "FieldValue",
                            "Child",
                            [id_value(mapped(*id, &mapping)?, "NodeRef", schema, budget)?],
                            budget,
                        ),
                        FieldValue::Children(ids) => variant(
                            schema,
                            "FieldValue",
                            "Children",
                            [sequence(ids, budget, |id, b| {
                                id_value(mapped(*id, &mapping)?, "NodeRef", schema, b)
                            })?],
                            budget,
                        ),
                        FieldValue::Foreign(guest) => variant(
                            schema,
                            "FieldValue",
                            "Foreign",
                            [record(
                                schema,
                                "ForeignSyntax",
                                [
                                    schema_value(&guest.schema, schema, budget)?,
                                    text(&guest.category, budget)?,
                                    id_value(0, "NodeRef", schema, budget)?,
                                    guests.next().ok_or(WireError::InvalidType)?,
                                    environment_ref_value(&guest.environment, schema, budget)?,
                                ],
                                budget,
                            )?],
                            budget,
                        ),
                    })?;
                    record(
                        schema,
                        "SyntaxNode",
                        [
                            schema_value(&node.schema, schema, budget)?,
                            text(&node.kind, budget)?,
                            values,
                            option(node.head.as_ref(), budget, |span, b| {
                                span_value(span, schema, b)
                            })?,
                            option(node.cover.as_ref(), budget, |span, b| {
                                span_value(span, schema, b)
                            })?,
                            id_value(node.origin.0, "OriginRef", schema, budget)?,
                            option(node.token.as_ref(), budget, |id, b| {
                                id_value(id.0, "TokenRef", schema, b)
                            })?,
                        ],
                        budget,
                    )
                })?;
                if guests.next().is_some() {
                    return Err(WireError::InvalidType);
                }
                drop(guests);
                let value = record(
                    schema,
                    "SyntaxBundle",
                    [
                        sources_value(&bundle.sources, schema, admission, budget)?,
                        nodes,
                        sequence(&bundle.origins, budget, |origin, b| {
                            origin_value(origin, schema, b)
                        })?,
                        id_value(0, "NodeRef", schema, budget)?,
                        sequence(&bundle.environments, budget, |entry, b| {
                            entry_value(entry, schema, registry, b)
                        })?,
                        sequence(&bundle.tokens, budget, |token, b| {
                            token_value(token, schema, b)
                        })?,
                    ],
                    budget,
                )?;
                push(&mut results, value, budget)?;
            }
        }
    }
    if results.len() != 1 {
        return Err(WireError::InvalidType);
    }
    results.pop().ok_or(WireError::InvalidType)
}

enum Decode<'a> {
    Enter(&'a NdfValue, u64),
    Finish(&'a NdfValue, usize),
}
fn bundle_from(
    value: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<SyntaxBundle, WireError> {
    let mut pending = Vec::new();
    let mut results = Vec::new();
    push(&mut pending, Decode::Enter(value, 1), budget)?;
    while let Some(step) = pending.pop() {
        budget.charge(Resource::Work, 1)?;
        match step {
            Decode::Enter(value, depth) => {
                budget.observe_depth(depth)?;
                let root = fields(value, schema, "SyntaxBundle", 6)?;
                let mut guests = Vec::new();
                for node in list(&root[1])? {
                    let f = fields(node, schema, "SyntaxNode", 7)?;
                    for field in list(&f[2])? {
                        let (case, f) = variant_parts(field, schema, "FieldValue")?;
                        if case == "Foreign" {
                            let [guest] = f else {
                                return Err(WireError::InvalidType);
                            };
                            let f = fields(guest, schema, "ForeignSyntax", 5)?;
                            push(&mut guests, &f[3], budget)?;
                        }
                    }
                }
                push(&mut pending, Decode::Finish(value, guests.len()), budget)?;
                for guest in guests.into_iter().rev() {
                    push(
                        &mut pending,
                        Decode::Enter(guest, depth.checked_add(1).ok_or(StopReason::DepthLimit)?),
                        budget,
                    )?;
                }
            }
            Decode::Finish(value, count) => {
                let f = fields(value, schema, "SyntaxBundle", 6)?;
                let snapshots = sources_from(&f[0], schema, admission, budget)?;
                let sources = store(&snapshots, budget)?;
                let start = results
                    .len()
                    .checked_sub(count)
                    .ok_or(WireError::InvalidType)?;
                let mut guests = results.drain(start..);
                let nodes = collect(list(&f[1])?, budget, |value, budget| {
                    node_from(value, schema, &sources, &mut guests, budget)
                })?;
                if guests.next().is_some() {
                    return Err(WireError::InvalidType);
                }
                drop(guests);
                let bundle = SyntaxBundle {
                    sources: snapshots,
                    nodes,
                    origins: collect(list(&f[2])?, budget, |value, b| {
                        origin_from(value, schema, &sources, b)
                    })?,
                    root: NodeRef(id_from(&f[3], "NodeRef", schema)?),
                    environments: collect(list(&f[4])?, budget, |value, b| {
                        entry_from(value, schema, registry, b)
                    })?,
                    tokens: collect(list(&f[5])?, budget, |value, b| {
                        token_from(value, schema, &sources, b)
                    })?,
                };
                let (indices, _) = order(&bundle, budget)?;
                if bundle.root.0 != 0
                    || indices
                        .iter()
                        .enumerate()
                        .any(|(expected, actual)| expected != *actual)
                {
                    return Err(WireError::NonCanonical);
                }
                push(&mut results, bundle, budget)?;
            }
        }
    }
    if results.len() != 1 {
        return Err(WireError::InvalidType);
    }
    results.pop().ok_or(WireError::InvalidType)
}
fn node_from(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    guests: &mut impl Iterator<Item = SyntaxBundle>,
    budget: &mut Budget,
) -> Result<SyntaxNode, WireError> {
    let f = fields(value, schema, "SyntaxNode", 7)?;
    let values = collect(list(&f[2])?, budget, |value, budget| {
        let (case, f) = variant_parts(value, schema, "FieldValue")?;
        Ok(match (case, f) {
            ("Atom", [value]) => FieldValue::Atom(scalar_from(value, budget)?),
            ("Child", [id]) => FieldValue::Child(NodeRef(id_from(id, "NodeRef", schema)?)),
            ("Children", [ids]) => {
                FieldValue::Children(collect(list(ids)?, budget, |value, _| {
                    Ok(NodeRef(id_from(value, "NodeRef", schema)?))
                })?)
            }
            ("Foreign", [value]) => {
                let f = fields(value, schema, "ForeignSyntax", 5)?;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<ForeignSyntax>() as u64,
                )?;
                FieldValue::Foreign(Box::new(ForeignSyntax {
                    schema: schema_from(&f[0], schema, budget)?,
                    category: copied(&f[1], budget)?,
                    root: NodeRef(id_from(&f[2], "NodeRef", schema)?),
                    bundle: guests.next().ok_or(WireError::InvalidType)?,
                    environment: environment_ref_from(&f[4], schema)?,
                }))
            }
            _ => return Err(WireError::InvalidType),
        })
    })?;
    Ok(SyntaxNode {
        schema: schema_from(&f[0], schema, budget)?,
        kind: copied(&f[1], budget)?,
        fields: values,
        head: option_from(&f[3], budget, |value, b| {
            span_from_value(value, schema, sources, b)
        })?,
        cover: option_from(&f[4], budget, |value, b| {
            span_from_value(value, schema, sources, b)
        })?,
        origin: OriginId(id_from(&f[5], "OriginRef", schema)?),
        token: option_from(&f[6], budget, |value, _| {
            Ok(TokenRef(id_from(value, "TokenRef", schema)?))
        })?,
    })
}

/// Validates graph/source geometry and environment hashes, preserving token sidecars.
/// Form/category invariants and domain meaning remain the engine/language obligations.
pub fn encode_syntax(
    bundle: &SyntaxBundle,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    bundle.validate_with_sources(registry, budget, admission)?;
    crate::encode_checked(
        &bundle_value(bundle, schema, registry, admission, budget)?,
        &expected("SyntaxBundle"),
        registry,
        budget,
    )
}
pub fn decode_syntax(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<SyntaxBundle, WireError> {
    let value = crate::decode_checked(bytes, &expected("SyntaxBundle"), registry, budget)?;
    let bundle = bundle_from(value.value(), schema, registry, admission, budget)?;
    bundle.validate_with_sources(registry, budget, admission)?;
    Ok(bundle)
}
