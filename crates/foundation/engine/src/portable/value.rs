use super::{PortableError, boundary};
use crate::{
    package::{EntryContext, PackageIdentity, ReadSpecId},
    recovery::ForeignStep,
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::{Digest, Span},
    syntax::{NodeRef, TokenRef},
    value::{NdfValue, Record, SchemaRef, Variant},
    value_codec::FoundationValueCodec,
};

pub(super) struct Schemas<'a> {
    pub engine: &'a SchemaRef,
    pub foundation: &'a SchemaRef,
}
impl<'a> Schemas<'a> {
    pub fn new<E>(registry: &'a SchemaRegistry) -> Result<Self, PortableError<E>> {
        Ok(Self {
            engine: registry
                .selected("nepl3.engine", 1)
                .ok_or(SchemaError::UnknownSchema)?,
            foundation: registry
                .selected("nepl3.foundation", 1)
                .ok_or(SchemaError::UnknownSchema)?,
        })
    }
}
pub(super) fn expected<E>(name: &str, b: &mut Budget) -> Result<TypeDescriptor, PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        ("nepl3.engine".len() + name.len()) as u64,
    )?;
    Ok(TypeDescriptor::Named(TypeRef {
        package: "nepl3.engine".into(),
        revision: 1,
        name: name.into(),
    }))
}
pub(super) fn push<T, E>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), PortableError<E>> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(item);
    Ok(())
}
pub(super) fn record<E, const N: usize>(
    schema: &SchemaRef,
    name: &str,
    fields: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + N * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Record(Record {
        schema: schema.clone(),
        kind: name.into(),
        fields: Vec::from(fields),
    }))
}
pub(super) fn variant<E, const N: usize>(
    schema: &SchemaRef,
    name: &str,
    case: &str,
    fields: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + case.len() + N * core::mem::size_of::<NdfValue>())
            as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: name.into(),
        variant: case.into(),
        fields: Vec::from(fields),
    }))
}
pub(super) fn fields<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    name: &str,
    n: usize,
) -> Result<&'a [NdfValue], PortableError<E>> {
    match v {
        NdfValue::Record(v) if &v.schema == s && v.kind == name && v.fields.len() == n => {
            Ok(&v.fields)
        }
        _ => Err(PortableError::Shape),
    }
}
pub(super) fn parts<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    name: &str,
) -> Result<(&'a str, &'a [NdfValue]), PortableError<E>> {
    match v {
        NdfValue::Variant(v) if &v.schema == s && v.type_name == name => {
            Ok((&v.variant, &v.fields))
        }
        _ => Err(PortableError::Shape),
    }
}
pub(super) fn list<E>(v: &NdfValue) -> Result<&[NdfValue], PortableError<E>> {
    match v {
        NdfValue::List(v) => Ok(v),
        _ => Err(PortableError::Shape),
    }
}
pub(super) trait Value: Sized {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>>;
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>>;
}
impl Value for String {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::AllocationUnits, self.len() as u64)?;
        Ok(NdfValue::Text(self.clone()))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match v {
            NdfValue::Text(v) => {
                b.charge(Resource::AllocationUnits, v.len() as u64)?;
                Ok(v.clone())
            }
            _ => Err(PortableError::Shape),
        }
    }
}
macro_rules! scalar {
    ($t:ty,$case:ident) => {
        impl Value for $t {
            fn value<C: FoundationValueCodec>(
                &self,
                _: &Schemas<'_>,
                _: &mut C,
                _: &mut Budget,
            ) -> Result<NdfValue, PortableError<C::Error>> {
                Ok(NdfValue::$case(*self))
            }
            fn read<C: FoundationValueCodec>(
                v: &NdfValue,
                _: &Schemas<'_>,
                _: &mut C,
                _: &mut Budget,
            ) -> Result<Self, PortableError<C::Error>> {
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
impl Value for Digest {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::AllocationUnits, 32)?;
        Ok(NdfValue::Bytes(self.0.to_vec()))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        _: &mut C,
        _: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match v {
            NdfValue::Bytes(v) => Ok(Self(
                v.as_slice().try_into().map_err(|_| PortableError::Shape)?,
            )),
            _ => Err(PortableError::Shape),
        }
    }
}
impl Value for Span {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        c.encode_span(self, b).map_err(boundary)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        c.decode_span(v, b).map_err(boundary)
    }
}
impl<T: Value> Value for Vec<T> {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let mut out = Vec::new();
        for v in self {
            push(&mut out, v.value(s, c, b)?, b)?;
        }
        Ok(NdfValue::List(out))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let mut out = Vec::new();
        for v in list(v)? {
            push(&mut out, T::read(v, s, c, b)?, b)?;
        }
        Ok(out)
    }
}
macro_rules! id {
    ($ty:ident,$owner:ident,$name:literal) => {
        impl Value for $ty {
            fn value<C: FoundationValueCodec>(
                &self,
                s: &Schemas<'_>,
                _: &mut C,
                b: &mut Budget,
            ) -> Result<NdfValue, PortableError<C::Error>> {
                record(s.$owner, $name, [NdfValue::U64(self.0)], b)
            }
            fn read<C: FoundationValueCodec>(
                v: &NdfValue,
                s: &Schemas<'_>,
                c: &mut C,
                b: &mut Budget,
            ) -> Result<Self, PortableError<C::Error>> {
                Ok(Self(<u64 as Value>::read(
                    &fields(v, s.$owner, $name, 1)?[0],
                    s,
                    c,
                    b,
                )?))
            }
        }
    };
}
id!(NodeRef, foundation, "NodeRef");
id!(TokenRef, foundation, "TokenRef");
id!(ReadSpecId, engine, "ReadSpecId");
macro_rules! record_value{($ty:ident,$owner:ident,$name:literal,$n:literal,[$($field:ident:$index:literal),*])=>{impl Value for $ty{
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{record(s.$owner,$name,[$(self.$field.value(s,c,b)?),*],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>>{let f=fields(v,s.$owner,$name,$n)?;Ok(Self{$($field:Value::read(&f[$index],s,c,b)?),*})}}};}
record_value!(SchemaRef,foundation,"SchemaRef",3,[package:0,revision:1,digest:2]);
record_value!(PackageIdentity,engine,"PackageIdentity",2,[schema:0,semantic_digest:1]);
record_value!(EntryContext,engine,"EntryContext",4,[package:0,alias:1,category:2,mode:3]);
record_value!(ForeignStep,engine,"ForeignStep",2,[node:0,field:1]);
mod head;
