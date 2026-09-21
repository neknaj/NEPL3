//! Complete package definitions cross NDF; host operation implementations remain external.
use super::{PortableError, boundary, value::*};
use crate::package::{CheckedLanguagePackage, LanguagePackage, PackageError, PackageProvenance};
use alloc::vec::Vec;
use nepl3_core::{
    budget::Budget,
    schema::SchemaRegistry,
    source::{SourceSnapshot, SourceStore},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
use nepl3_reader::portable::plan;

fn package_error<E>(error: PackageError) -> PortableError<E> {
    match error {
        PackageError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Package(other),
    }
}
fn reader_error<E>(error: nepl3_reader::portable::PortableError<E>) -> PortableError<E> {
    match error {
        nepl3_reader::portable::PortableError::Stopped(reason) => PortableError::Stopped(reason),
        other => PortableError::Reader(other),
    }
}
fn source_store<E>(
    sources: &[SourceSnapshot],
    budget: &mut Budget,
) -> Result<SourceStore, PortableError<E>> {
    let mut store = SourceStore::default();
    for source in sources {
        store.insert_with_budget(source.clone_with_budget(budget)?, budget)?;
    }
    Ok(store)
}
/// Serialize all declaration arenas and provenance of a checked package.
pub fn to_value<C: FoundationValueCodec>(
    checked: &CheckedLanguagePackage<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.poll()?;
    let p = checked.package();
    let registry = checked.registry();
    let s = Schemas::new(registry)?;
    let store = source_store(&p.provenance.sources, b)?;
    let mut codec = codec.scoped(&store);
    let mut modes = Vec::new();
    for mode in &p.modes {
        push(
            &mut modes,
            plan::mode_to_value(mode, checked.reader(), &mut codec, b).map_err(reader_error)?,
            b,
        )?;
    }
    let provenance = record(
        s.engine,
        "PackageProvenance",
        [
            codec
                .encode_sources(&p.provenance.sources, b)
                .map_err(boundary)?,
            codec
                .encode_origins(&p.provenance.origins, b)
                .map_err(boundary)?,
            codec
                .encode_mappings(&p.provenance.source_maps, b)
                .map_err(boundary)?,
            p.provenance.declarations.value(&s, &mut codec, b)?,
        ],
        b,
    )?;
    let value = record(
        s.engine,
        "LanguagePackage",
        [
            p.schema.value(&s, &mut codec, b)?,
            p.payload_schemas.value(&s, &mut codec, b)?,
            p.root.value(&s, &mut codec, b)?,
            plan::to_value(checked.reader(), &mut codec, b).map_err(reader_error)?,
            NdfValue::List(modes),
            p.categories.value(&s, &mut codec, b)?,
            p.reads.value(&s, &mut codec, b)?,
            p.forms.value(&s, &mut codec, b)?,
            p.leaves.value(&s, &mut codec, b)?,
            p.namespaces.value(&s, &mut codec, b)?,
            p.bindings.value(&s, &mut codec, b)?,
            p.extensions.value(&s, &mut codec, b)?,
            p.recovery.value(&s, &mut codec, b)?,
            provenance,
        ],
        b,
    )?;
    registry.validate(&expected("LanguagePackage", b)?, &value, b)?;
    Ok(value)
}
/// Receive declarations with their own source closure, then run normal package checks.
/// Schema descriptors and executable providers must be supplied independently by the host.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<LanguagePackage, PortableError<C::Error>> {
    b.poll()?;
    registry.validate(&expected("LanguagePackage", b)?, value, b)?;
    let s = Schemas::new(registry)?;
    let f = fields(value, s.engine, "LanguagePackage", 14)?;
    let provenance = fields(&f[13], s.engine, "PackageProvenance", 4)?;
    let sources = codec.decode_sources(&provenance[0], b).map_err(boundary)?;
    let store = source_store(&sources, b)?;
    let mut codec = codec.scoped(&store);
    let origins = codec.decode_origins(&provenance[1], b).map_err(boundary)?;
    let source_maps = codec.decode_mappings(&provenance[2], b).map_err(boundary)?;
    let declarations = Value::read(&provenance[3], &s, &mut codec, b)?;
    let reader = plan::from_value(&f[3], registry, &mut codec, b).map_err(reader_error)?;
    let checked = reader
        .check(registry, b)
        .map_err(|e| package_error(PackageError::from(e)))?;
    let mut modes = Vec::new();
    for value in list(&f[4])? {
        push(
            &mut modes,
            plan::mode_from_value(value, &checked, &mut codec, b).map_err(reader_error)?,
            b,
        )?;
    }
    let package = LanguagePackage {
        schema: Value::read(&f[0], &s, &mut codec, b)?,
        payload_schemas: Value::read(&f[1], &s, &mut codec, b)?,
        root: Value::read(&f[2], &s, &mut codec, b)?,
        reader,
        modes,
        categories: Value::read(&f[5], &s, &mut codec, b)?,
        reads: Value::read(&f[6], &s, &mut codec, b)?,
        forms: Value::read(&f[7], &s, &mut codec, b)?,
        leaves: Value::read(&f[8], &s, &mut codec, b)?,
        namespaces: Value::read(&f[9], &s, &mut codec, b)?,
        bindings: Value::read(&f[10], &s, &mut codec, b)?,
        extensions: Value::read(&f[11], &s, &mut codec, b)?,
        recovery: Value::read(&f[12], &s, &mut codec, b)?,
        provenance: PackageProvenance {
            sources,
            origins,
            source_maps,
            declarations,
        },
    };
    package
        .check_with_admission(registry, b, codec.source_admission())
        .map_err(package_error)?;
    Ok(package)
}
