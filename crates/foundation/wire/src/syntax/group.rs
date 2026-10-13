//! Closed shared owner tables. Portable decoding recomputes environment digests.
use super::*;
pub(crate) mod identity;
use nepl3_core::syntax::group::{
    ForeignClosureGroup, ForeignGroupEntry, ForeignOwnerEnvironment, ForeignProvenance,
    OwnerEnvironmentRef, ProvenanceRef,
};

pub(crate) fn to_value(
    value: &ForeignClosureGroup,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    value.validate(registry, b, admission)?;
    let provenances = sequence(&value.provenances, b, |v, b| {
        record(
            schema,
            "ForeignProvenance",
            [
                sequence(&v.origins, b, |v, b| origin_value(v, schema, b))?,
                sources_value(&v.sources, schema, admission, b)?,
                sequence(&v.source_maps, b, |v, b| mapping_value(v, schema, b))?,
            ],
            b,
        )
    })?;
    let environments = sequence(&value.environments, b, |v, b| {
        record(
            schema,
            "ForeignOwnerEnvironment",
            [
                id_value(v.provenance.0, "ForeignProvenanceRef", schema, b)?,
                entry_value(&v.environment, schema, registry, b)?,
            ],
            b,
        )
    })?;
    let entries = sequence(&value.entries, b, |v, b| {
        record(
            schema,
            "ForeignGroupEntry",
            [
                id_value(v.owner.0, "ForeignOwnerEnvironmentRef", schema, b)?,
                foreign::syntax_value(&v.syntax, schema, registry, admission, b)?,
            ],
            b,
        )
    })?;
    record(
        schema,
        "ForeignClosureGroup",
        [provenances, environments, entries],
        b,
    )
}
pub(crate) fn from_value(
    value: &NdfValue,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ForeignClosureGroup, WireError> {
    b.poll()?;
    let f = fields(value, schema, "ForeignClosureGroup", 3)?;
    let provenances = collect(list(&f[0])?, b, |v, b| {
        let f = fields(v, schema, "ForeignProvenance", 3)?;
        let sources = sources_from(&f[1], schema, admission, b)?;
        let store = store(&sources, b)?;
        Ok(ForeignProvenance {
            origins: collect(list(&f[0])?, b, |v, b| origin_from(v, schema, &store, b))?,
            sources,
            source_maps: collect(list(&f[2])?, b, |v, b| mapping_from(v, schema, &store, b))?,
        })
    })?;
    let environments = collect(list(&f[1])?, b, |v, b| {
        let f = fields(v, schema, "ForeignOwnerEnvironment", 2)?;
        Ok(ForeignOwnerEnvironment {
            provenance: ProvenanceRef(id_from(&f[0], "ForeignProvenanceRef", schema)?),
            environment: entry_from(&f[1], schema, registry, b)?,
        })
    })?;
    let entries = collect(list(&f[2])?, b, |v, b| {
        let f = fields(v, schema, "ForeignGroupEntry", 2)?;
        Ok(ForeignGroupEntry {
            owner: OwnerEnvironmentRef(id_from(&f[0], "ForeignOwnerEnvironmentRef", schema)?),
            syntax: foreign::syntax_from(&f[1], schema, registry, admission, b)?,
        })
    })?;
    let value = ForeignClosureGroup {
        provenances,
        environments,
        entries,
    };
    value.validate(registry, b, admission)?;
    Ok(value)
}
pub fn encode(
    value: &ForeignClosureGroup,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let value = to_value(value, schema, registry, admission, b)?;
    crate::encode_checked(&value, &expected("ForeignClosureGroup"), registry, b)
}
pub fn decode(
    bytes: &[u8],
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<ForeignClosureGroup, WireError> {
    let value = crate::decode_checked(bytes, &expected("ForeignClosureGroup"), registry, b)?;
    from_value(value.value(), schema, registry, admission, b)
}
