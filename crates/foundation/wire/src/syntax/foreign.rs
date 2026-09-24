use super::*;

/// One immutable owner's portable tables, reused while encoding its closures.
/// This stores data, not a validation proof. Every operation revalidates the
/// closure, source admission and complete registered wire record.
pub struct ForeignClosureEncoder<'a> {
    owner: &'a OwnerProvenance,
    schema: &'a SchemaRef,
    registry: &'a SchemaRegistry,
    owner_fields: [NdfValue; 3],
}
impl<'a> ForeignClosureEncoder<'a> {
    pub fn new(
        owner: &'a OwnerProvenance,
        schema: &'a SchemaRef,
        registry: &'a SchemaRegistry,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        b.charge(Resource::Work, 1)?;
        if !registry.is_finalized() {
            return Err(nepl3_core::schema::SchemaError::Unfinalized.into());
        }
        if registry.selected("nepl3.foundation", 1) != Some(schema) {
            return Err(nepl3_core::schema::SchemaError::WrongType.into());
        }
        Ok(Self {
            owner,
            schema,
            registry,
            owner_fields: [
                sequence(owner.origins(), b, |v, b| origin_value(v, schema, b))?,
                sources_value(owner.sources(), schema, admission, b)?,
                sequence(owner.source_maps(), b, |v, b| mapping_value(v, schema, b))?,
            ],
        })
    }
    fn parts(
        &self,
        value: &ForeignClosure,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<[NdfValue; 2], WireError> {
        b.charge(Resource::Work, 1)?;
        // OwnerProvenance exposes immutable slices. Shared storage has exactly
        // the same three slices; addresses are never serialized or hashed.
        // Independent empty tables can have equal pointers and are equivalent
        // here. This is data reuse, not owner authority or allocation identity.
        if !core::ptr::eq(self.owner.origins(), value.provenance.origins())
            || !core::ptr::eq(self.owner.sources(), value.provenance.sources())
            || !core::ptr::eq(self.owner.source_maps(), value.provenance.source_maps())
        {
            return Err(WireError::InvalidType);
        }
        value.validate(self.registry, b, admission)?;
        closure_parts(value, self.schema, self.registry, admission, b)
    }
    pub fn encode(
        &self,
        value: &ForeignClosure,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<Vec<u8>, WireError> {
        let parts = self.parts(value, admission, b)?;
        crate::borrowed::record(
            self.schema,
            "ForeignClosure",
            &[
                &parts[0],
                &parts[1],
                &self.owner_fields[0],
                &self.owner_fields[1],
                &self.owner_fields[2],
            ],
            self.registry,
            b,
        )
    }
    /// Same domain-separated digest as the ordinary checked closure NDF value.
    pub fn digest(
        &self,
        domain: &[u8],
        value: &ForeignClosure,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<nepl3_core::source::Digest, WireError> {
        let parts = self.parts(value, admission, b)?;
        crate::borrowed::record_digest(
            domain,
            self.schema,
            "ForeignClosure",
            &[
                &parts[0],
                &parts[1],
                &self.owner_fields[0],
                &self.owner_fields[1],
                &self.owner_fields[2],
            ],
            self.registry,
            b,
        )
    }
}

fn closure_parts(
    value: &ForeignClosure,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<[NdfValue; 2], WireError> {
    let syntax = &value.syntax;
    Ok([
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
    ])
}

pub(crate) fn foreign_value(
    value: &ForeignClosure,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    value.validate(registry, b, admission)?;
    let [syntax, environment] = closure_parts(value, schema, registry, admission, b)?;
    record(
        schema,
        "ForeignClosure",
        [
            syntax,
            environment,
            sequence(value.provenance.origins(), b, |v, b| {
                origin_value(v, schema, b)
            })?,
            sources_value(value.provenance.sources(), schema, admission, b)?,
            sequence(value.provenance.source_maps(), b, |v, b| {
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
        provenance: nepl3_core::syntax::OwnerProvenance::new(
            collect(list(&f[2])?, b, |v, b| origin_from(v, schema, &store, b))?,
            owner_sources,
            collect(list(&f[4])?, b, |v, b| mapping_from(v, schema, &store, b))?,
            b,
        )?,
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
