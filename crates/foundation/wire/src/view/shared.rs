//! Compact local view tables. Table membership never grants source authority.
use super::*;
mod index;
use index::{Index, schema_order, source_order};

fn push<T>(values: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), WireError> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<T>() as u64 * 2,
    )?;
    values
        .try_reserve(1)
        .map_err(|_| b.stop(nepl3_core::budget::StopReason::AllocationLimit))?;
    values.push(value);
    Ok(())
}
fn refs(values: &[ViewRef], b: &mut Budget) -> Result<NdfValue, WireError> {
    Ok(NdfValue::List(collect(values, b, |v, _| {
        Ok(NdfValue::U64(v.0))
    })?))
}

/// Encode local schema/source tables and preserve every ordered view field.
/// The supplied store remains the sole authority for source resolution.
pub fn encode(
    views: &ViewBundle,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    admit_views(views, sources, admission, b)?;
    views.validate(sources, registry, b)?;
    encode_checked(
        &value(views, schema, b)?,
        &expected("SharedViewBundle"),
        registry,
        b,
    )
}

/// Decode against the supplied declaration scope, then validate the complete
/// native graph. Local tables cannot introduce snapshots into that scope.
pub fn decode(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ViewBundle, WireError> {
    let checked = decode_checked(bytes, &expected("SharedViewBundle"), registry, b)?;
    let views = from_value(checked.value(), schema, sources, b)?;
    admit_views(&views, sources, admission, b)?;
    views.validate(sources, registry, b)?;
    Ok(views)
}

fn admit_views(
    views: &ViewBundle,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<(), WireError> {
    for element in &views.elements {
        let id = element.span.snapshot_ref();
        let source = sources
            .get_revision_with_budget(&id.source, id.revision, b)?
            .ok_or(SourceError::MissingSnapshot)?;
        b.charge(Resource::Work, 32)?;
        if source.identity().digest != id.digest {
            return Err(SourceError::MissingSnapshot.into());
        }
        admission.admit_existing(source, b)?;
    }
    Ok(())
}

fn selected<'a, T>(
    values: &'a [T],
    used: &mut [bool],
    index: &NdfValue,
    b: &mut Budget,
) -> Result<&'a T, WireError> {
    b.charge(Resource::Work, 1)?;
    let at = usize::try_from(as_u64(index)?).map_err(|_| WireError::InvalidType)?;
    let value = values.get(at).ok_or(WireError::InvalidType)?;
    *used.get_mut(at).ok_or(WireError::InvalidType)? = true;
    Ok(value)
}
fn selected_schema(
    values: &[SchemaRef],
    used: &mut [bool],
    index: &NdfValue,
    b: &mut Budget,
) -> Result<SchemaRef, WireError> {
    let value = selected(values, used, index, b)?;
    b.charge(Resource::Work, value.package.len() as u64 + 33)?;
    b.charge(Resource::AllocationUnits, value.package.len() as u64)?;
    Ok(value.clone())
}
fn read_refs(value: &NdfValue, b: &mut Budget) -> Result<Vec<ViewRef>, WireError> {
    collect(list(value)?, b, |value, _| Ok(ViewRef(as_u64(value)?)))
}

