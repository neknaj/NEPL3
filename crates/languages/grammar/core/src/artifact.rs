//! Staged exchange of compiled data. This does not prove correspondence to a
//! compilation request, register executable providers, or implement grammar.compile.
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaDescriptor, SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    value::{NdfValue, Record, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use nepl3_engine::{
    package::{CheckedLanguagePackage, LanguagePackage},
    portable::{PortableError, package},
};

const OWNER: &str = "nepl3.grammar";
const KIND: &str = "CompiledArtifact";
fn boundary<E: FoundationCodecError>(error: E) -> PortableError<E> {
    match error.stop_reason() {
        Some(reason) => PortableError::Stopped(reason),
        None => PortableError::Boundary(error),
    }
}
fn selected<'a, E>(
    registry: &'a SchemaRegistry,
    budget: &mut Budget,
) -> Result<&'a SchemaRef, PortableError<E>> {
    budget.poll()?;
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    registry
        .selected_descriptor_with_budget(OWNER, 1, budget)?
        .map(|(reference, _)| reference)
        .ok_or(SchemaError::UnknownSchema.into())
}
fn validate<E>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    budget.charge(Resource::Work, (OWNER.len() + KIND.len()) as u64)?;
    budget.charge(Resource::AllocationUnits, (OWNER.len() + KIND.len()) as u64)?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: OWNER.into(),
            revision: 1,
            name: KIND.into(),
        }),
        value,
        budget,
    )?;
    Ok(())
}
/// Exchange one generated surface descriptor and the complete checked package.
/// Other payload/provider descriptors remain explicit host dependencies.
pub fn to_value<C: FoundationValueCodec>(
    checked: &CheckedLanguagePackage<'_>,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let registry = checked.registry();
    let schema = selected(registry, budget)?;
    let descriptor = registry
        .descriptor_with_budget(&checked.package().schema, budget)?
        .ok_or(SchemaError::UnknownSchema)?;
    let surface = codec
        .encode_schema_descriptor(descriptor, budget)
        .map_err(boundary)?;
    let package = package::to_value(checked, codec, budget)?;
    budget.charge(
        Resource::Work,
        (schema.package.len() + KIND.len() + 2) as u64,
    )?;
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + KIND.len() + 2 * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    let value = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: KIND.into(),
        fields: Vec::from([surface, package]),
    });
    validate(&value, registry, budget)?;
    Ok(value)
}
/// A borrowed decoding stage, not a checked package or an authority proof.
/// Its lifetime deliberately does not borrow a registry or a foundation codec.
pub struct PendingArtifact<'a> {
    raw: &'a NdfValue,
    package: &'a NdfValue,
    expected_surface: &'a SchemaRef,
}
/// Decode only the descriptor against a host-selected exact identity. The host
/// drops this codec, registers the descriptor in its private transaction registry,
/// finalizes dependencies, and recreates its codec before `PendingArtifact::decode`.
/// Both stages MUST reuse the same Budget and SourceAdmission. Discard the whole
/// transaction on failure; no provisional registry may be published as a package.
/// The expected identity is not evidence of compilation from any requested source.
pub fn prepare_received<'a, C: FoundationValueCodec>(
    raw: &'a NdfValue,
    expected_surface: &'a SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<(SchemaDescriptor, PendingArtifact<'a>), PortableError<C::Error>> {
    let schema = selected(registry, budget)?;
    let NdfValue::Record(record) = raw else {
        return Err(PortableError::Shape);
    };
    budget.charge(
        Resource::Work,
        (record.schema.package.len() + schema.package.len() + record.kind.len() + KIND.len() + 41)
            as u64,
    )?;
    if record.schema != *schema || record.kind != KIND || record.fields.len() != 2 {
        return Err(PortableError::Shape);
    }
    let descriptor = codec
        .decode_schema_descriptor(&record.fields[0], expected_surface, budget)
        .map_err(boundary)?;
    Ok((
        descriptor,
        PendingArtifact {
            raw,
            package: &record.fields[1],
            expected_surface,
        },
    ))
}
impl PendingArtifact<'_> {
    /// Complete structural and semantic package checking only after exact
    /// dependency registration. The host retains ownership of publication.
    pub fn decode<C: FoundationValueCodec>(
        self,
        registry: &SchemaRegistry,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<LanguagePackage, PortableError<C::Error>> {
        selected(registry, budget)?;
        registry
            .descriptor_with_budget(self.expected_surface, budget)?
            .ok_or(SchemaError::UnknownSchema)?;
        validate(self.raw, registry, budget)?;
        let package = package::from_value(self.package, registry, codec, budget)?;
        budget.charge(
            Resource::Work,
            (package.schema.package.len() + self.expected_surface.package.len() + 41) as u64,
        )?;
        if package.schema != *self.expected_surface {
            return Err(SchemaError::IdentityMismatch.into());
        }
        Ok(package)
    }
}
