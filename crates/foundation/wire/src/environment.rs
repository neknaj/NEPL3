//! Environment content is hashed independently of its local table ID.
use crate::{WireError, boundary::*, source::*, view::*};
use nepl3_core::{
    budget::{Budget, Resource},
    origin::OriginId,
    schema::SchemaRegistry,
    source::Digest,
    syntax::{
        Environment, EnvironmentBinding, EnvironmentEntry, NamespaceRef, ResourceContent,
        SyntaxError,
    },
    value::{NdfValue, SchemaRef, TypedValue},
};

pub(crate) fn environment_value(
    environment: &Environment,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let bindings = sequence(&environment.bindings, budget, |binding, budget| {
        let typed = match binding.value.clone_with_budget(budget)? {
            TypedValue::Record(v) => NdfValue::Record(v),
            TypedValue::Variant(v) => NdfValue::Variant(v),
        };
        record(
            schema,
            "EnvironmentBinding",
            [
                record(
                    schema,
                    "EnvironmentNamespaceRef",
                    [
                        schema_value(&binding.namespace.schema, schema, budget)?,
                        text(&binding.namespace.name, budget)?,
                    ],
                    budget,
                )?,
                text(&binding.name, budget)?,
                typed,
                option(binding.origin.as_ref(), budget, |id, b| {
                    id_value(id.0, "OriginRef", schema, b)
                })?,
            ],
            budget,
        )
    })?;
    let resources = sequence(&environment.resources, budget, |resource, budget| {
        record(
            schema,
            "ResourceContent",
            [
                text(&resource.id, budget)?,
                bytes(&resource.digest.0, budget)?,
                bytes(&resource.bytes, budget)?,
            ],
            budget,
        )
    })?;
    record(schema, "Environment", [bindings, resources], budget)
}
pub(crate) fn environment_from(
    value: &NdfValue,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<Environment, WireError> {
    let f = fields(value, schema, "Environment", 2)?;
    let bindings = collect(list(&f[0])?, budget, |value, budget| {
        let f = fields(value, schema, "EnvironmentBinding", 4)?;
        let namespace = fields(&f[0], schema, "EnvironmentNamespaceRef", 2)?;
        let mut payload = f[2].clone_with_budget(budget)?;
        let value = match &mut payload {
            NdfValue::Record(v) => TypedValue::Record(nepl3_core::value::Record {
                schema: core::mem::replace(
                    &mut v.schema,
                    SchemaRef {
                        package: alloc::string::String::new(),
                        revision: 0,
                        digest: Digest([0; 32]),
                    },
                ),
                kind: core::mem::take(&mut v.kind),
                fields: core::mem::take(&mut v.fields),
            }),
            NdfValue::Variant(v) => TypedValue::Variant(nepl3_core::value::Variant {
                schema: core::mem::replace(
                    &mut v.schema,
                    SchemaRef {
                        package: alloc::string::String::new(),
                        revision: 0,
                        digest: Digest([0; 32]),
                    },
                ),
                type_name: core::mem::take(&mut v.type_name),
                variant: core::mem::take(&mut v.variant),
                fields: core::mem::take(&mut v.fields),
            }),
            _ => return Err(WireError::InvalidType),
        };
        Ok(EnvironmentBinding {
            namespace: NamespaceRef {
                schema: schema_from(&namespace[0], schema, budget)?,
                name: copied(&namespace[1], budget)?,
            },
            name: copied(&f[1], budget)?,
            value,
            origin: option_from(&f[3], budget, |value, _| {
                Ok(OriginId(id_from(value, "OriginRef", schema)?))
            })?,
        })
    })?;
    let resources = collect(list(&f[1])?, budget, |value, budget| {
        let f = fields(value, schema, "ResourceContent", 3)?;
        Ok(ResourceContent {
            id: copied(&f[0], budget)?,
            digest: as_digest(&f[1])?,
            bytes: bytes_from(&f[2], budget)?,
        })
    })?;
    Ok(Environment {
        bindings,
        resources,
    })
}
/// Computes the identity of a structurally valid Environment, without claiming
/// validation of bundle-local origin references or domain binding semantics.
pub fn environment_digest(
    environment: &Environment,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Digest, WireError> {
    let value = environment_value(environment, schema, budget)?;
    let bytes = crate::encode_checked(&value, &expected("Environment"), registry, budget)?;
    budget.charge(Resource::Work, bytes.len() as u64)?;
    Ok(Digest::domain(b"NEPL3-ENVIRONMENT-1\0", &bytes))
}
pub(crate) fn entry_value(
    entry: &EnvironmentEntry,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    if environment_digest(&entry.value, schema, registry, budget)? != entry.digest {
        return Err(SyntaxError::Environment.into());
    }
    record(
        schema,
        "EnvironmentEntry",
        [
            NdfValue::U64(entry.id),
            bytes(&entry.digest.0, budget)?,
            environment_value(&entry.value, schema, budget)?,
        ],
        budget,
    )
}
pub(crate) fn entry_from(
    value: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<EnvironmentEntry, WireError> {
    let f = fields(value, schema, "EnvironmentEntry", 3)?;
    let entry = EnvironmentEntry {
        id: as_u64(&f[0])?,
        digest: as_digest(&f[1])?,
        value: environment_from(&f[2], schema, budget)?,
    };
    if environment_digest(&entry.value, schema, registry, budget)? != entry.digest {
        return Err(SyntaxError::Environment.into());
    }
    Ok(entry)
}
