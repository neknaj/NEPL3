//! Origin tables retain bundle-local IDs and typed operation identities.
use crate::{WireError, boundary::*, source::*, view::*};
use alloc::vec::Vec;
use nepl3_core::{
    budget::Budget,
    origin::*,
    source::{SourceAdmission, SourceSnapshot, SourceStore},
    value::{NdfValue, OperationRef, SchemaRef},
};

pub(crate) fn mapping_value(
    mapping: &Mapping,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    record(
        schema,
        "SourceMapping",
        [
            span_value(&mapping.source, schema, budget)?,
            span_value(&mapping.target, schema, budget)?,
            variant(
                schema,
                "MappingKind",
                match mapping.kind {
                    MappingKind::Exact => "Exact",
                    MappingKind::Transformed => "Transformed",
                },
                [],
                budget,
            )?,
        ],
        budget,
    )
}
pub(crate) fn mapping_from(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<Mapping, WireError> {
    let f = fields(value, schema, "SourceMapping", 3)?;
    let (case, payload) = variant_parts(&f[2], schema, "MappingKind")?;
    if !payload.is_empty() {
        return Err(WireError::InvalidType);
    }
    Ok(Mapping {
        source: span_from_value(&f[0], schema, sources, budget)?,
        target: span_from_value(&f[1], schema, sources, budget)?,
        kind: match case {
            "Exact" => MappingKind::Exact,
            "Transformed" => MappingKind::Transformed,
            _ => return Err(WireError::InvalidType),
        },
    })
}

pub(crate) fn origin_value(
    origin: &Origin,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let refs = |ids: &[OriginId], budget: &mut Budget| {
        sequence(ids, budget, |id, b| id_value(id.0, "OriginRef", schema, b))
    };
    match origin {
        Origin::Direct(span) => variant(
            schema,
            "Origin",
            "Direct",
            [span_value(span, schema, budget)?],
            budget,
        ),
        Origin::Composite(ids) => {
            variant(schema, "Origin", "Composite", [refs(ids, budget)?], budget)
        }
        Origin::Generated {
            operation,
            callsite,
            inputs,
        } => variant(
            schema,
            "Origin",
            "Generated",
            [
                record(
                    schema,
                    "OperationRef",
                    [
                        schema_value(&operation.schema, schema, budget)?,
                        text(&operation.name, budget)?,
                    ],
                    budget,
                )?,
                option(callsite.as_ref(), budget, |span, b| {
                    span_value(span, schema, b)
                })?,
                refs(inputs, budget)?,
            ],
            budget,
        ),
        Origin::Synthetic { reason, anchor } => variant(
            schema,
            "Origin",
            "Synthetic",
            [
                text(reason, budget)?,
                option(anchor.as_ref(), budget, |span, b| {
                    span_value(span, schema, b)
                })?,
            ],
            budget,
        ),
    }
}
pub(crate) fn origin_from(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<Origin, WireError> {
    let (case, f) = variant_parts(value, schema, "Origin")?;
    let refs = |value: &NdfValue, budget: &mut Budget| {
        collect(list(value)?, budget, |value, _| {
            Ok(OriginId(id_from(value, "OriginRef", schema)?))
        })
    };
    Ok(match (case, f) {
        ("Direct", [span]) => Origin::Direct(span_from_value(span, schema, sources, budget)?),
        ("Composite", [ids]) => Origin::Composite(refs(ids, budget)?),
        ("Generated", [operation, callsite, inputs]) => {
            let f = fields(operation, schema, "OperationRef", 2)?;
            Origin::Generated {
                operation: OperationRef {
                    schema: schema_from(&f[0], schema, budget)?,
                    name: copied(&f[1], budget)?,
                },
                callsite: option_from(callsite, budget, |value, b| {
                    span_from_value(value, schema, sources, b)
                })?,
                inputs: refs(inputs, budget)?,
            }
        }
        ("Synthetic", [reason, anchor]) => Origin::Synthetic {
            reason: copied(reason, budget)?,
            anchor: option_from(anchor, budget, |value, b| {
                span_from_value(value, schema, sources, b)
            })?,
        },
        _ => return Err(WireError::InvalidType),
    })
}
pub fn encode_origins(
    origins: &[Origin],
    snapshots: &[SourceSnapshot],
    schema: &SchemaRef,
    registry: &nepl3_core::schema::SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let sources = store(snapshots, budget)?;
    OriginGraph::validate_origins(origins, &sources, budget)?;
    let value = record(
        schema,
        "OriginBundle",
        [
            record(
                schema,
                "SourceBundle",
                [sources_value(snapshots, schema, admission, budget)?],
                budget,
            )?,
            sequence(origins, budget, |origin, b| origin_value(origin, schema, b))?,
        ],
        budget,
    )?;
    crate::encode_checked(&value, &expected("OriginBundle"), registry, budget)
}
pub fn decode_origins(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &nepl3_core::schema::SchemaRegistry,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<(Vec<SourceSnapshot>, Vec<Origin>), WireError> {
    let value = crate::decode_checked(bytes, &expected("OriginBundle"), registry, budget)?;
    let f = fields(value.value(), schema, "OriginBundle", 2)?;
    let snapshots = sources_from(
        &fields(&f[0], schema, "SourceBundle", 1)?[0],
        schema,
        admission,
        budget,
    )?;
    let sources = store(&snapshots, budget)?;
    let origins = collect(list(&f[1])?, budget, |value, b| {
        origin_from(value, schema, &sources, b)
    })?;
    OriginGraph::validate_origins(&origins, &sources, budget)?;
    Ok((snapshots, origins))
}
