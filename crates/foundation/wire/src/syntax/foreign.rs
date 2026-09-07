use super::*;

pub(crate) fn foreign_value(
    value: &ForeignClosure,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    value.validate(registry, b, admission)?;
    let syntax = &value.syntax;
    record(
        schema,
        "ForeignClosure",
        [
            record(
                schema,
                "ForeignSyntax",
                [
                    schema_value(&syntax.schema, schema, b)?,
                    text(&syntax.category, b)?,
                    id_value(0, "NodeRef", schema, b)?,
                    bundle_value(&syntax.bundle, schema, registry, admission, b)?,
                    environment_ref_value(&syntax.environment, schema, b)?,
                ],
                b,
            )?,
            entry_value(&value.owner_environment, schema, registry, b)?,
            sequence(&value.owner_origins, b, |v, b| origin_value(v, schema, b))?,
            sources_value(&value.owner_sources, schema, admission, b)?,
            sequence(&value.owner_source_maps, b, |v, b| {
                mapping_value(v, schema, b)
            })?,
        ],
        b,
    )
}
pub(crate) fn foreign_from(
    value: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ForeignClosure, WireError> {
    let f = fields(value, schema, "ForeignClosure", 5)?;
    let sf = fields(&f[0], schema, "ForeignSyntax", 5)?;
    let owner_sources = sources_from(&f[3], schema, admission, b)?;
    let store = store(&owner_sources, b)?;
    let result = ForeignClosure {
        syntax: ForeignSyntax {
            schema: schema_from(&sf[0], schema, b)?,
            category: copied(&sf[1], b)?,
            root: NodeRef(id_from(&sf[2], "NodeRef", schema)?),
            bundle: bundle_from(&sf[3], schema, registry, admission, b)?,
            environment: environment_ref_from(&sf[4], schema)?,
        },
        owner_environment: entry_from(&f[1], schema, registry, b)?,
        owner_origins: collect(list(&f[2])?, b, |v, b| origin_from(v, schema, &store, b))?,
        owner_sources,
        owner_source_maps: collect(list(&f[4])?, b, |v, b| mapping_from(v, schema, &store, b))?,
    };
    result.validate(registry, b, admission)?;
    Ok(result)
}
pub fn encode_foreign_closure(
    value: &ForeignClosure,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let value = foreign_value(value, schema, registry, admission, b)?;
    crate::encode_checked(&value, &expected("ForeignClosure"), registry, b)
}
pub fn decode_foreign_closure(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ForeignClosure, WireError> {
    let value = crate::decode_checked(bytes, &expected("ForeignClosure"), registry, b)?;
    foreign_from(value.value(), schema, registry, admission, b)
}
