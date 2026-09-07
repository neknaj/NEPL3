//! Budgeted typed value mechanics shared by foundation adapters.
use crate::{WireError, boundary::*, source::*, view::*};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    origin::{Mapping, Origin},
    source::{SourceStore, Span},
    value::{NdfValue, Record, SchemaRef, TypedValue, Variant},
};
pub(crate) trait Codec: Sized {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError>;
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError>;
}
impl Codec for u64 {
    fn value(&self, _: &SchemaRef, _: &mut Budget) -> Result<NdfValue, WireError> {
        Ok(NdfValue::U64(*self))
    }
    fn from(
        v: &NdfValue,
        _: &SchemaRef,
        _: &SourceStore,
        _: &mut Budget,
    ) -> Result<Self, WireError> {
        as_u64(v)
    }
}
impl Codec for String {
    fn value(&self, _: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        text(self, b)
    }
    fn from(
        v: &NdfValue,
        _: &SchemaRef,
        _: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        copied(v, b)
    }
}
impl<T: Codec> Codec for Vec<T> {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        sequence(self, b, |v, b| v.value(s, b))
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        collect(list(v)?, b, |v, b| T::from(v, s, store, b))
    }
}
impl<T: Codec> Codec for Option<T> {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        option(self.as_ref(), b, |v, b| v.value(s, b))
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        option_from(v, b, |v, b| T::from(v, s, store, b))
    }
}
impl Codec for Span {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        span_value(self, s, b)
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        span_from_value(v, s, store, b)
    }
}
impl Codec for SchemaRef {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        schema_value(self, s, b)
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        _: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        schema_from(v, s, b)
    }
}
impl Codec for Origin {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        crate::origin::origin_value(self, s, b)
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        crate::origin::origin_from(v, s, store, b)
    }
}
impl Codec for Mapping {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        crate::origin::mapping_value(self, s, b)
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        crate::origin::mapping_from(v, s, store, b)
    }
}
impl Codec for TypedValue {
    fn value(&self, _: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        Ok(match self.clone_with_budget(b)? {
            TypedValue::Record(v) => NdfValue::Record(v),
            TypedValue::Variant(v) => NdfValue::Variant(v),
        })
    }
    fn from(
        v: &NdfValue,
        _: &SchemaRef,
        _: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let (schema, name, case, values) = match v {
            NdfValue::Record(v) => (&v.schema, &v.kind, None, &v.fields),
            NdfValue::Variant(v) => (&v.schema, &v.type_name, Some(&v.variant), &v.fields),
            _ => return Err(WireError::InvalidType),
        };
        b.charge(
            Resource::AllocationUnits,
            (schema.package.len() + name.len() + case.map_or(0, |v| v.len())) as u64,
        )?;
        let fields = collect(values, b, |v, b| Ok(v.clone_with_budget(b)?))?;
        Ok(match case {
            None => TypedValue::Record(Record {
                schema: schema.clone(),
                kind: name.clone(),
                fields,
            }),
            Some(case) => TypedValue::Variant(Variant {
                schema: schema.clone(),
                type_name: name.clone(),
                variant: case.clone(),
                fields,
            }),
        })
    }
}
