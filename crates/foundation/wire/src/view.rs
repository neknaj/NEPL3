//! Typed token/view projection. Numeric references remain local to each token.
use crate::{WireError, decode_checked, encode_checked, source::*};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceError, SourceStore},
    value::{KindRef, NdfValue, SchemaRef, Variant},
    view::*,
};

pub(crate) fn list(value: &NdfValue) -> Result<&[NdfValue], WireError> {
    match value {
        NdfValue::List(values) => Ok(values),
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn copied(value: &NdfValue, budget: &mut Budget) -> Result<String, WireError> {
    let value = as_text(value)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(value.into())
}
pub(crate) fn collect<T, U>(
    items: &[T],
    budget: &mut Budget,
    mut convert: impl FnMut(&T, &mut Budget) -> Result<U, WireError>,
) -> Result<Vec<U>, WireError> {
    budget.charge(
        Resource::AllocationUnits,
        (items.len() * core::mem::size_of::<U>()) as u64,
    )?;
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        budget.charge(Resource::Work, 1)?;
        result.push(convert(item, budget)?);
    }
    Ok(result)
}
pub(crate) fn schema_value(
    value: &SchemaRef,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    budget.charge(Resource::AllocationUnits, 32)?;
    record(
        schema,
        "SchemaRef",
        [
            text(&value.package, budget)?,
            NdfValue::U64(value.revision),
            NdfValue::Bytes(value.digest.0.to_vec()),
        ],
        budget,
    )
}
pub(crate) fn schema_from(
    value: &NdfValue,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<SchemaRef, WireError> {
    let f = fields(value, schema, "SchemaRef", 3)?;
    Ok(SchemaRef {
        package: copied(&f[0], budget)?,
        revision: as_u64(&f[1])?,
        digest: as_digest(&f[2])?,
    })
}
fn kind_value(
    value: &KindRef,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    record(
        schema,
        "KindRef",
        [
            schema_value(&value.schema, schema, budget)?,
            NdfValue::U64(value.local_kind),
        ],
        budget,
    )
}
fn kind_from(
    value: &NdfValue,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<KindRef, WireError> {
    let f = fields(value, schema, "KindRef", 2)?;
    Ok(KindRef {
        schema: schema_from(&f[0], schema, budget)?,
        local_kind: as_u64(&f[1])?,
    })
}
fn enumeration(
    schema: &SchemaRef,
    name: &str,
    variant: &str,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + variant.len()) as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: name.into(),
        variant: variant.into(),
        fields: Vec::new(),
    }))
}
fn enum_name<'a>(
    value: &'a NdfValue,
    schema: &SchemaRef,
    name: &str,
) -> Result<&'a str, WireError> {
    match value {
        NdfValue::Variant(value)
            if &value.schema == schema && value.type_name == name && value.fields.is_empty() =>
        {
            Ok(&value.variant)
        }
        _ => Err(WireError::InvalidType),
    }
}
fn role_name(role: FallbackRole) -> &'static str {
    match role {
        FallbackRole::Content => "Content",
        FallbackRole::Marker => "Marker",
        FallbackRole::Delimiter => "Delimiter",
        FallbackRole::Name => "Name",
        FallbackRole::Quantity => "Quantity",
        FallbackRole::Annotation => "Annotation",
    }
}
fn role(name: &str) -> Result<FallbackRole, WireError> {
    Ok(match name {
        "Content" => FallbackRole::Content,
        "Marker" => FallbackRole::Marker,
        "Delimiter" => FallbackRole::Delimiter,
        "Name" => FallbackRole::Name,
        "Quantity" => FallbackRole::Quantity,
        "Annotation" => FallbackRole::Annotation,
        _ => return Err(WireError::InvalidType),
    })
}
fn refs_value(
    values: &[ViewRef],
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    Ok(NdfValue::List(collect(values, budget, |id, budget| {
        record(schema, "ViewRef", [NdfValue::U64(id.0)], budget)
    })?))
}
fn refs_from(
    value: &NdfValue,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<Vec<ViewRef>, WireError> {
    collect(list(value)?, budget, |value, _| {
        Ok(ViewRef(as_u64(&fields(value, schema, "ViewRef", 1)?[0])?))
    })
}

pub(crate) fn views_value(
    views: &ViewBundle,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let elements = collect(&views.elements, budget, |element, budget| {
        let fields_value = collect(&element.fields, budget, |field, budget| {
            record(
                schema,
                "ViewField",
                [
                    text(&field.name, budget)?,
                    refs_value(&field.children, schema, budget)?,
                ],
                budget,
            )
        })?;
        let roles = collect(&element.roles, budget, |role, budget| {
            record(
                schema,
                "PresentationClass",
                [
                    schema_value(&role.schema, schema, budget)?,
                    text(&role.name, budget)?,
                    enumeration(schema, "FallbackRole", role_name(role.fallback), budget)?,
                ],
                budget,
            )
        })?;
        let relations = collect(&element.relations, budget, |relation, budget| {
            record(
                schema,
                "ViewRelation",
                [
                    schema_value(&relation.schema, schema, budget)?,
                    text(&relation.kind, budget)?,
                    record(
                        schema,
                        "ViewRef",
                        [NdfValue::U64(relation.target.0)],
                        budget,
                    )?,
                ],
                budget,
            )
        })?;
        record(
            schema,
            "ViewElement",
            [
                kind_value(&element.kind, schema, budget)?,
                span_value(&element.span, schema, budget)?,
                NdfValue::List(fields_value),
                NdfValue::List(roles),
                NdfValue::List(relations),
            ],
            budget,
        )
    })?;
    record(
        schema,
        "ViewBundle",
        [
            NdfValue::List(elements),
            refs_value(&views.roots, schema, budget)?,
        ],
        budget,
    )
}
pub(crate) fn views_from(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<ViewBundle, WireError> {
    let root = fields(value, schema, "ViewBundle", 2)?;
    let elements = collect(list(&root[0])?, budget, |value, budget| {
        let f = fields(value, schema, "ViewElement", 5)?;
        let fields_value = collect(list(&f[2])?, budget, |value, budget| {
            let f = fields(value, schema, "ViewField", 2)?;
            Ok(ViewField {
                name: copied(&f[0], budget)?,
                children: refs_from(&f[1], schema, budget)?,
            })
        })?;
        let roles = collect(list(&f[3])?, budget, |value, budget| {
            let f = fields(value, schema, "PresentationClass", 3)?;
            Ok(PresentationClass {
                schema: schema_from(&f[0], schema, budget)?,
                name: copied(&f[1], budget)?,
                fallback: role(enum_name(&f[2], schema, "FallbackRole")?)?,
            })
        })?;
        let relations = collect(list(&f[4])?, budget, |value, budget| {
            let f = fields(value, schema, "ViewRelation", 3)?;
            Ok(ViewRelation {
                schema: schema_from(&f[0], schema, budget)?,
                kind: copied(&f[1], budget)?,
                target: ViewRef(as_u64(&fields(&f[2], schema, "ViewRef", 1)?[0])?),
            })
        })?;
        Ok(ViewElement {
            kind: kind_from(&f[0], schema, budget)?,
            span: span_from_value(&f[1], schema, sources, budget)?,
            fields: fields_value,
            roles,
            relations,
        })
    })?;
    Ok(ViewBundle {
        elements,
        roots: refs_from(&root[1], schema, budget)?,
    })
}

pub(crate) fn token_value(
    token: &Token,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    let trivia = collect(&token.leading_trivia, budget, |trivia, budget| {
        let name = match trivia.kind {
            TriviaKind::Whitespace => "Whitespace",
            TriviaKind::Comment => "Comment",
            TriviaKind::Bom => "Bom",
        };
        record(
            schema,
            "Trivia",
            [
                span_value(&trivia.span, schema, budget)?,
                enumeration(schema, "TriviaKind", name, budget)?,
            ],
            budget,
        )
    })?;
    record(
        schema,
        "Token",
        [
            kind_value(&token.kind, schema, budget)?,
            span_value(&token.head, schema, budget)?,
            token.payload.clone_with_budget(budget)?,
            views_value(&token.views, schema, budget)?,
            NdfValue::List(trivia),
        ],
        budget,
    )
}
pub(crate) fn token_from(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<Token, WireError> {
    let f = fields(value, schema, "Token", 5)?;
    let trivia = collect(list(&f[4])?, budget, |value, budget| {
        let f = fields(value, schema, "Trivia", 2)?;
        let kind = match enum_name(&f[1], schema, "TriviaKind")? {
            "Whitespace" => TriviaKind::Whitespace,
            "Comment" => TriviaKind::Comment,
            "Bom" => TriviaKind::Bom,
            _ => return Err(WireError::InvalidType),
        };
        Ok(Trivia {
            span: span_from_value(&f[0], schema, sources, budget)?,
            kind,
        })
    })?;
    Ok(Token {
        kind: kind_from(&f[0], schema, budget)?,
        head: span_from_value(&f[1], schema, sources, budget)?,
        payload: f[2].clone_with_budget(budget)?,
        views: views_from(&f[3], schema, sources, budget)?,
        leading_trivia: trivia,
    })
}

/// Checks view geometry and the typed payload's structural schema before encoding.
pub fn encode_token(
    token: &Token,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    admit(token, sources, admission, budget)?;
    token.validate(sources, registry, budget)?;
    encode_checked(
        &token_value(token, schema, budget)?,
        &expected("Token"),
        registry,
        budget,
    )
}
/// Reconstructs local references and validates them against the supplied snapshots.
pub fn decode_token(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<Token, WireError> {
    let value = decode_checked(bytes, &expected("Token"), registry, budget)?;
    let token = token_from(value.value(), schema, sources, budget)?;
    admit(&token, sources, admission, budget)?;
    token.validate(sources, registry, budget)?;
    Ok(token)
}

fn admit(
    token: &Token,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    budget: &mut Budget,
) -> Result<(), WireError> {
    for span in core::iter::once(&token.head)
        .chain(token.views.elements.iter().map(|v| &v.span))
        .chain(token.leading_trivia.iter().map(|v| &v.span))
    {
        budget.charge(
            Resource::AllocationUnits,
            span.snapshot_ref().source.0.len() as u64,
        )?;
        let snapshot = sources
            .get(span.snapshot())
            .ok_or(SourceError::MissingSnapshot)?;
        admission.admit_existing(snapshot, budget)?;
    }
    Ok(())
}
