use super::{PortableError, boundary};
use crate::model::*;
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    source::Span,
    value::{Integer, NdfValue, Record, SchemaRef, Variant},
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
        let mut out = Vec::with_capacity(v.len());
        for v in v {
            b.charge(Resource::Work, 1)?;
            out.push(T::read(v, s, c, b)?);
        }
        Ok(out)
    }
}

impl Value for Span {
    fn put<C: FoundationValueCodec>(
        &self,
        _: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        c.encode_span(self, b).map_err(boundary)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        c.decode_span(v, b).map_err(boundary)
    }
}
fn copy_integer(v: &Integer, b: &mut Budget) -> Result<Integer, nepl3_core::budget::StopReason> {
    let bytes = v.as_bigint().bits().div_ceil(64).saturating_mul(8);
    b.charge(Resource::Work, bytes.saturating_add(1))?;
    b.charge(Resource::AllocationUnits, bytes)?;
    Ok(v.clone())
}
impl Value for Integer {
    fn put<C: FoundationValueCodec>(
        &self,
        _: &SchemaRef,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        Ok(NdfValue::Integer(copy_integer(self, b)?))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &SchemaRef,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let NdfValue::Integer(value) = v else {
            return Err(PortableError::Shape);
        };
        Ok(copy_integer(value, b)?)
    }
}
fn put_literal<C: FoundationValueCodec, T: Value>(
    value: &Located<T>,
    name: &str,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    record(
        s,
        name,
        [value.value.put(s, c, b)?, value.span.put(s, c, b)?],
        b,
    )
}
fn read_literal<C: FoundationValueCodec, T: Value>(
    v: &NdfValue,
    name: &str,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<Located<T>, PortableError<C::Error>> {
    b.charge(Resource::Work, (s.package.len() + name.len()) as u64 + 1)?;
    let f = fields(v, s, name, 2)?;
    Ok(Located {
        value: Value::read(&f[0], s, c, b)?,
        span: Value::read(&f[1], s, c, b)?,
    })
}
macro_rules! literal {
    ($put:ident,$read:ident,$ty:ty,$name:literal) => {
        fn $put<C: FoundationValueCodec>(
            v: &$ty,
            s: &SchemaRef,
            c: &mut C,
            b: &mut Budget,
        ) -> Result<NdfValue, PortableError<C::Error>> {
            put_literal(v, $name, s, c, b)
        }
        fn $read<C: FoundationValueCodec>(
            v: &NdfValue,
            s: &SchemaRef,
            c: &mut C,
            b: &mut Budget,
        ) -> Result<$ty, PortableError<C::Error>> {
            read_literal(v, $name, s, c, b)
        }
    };
}
literal!(put_name, read_name, NameLiteral, "NameLiteral");
literal!(put_text, read_text, TextLiteral, "TextLiteral");
literal!(put_nat, read_nat, NatLiteral, "NatLiteral");
