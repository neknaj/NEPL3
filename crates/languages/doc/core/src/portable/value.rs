use super::{PortableError, boundary};
use crate::model::*;
use crate::pages::*;
use crate::prepare::{DocPreparationPlan, DocRequirement};
use crate::print::*;
use crate::text::*;
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    origin::{Mapping, MappingKind, OriginId},
    source::{Digest, Span},
    syntax::ForeignClosure,
    value::{NdfValue, Record, SchemaRef, Variant},
    value_codec::FoundationValueCodec,
    view::ViewBundle,
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
impl Value for Digest {
    fn put<C: FoundationValueCodec>(
        &self,
        _: &SchemaRef,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::Work, 32)?;
        b.charge(Resource::AllocationUnits, 32)?;
        Ok(NdfValue::Bytes(self.0.to_vec()))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &SchemaRef,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        b.charge(Resource::Work, 32)?;
        match v {
            NdfValue::Bytes(v) => Ok(Digest(
                v.as_slice().try_into().map_err(|_| PortableError::Shape)?,
            )),
            _ => Err(PortableError::Shape),
        }
    }
}
impl Value for SchemaRef {
    fn put<C: FoundationValueCodec>(
        &self,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let package = self.package.put(s, c, b)?;
        let revision = self.revision.put(s, c, b)?;
        let digest = self.digest.put(s, c, b)?;
        record(
            c.foundation_schema(),
            "SchemaRef",
            [package, revision, digest],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, c.foundation_schema(), "SchemaRef", 3)?;
        Ok(Self {
            package: Value::read(&f[0], s, c, b)?,
            revision: Value::read(&f[1], s, c, b)?,
            digest: Value::read(&f[2], s, c, b)?,
        })
    }
}
macro_rules! foundation {
    ($ty:ty,$put:ident,$read:ident) => {
        impl Value for $ty {
            fn put<C: FoundationValueCodec>(
                &self,
                _: &SchemaRef,
                c: &mut C,
                b: &mut Budget,
            ) -> Result<NdfValue, PortableError<C::Error>> {
                c.$put(self, b).map_err(boundary)
            }
            fn read<C: FoundationValueCodec>(
                v: &NdfValue,
                _: &SchemaRef,
                c: &mut C,
                b: &mut Budget,
            ) -> Result<Self, PortableError<C::Error>> {
                c.$read(v, b).map_err(boundary)
            }
        }
    };
}
foundation!(Span, encode_span, decode_span);
foundation!(Report, encode_report, decode_report);
impl Value for StopReason {
    fn put<C: FoundationValueCodec>(
        &self,
        _: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let tag = match self {
            Self::Cancelled => "Cancelled",
            Self::SourceLimit => "SourceLimit",
            Self::WorkLimit => "WorkLimit",
            Self::DepthLimit => "DepthLimit",
            Self::NodeLimit => "NodeLimit",
            Self::AllocationLimit => "AllocationLimit",
            Self::OutputLimit => "OutputLimit",
            Self::DiagnosticLimit => "DiagnosticLimit",
            Self::EventLimit => "EventLimit",
        };
        variant(c.foundation_schema(), "StopReason", tag, [], b)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        b.charge(Resource::Work, 64)?;
        let (tag, fields) = case(v, c.foundation_schema(), "StopReason")?;
        if !fields.is_empty() {
            return Err(PortableError::Shape);
        }
        Ok(match tag {
            "Cancelled" => Self::Cancelled,
            "SourceLimit" => Self::SourceLimit,
            "WorkLimit" => Self::WorkLimit,
            "DepthLimit" => Self::DepthLimit,
            "NodeLimit" => Self::NodeLimit,
            "AllocationLimit" => Self::AllocationLimit,
            "OutputLimit" => Self::OutputLimit,
            "DiagnosticLimit" => Self::DiagnosticLimit,
            "EventLimit" => Self::EventLimit,
            _ => return Err(PortableError::Shape),
        })
    }
}
foundation!(ViewBundle, encode_views, decode_views);
foundation!(
    ForeignClosure,
    encode_foreign_closure,
    decode_foreign_closure
);
impl Value for OriginId {
    fn put<C: FoundationValueCodec>(
        &self,
        _: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            c.foundation_schema(),
            "OriginRef",
            [NdfValue::U64(self.0)],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, c.foundation_schema(), "OriginRef", 1)?;
        Ok(Self(u64::read(&f[0], s, c, b)?))
    }
}
impl Value for Mapping {
    fn put<C: FoundationValueCodec>(
        &self,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let source = self.source.put(s, c, b)?;
        let target = self.target.put(s, c, b)?;
        let kind = variant(
            c.foundation_schema(),
            "MappingKind",
            match self.kind {
                MappingKind::Exact => "Exact",
                MappingKind::Transformed => "Transformed",
            },
            [],
            b,
        )?;
        record(
            c.foundation_schema(),
            "SourceMapping",
            [source, target, kind],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &SchemaRef,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, c.foundation_schema(), "SourceMapping", 3)?;
        let (tag, empty) = case(&f[2], c.foundation_schema(), "MappingKind")?;
        if !empty.is_empty() {
            return Err(PortableError::Shape);
        }
        let kind = match tag {
            "Exact" => MappingKind::Exact,
            "Transformed" => MappingKind::Transformed,
            _ => return Err(PortableError::Shape),
        };
        Ok(Self {
            source: Span::read(&f[0], s, c, b)?,
            target: Span::read(&f[1], s, c, b)?,
            kind,
        })
    }
}
