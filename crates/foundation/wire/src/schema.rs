//! Schema descriptors exchanged against a known foundation schema. Hosts choose
//! trusted identities, register dependencies and finalize before using a schema.
use crate::{WireError, boundary::typed::Codec, boundary::*, source::*, view::*};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{
        FieldDescriptor, NamedType, OperationDescriptor, SchemaDescriptor, SchemaError,
        SchemaRegistry, TypeDescriptor, TypeShape, VariantDescriptor,
    },
    source::SourceStore,
    value::{NdfValue, SchemaRef},
};

impl Codec for TypeDescriptor {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        crate::foundation::types::encode(self, s, b)
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        _: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        crate::foundation::types::decode(v, s, b)
    }
}
impl Codec for FieldDescriptor {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "FieldDescriptor",
            [text(&self.name, b)?, self.ty.value(s, b)?],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "FieldDescriptor", 2)?;
        Ok(Self {
            name: copied(&f[0], b)?,
            ty: Codec::from(&f[1], s, store, b)?,
        })
    }
}
impl Codec for VariantDescriptor {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "VariantDescriptor",
            [text(&self.name, b)?, self.fields.value(s, b)?],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "VariantDescriptor", 2)?;
        Ok(Self {
            name: copied(&f[0], b)?,
            fields: Codec::from(&f[1], s, store, b)?,
        })
    }
}
impl Codec for TypeShape {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        match self {
            Self::Record { fields } => variant(s, "TypeShape", "Record", [fields.value(s, b)?], b),
            Self::Variant { variants } => {
                variant(s, "TypeShape", "Variant", [variants.value(s, b)?], b)
            }
        }
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        match variant_parts(v, s, "TypeShape")? {
            ("Record", [v]) => Ok(Self::Record {
                fields: Codec::from(v, s, store, b)?,
            }),
            ("Variant", [v]) => Ok(Self::Variant {
                variants: Codec::from(v, s, store, b)?,
            }),
            _ => Err(WireError::InvalidType),
        }
    }
}
impl Codec for NamedType {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "NamedTypeDescriptor",
            [
                text(&self.name, b)?,
                self.shape.value(s, b)?,
                self.constraints.value(s, b)?,
            ],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "NamedTypeDescriptor", 3)?;
        Ok(Self {
            name: copied(&f[0], b)?,
            shape: Codec::from(&f[1], s, store, b)?,
            constraints: Codec::from(&f[2], s, store, b)?,
        })
    }
}
impl Codec for OperationDescriptor {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "OperationDescriptor",
            [
                text(&self.name, b)?,
                self.input.value(s, b)?,
                self.output.value(s, b)?,
                NdfValue::Bool(self.pure),
            ],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "OperationDescriptor", 4)?;
        let NdfValue::Bool(pure) = f[3] else {
            return Err(WireError::InvalidType);
        };
        Ok(Self {
            name: copied(&f[0], b)?,
            input: Codec::from(&f[1], s, store, b)?,
            output: Codec::from(&f[2], s, store, b)?,
            pure,
        })
    }
}
impl Codec for SchemaDescriptor {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "SchemaDescriptor",
            [
                text(&self.package, b)?,
                NdfValue::U64(self.revision),
                self.types.value(s, b)?,
                self.operations.value(s, b)?,
            ],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "SchemaDescriptor", 4)?;
        Ok(Self {
            package: copied(&f[0], b)?,
            revision: as_u64(&f[1])?,
            types: Codec::from(&f[2], s, store, b)?,
            operations: Codec::from(&f[3], s, store, b)?,
        })
    }
}
fn foundation<'a>(
    registry: &'a SchemaRegistry,
    b: &mut Budget,
) -> Result<&'a SchemaRef, WireError> {
    b.poll()?;
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    registry
        .selected("nepl3.foundation", 1)
        .ok_or(SchemaError::UnknownSchema.into())
}

/// Validate names and field shapes, then encode using the selected foundation.
/// Symbolic dependencies can be supplied later, before registry finalization.
pub fn encode(
    value: &SchemaDescriptor,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let s = foundation(registry, b)?;
    value.reference(b)?;
    let value = value.value(s, b)?;
    crate::encode_checked(&value, &expected("SchemaDescriptor"), registry, b)
}

/// Decode and compare the full identity to the host-selected expected schema.
/// Returning a descriptor grants no execution or resource permissions.
pub fn decode(
    input: &[u8],
    identity: &SchemaRef,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<SchemaDescriptor, WireError> {
    let s = foundation(registry, b)?;
    let value = crate::decode_checked(input, &expected("SchemaDescriptor"), registry, b)?;
    let descriptor: SchemaDescriptor = Codec::from(value.value(), s, &SourceStore::default(), b)?;
    let actual = descriptor.reference(b)?;
    let comparison_work = (actual.package.len() as u64)
        .checked_add(identity.package.len() as u64)
        .and_then(|n| n.checked_add(40))
        .ok_or_else(|| b.stop(StopReason::WorkLimit))?;
    b.charge(Resource::Work, comparison_work)?;
    if &actual != identity {
        return Err(SchemaError::IdentityMismatch.into());
    }
    Ok(descriptor)
}