pub(crate) fn from_value(
    value: &NdfValue,
    schema: &SchemaRef,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<ViewBundle, WireError> {
    let root = fields(value, schema, "SharedViewBundle", 4)?;
    let schemas = collect(list(&root[0])?, b, |value, b| schema_from(value, schema, b))?;
    for pair in schemas.windows(2) {
        if schema_order(&pair[0], &pair[1], b)? != core::cmp::Ordering::Less {
            return Err(WireError::NonCanonical);
        }
    }
    let snapshots = collect(list(&root[1])?, b, |value, b| {
        let f = fields(value, schema, "SourceRef", 3)?;
        let reference = nepl3_core::source::SourceRef {
            source_id: nepl3_core::source::SourceId(copied(&f[0], b)?),
            revision: as_u64(&f[1])?,
            digest: as_digest(&f[2])?,
        };
        sources
            .resolve_with_budget(&reference, b)?
            .ok_or(SourceError::MissingSnapshot.into())
    })?;
    for pair in snapshots.windows(2) {
        if source_order(pair[0].identity(), pair[1].identity(), b)? != core::cmp::Ordering::Less {
            return Err(WireError::NonCanonical);
        }
    }
    let mut schema_used = collect(&schemas, b, |_, _| Ok(false))?;
    let mut source_used = collect(&snapshots, b, |_, _| Ok(false))?;
    let elements = collect(list(&root[2])?, b, |value, b| {
        let f = fields(value, schema, "SharedViewElement", 8)?;
        let kind = KindRef {
            schema: selected_schema(&schemas, &mut schema_used, &f[0], b)?,
            local_kind: as_u64(&f[1])?,
        };
        let snapshot = selected(&snapshots, &mut source_used, &f[2], b)?;
        b.charge(
            Resource::AllocationUnits,
            snapshot.identity().source.0.len() as u64,
        )?;
        let span = snapshot.span(as_u64(&f[3])?, as_u64(&f[4])?)?;
        let fields_value = collect(list(&f[5])?, b, |value, b| {
            let f = fields(value, schema, "SharedViewField", 2)?;
            Ok(ViewField {
                name: copied(&f[0], b)?,
                children: read_refs(&f[1], b)?,
            })
        })?;
        let roles = collect(list(&f[6])?, b, |value, b| {
            let f = fields(value, schema, "SharedPresentationClass", 3)?;
            Ok(PresentationClass {
                schema: selected_schema(&schemas, &mut schema_used, &f[0], b)?,
                name: copied(&f[1], b)?,
                fallback: role(enum_name(&f[2], schema, "FallbackRole")?)?,
            })
        })?;
        let relations = collect(list(&f[7])?, b, |value, b| {
            let f = fields(value, schema, "SharedViewRelation", 3)?;
            Ok(ViewRelation {
                schema: selected_schema(&schemas, &mut schema_used, &f[0], b)?,
                kind: copied(&f[1], b)?,
                target: ViewRef(as_u64(&f[2])?),
            })
        })?;
        Ok(ViewElement {
            kind,
            span,
            fields: fields_value,
            roles,
            relations,
        })
    })?;
    let roots = read_refs(&root[3], b)?;
    for used in schema_used.into_iter().chain(source_used) {
        b.charge(Resource::Work, 1)?;
        if !used {
            return Err(WireError::NonCanonical);
        }
    }
    Ok(ViewBundle { elements, roots })
}

pub(crate) fn value(
    views: &ViewBundle,
    schema: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    let (mut schemas, mut sources) = (Vec::new(), Vec::new());
    for element in &views.elements {
        push(&mut schemas, &element.kind.schema, b)?;
        push(&mut sources, element.span.snapshot_ref(), b)?;
        for role in &element.roles {
            push(&mut schemas, &role.schema, b)?;
        }
        for relation in &element.relations {
            push(&mut schemas, &relation.schema, b)?;
        }
    }
    let schemas = Index::new(schemas, schema_order, b)?;
    let sources = Index::new(sources, source_order, b)?;
    let schema_values = collect(&schemas.values, b, |v, b| schema_value(v, schema, b))?;
    let source_values = collect(&sources.values, b, |v, b| {
        b.charge(Resource::AllocationUnits, 32)?;
        record(
            schema,
            "SourceRef",
            [
                text(&v.source.0, b)?,
                NdfValue::U64(v.revision),
                NdfValue::Bytes(v.digest.0.to_vec()),
            ],
            b,
        )
    })?;
    let elements = collect(&views.elements, b, |element, b| {
        let field_values = collect(&element.fields, b, |field, b| {
            record(
                schema,
                "SharedViewField",
                [text(&field.name, b)?, refs(&field.children, b)?],
                b,
            )
        })?;
        let roles = collect(&element.roles, b, |role, b| {
            record(
                schema,
                "SharedPresentationClass",
                [
                    NdfValue::U64(schemas.find(&role.schema, b)?),
                    text(&role.name, b)?,
                    enumeration(schema, "FallbackRole", role_name(role.fallback), b)?,
                ],
                b,
            )
        })?;
        let relations = collect(&element.relations, b, |relation, b| {
            record(
                schema,
                "SharedViewRelation",
                [
                    NdfValue::U64(schemas.find(&relation.schema, b)?),
                    text(&relation.kind, b)?,
                    NdfValue::U64(relation.target.0),
                ],
                b,
            )
        })?;
        record(
            schema,
            "SharedViewElement",
            [
                NdfValue::U64(schemas.find(&element.kind.schema, b)?),
                NdfValue::U64(element.kind.local_kind),
                NdfValue::U64(sources.find(element.span.snapshot_ref(), b)?),
                NdfValue::U64(element.span.start()),
                NdfValue::U64(element.span.end()),
                NdfValue::List(field_values),
                NdfValue::List(roles),
                NdfValue::List(relations),
            ],
            b,
        )
    })?;
    record(
        schema,
        "SharedViewBundle",
        [
            NdfValue::List(schema_values),
            NdfValue::List(source_values),
            NdfValue::List(elements),
            refs(&views.roots, b)?,
        ],
        b,
    )
}
