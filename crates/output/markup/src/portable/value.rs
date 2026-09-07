use super::PortableError;
use crate::html::*;
use crate::text::TextContext;
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value::{NdfValue, Record, SchemaRef, Variant},
    value_codec::FoundationValueCodec,
};
mod generated;
pub(super) trait Value: Sized {
    fn put<C: FoundationValueCodec>(
        &self,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>>;
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>>;
}
pub(super) fn record<E, const N: usize>(
    s: &SchemaRef,
    name: &str,
    values: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(Resource::Work, (s.package.len() + name.len()) as u64 + 1)?;
    b.charge(
        Resource::AllocationUnits,
        (s.package.len() + name.len() + N * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Record(Record {
        schema: s.clone(),
        kind: name.into(),
        fields: Vec::from(values),
    }))
}
fn variant<E, const N: usize>(
    s: &SchemaRef,
    name: &str,
    case: &str,
    values: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(
        Resource::Work,
        (s.package.len() + name.len() + case.len()) as u64 + 1,
    )?;
    b.charge(
        Resource::AllocationUnits,
        (s.package.len() + name.len() + case.len() + N * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: s.clone(),
        type_name: name.into(),
        variant: case.into(),
        fields: Vec::from(values),
    }))
}
pub(super) fn fields<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    name: &str,
    n: usize,
) -> Result<&'a [NdfValue], PortableError<E>> {
    match v {
        NdfValue::Record(r) if &r.schema == s && r.kind == name && r.fields.len() == n => {
            Ok(&r.fields)
        }
        _ => Err(PortableError::Shape),
    }
}
fn case<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    name: &str,
) -> Result<(&'a str, &'a [NdfValue]), PortableError<E>> {
    match v {
        NdfValue::Variant(r) if &r.schema == s && r.type_name == name => {
            Ok((&r.variant, &r.fields))
        }
        _ => Err(PortableError::Shape),
    }
}
macro_rules! scalar {
    ($ty:ty,$case:ident) => {
        impl Value for $ty {
            fn put<C: FoundationValueCodec>(
                &self,
                _: &SchemaRef,
                _: &mut C,
                b: &mut Budget,
            ) -> Result<NdfValue, PortableError<C::Error>> {
                b.charge(Resource::Work, 1)?;
                Ok(NdfValue::$case(*self))
            }
            fn read<C: FoundationValueCodec>(
                v: &NdfValue,
                _: &SchemaRef,
                _: &mut C,
                b: &mut Budget,
            ) -> Result<Self, PortableError<C::Error>> {
                b.charge(Resource::Work, 1)?;
                match v {
                    NdfValue::$case(v) => Ok(*v),
                    _ => Err(PortableError::Shape),
                }
            }
        }
    };
}
scalar!(u64, U64);
scalar!(bool, Bool);
impl Value for String {
    fn put<C: FoundationValueCodec>(
        &self,
        _: &SchemaRef,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::Work, self.len() as u64)?;
        b.charge(Resource::AllocationUnits, self.len() as u64)?;
        Ok(NdfValue::Text(self.clone()))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &SchemaRef,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let NdfValue::Text(v) = v else {
            return Err(PortableError::Shape);
        };
        b.charge(Resource::Work, v.len() as u64)?;
        b.charge(Resource::AllocationUnits, v.len() as u64)?;
        Ok(v.clone())
    }
}
impl<T: Value> Value for Vec<T> {
    fn put<C: FoundationValueCodec>(
        &self,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(
            Resource::AllocationUnits,
            (self.len() as u64).saturating_mul(core::mem::size_of::<NdfValue>() as u64),
        )?;
        if self
            .len()
            .checked_mul(core::mem::size_of::<NdfValue>())
            .is_none_or(|n| n > isize::MAX as usize)
        {
            return Err(b.stop(StopReason::AllocationLimit).into());
        }
        let mut out = Vec::with_capacity(self.len());
        for v in self {
            b.charge(Resource::Work, 1)?;
            out.push(v.put(s, c, b)?);
        }
        Ok(NdfValue::List(out))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let NdfValue::List(v) = v else {
            return Err(PortableError::Shape);
        };
        b.charge(
            Resource::AllocationUnits,
            (v.len() as u64).saturating_mul(core::mem::size_of::<T>() as u64),
        )?;
        if v.len()
            .checked_mul(core::mem::size_of::<T>())
            .is_none_or(|n| n > isize::MAX as usize)
        {
            return Err(b.stop(StopReason::AllocationLimit).into());
        }
        let mut out = Vec::with_capacity(v.len());
        for v in v {
            b.charge(Resource::Work, 1)?;
            out.push(T::read(v, s, c, b)?);
        }
        Ok(out)
    }
}
impl<T: Value> Value for Option<T> {
    fn put<C: FoundationValueCodec>(
        &self,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            None => Ok(NdfValue::None),
            Some(v) => {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<NdfValue>() as u64,
                )?;
                Ok(NdfValue::Some(Box::new(v.put(s, c, b)?)))
            }
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match v {
            NdfValue::None => Ok(None),
            NdfValue::Some(v) => Ok(Some(T::read(v, s, c, b)?)),
            _ => Err(PortableError::Shape),
        }
    }
}
