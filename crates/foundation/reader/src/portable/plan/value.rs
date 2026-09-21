use super::*;
#[cfg(test)]
mod tests;
use crate::plan::*;
use crate::{
    builtin::BuiltinReader,
    tokenizer::{ReaderMode, SkipRule, TakeRule, TokenReader},
};
use nepl3_core::{
    value::{KindRef, OperationRef, Variant},
    view::{FallbackRole, PresentationClass},
};

pub(super) struct Context<'a> {
    reader: &'a SchemaRef,
    foundation: &'a SchemaRef,
}
impl<'a> Context<'a> {
    pub(super) fn new<E>(registry: &'a SchemaRegistry) -> Result<Self, PortableError<E>> {
        Ok(Self {
            reader: registry
                .selected("nepl3.reader", 1)
                .ok_or(PortableError::Shape)?,
            foundation: registry
                .selected("nepl3.foundation", 1)
                .ok_or(PortableError::Shape)?,
        })
    }
}
pub(super) trait Value: Sized {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>>;
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>>;
}
impl Value for String {
    fn encode<C: FoundationValueCodec>(
        &self,
        _: &Context<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::Work, self.len() as u64)?;
        text(self, b)
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Context<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        if let NdfValue::Text(v) = v {
            b.charge(Resource::Work, v.len() as u64)?;
        }
        string(v, b)
    }
}
macro_rules! scalar {
    ($ty:ty,$variant:ident) => {
        impl Value for $ty {
            fn encode<C: FoundationValueCodec>(
                &self,
                _: &Context<'_>,
                _: &mut C,
                b: &mut Budget,
            ) -> Result<NdfValue, PortableError<C::Error>> {
                b.charge(Resource::Work, 1)?;
                Ok(NdfValue::$variant(*self))
            }
            fn decode<C: FoundationValueCodec>(
                v: &NdfValue,
                _: &Context<'_>,
                _: &mut C,
                b: &mut Budget,
            ) -> Result<Self, PortableError<C::Error>> {
                b.charge(Resource::Work, 1)?;
                match v {
                    NdfValue::$variant(v) => Ok(*v),
                    _ => Err(PortableError::Shape),
                }
            }
        }
    };
}
scalar!(u64, U64);
scalar!(bool, Bool);
impl<T: Value> Value for Vec<T> {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let mut out = Vec::new();
        for value in self {
            b.charge(Resource::Work, 1)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            out.push(value.encode(s, c, b)?);
        }
        Ok(NdfValue::List(out))
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let NdfValue::List(values) = v else {
            return Err(PortableError::Shape);
        };
        let mut out = Vec::new();
        for value in values {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
            out.push(T::decode(value, s, c, b)?);
        }
        Ok(out)
    }
}
impl Value for TypeDescriptor {
    fn encode<C: FoundationValueCodec>(
        &self,
        _: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        c.encode_type_descriptor(self, b).map_err(boundary)
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        c.decode_type_descriptor(v, b).map_err(boundary)
    }
}
impl Value for SchemaRef {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        schema_value(self, s.foundation, b)
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        schema_from(v, s.foundation, b)
    }
}
macro_rules! record_value {($ty:ident,$owner:ident,$n:literal,[$($field:ident:$index:literal),*])=>{impl Value for $ty {
    fn encode<C:FoundationValueCodec>(&self,s:&Context<'_>,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> { b.charge(Resource::Work,1)?; record(s.$owner,stringify!($ty),[$(self.$field.encode(s,c,b)?),*],b) }
    fn decode<C:FoundationValueCodec>(v:&NdfValue,s:&Context<'_>,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> { b.charge(Resource::Work,1)?; let f=fields(v,s.$owner,stringify!($ty),$n)?; Ok(Self{$($field:Value::decode(&f[$index],s,c,b)?),*}) }
}};}
record_value!(KindRef,foundation,2,[schema:0,local_kind:1]);
record_value!(OperationRef,foundation,2,[schema:0,name:1]);
record_value!(PresentationClass,foundation,3,[schema:0,name:1,fallback:2]);
record_value!(ReaderPlan,reader,5,[schema:0,state_type:1,expressions:2,rules:3,providers:4]);
record_value!(ReaderRule,reader,3,[name:0,root:1,output:2]);
record_value!(ProviderSignature,reader,7,[operation:0,kind:1,value_input:2,value_output:3,state_type:4,continuation_type:5,pure:6]);
impl Value for ReaderId {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(s.reader, "ReaderId", [NdfValue::U64(self.0)], b)
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        b.charge(Resource::Work, 1)?;
        Ok(Self(number(&fields(v, s.reader, "ReaderId", 1)?[0])?))
    }
}
fn variant<E, const N: usize>(
    s: &SchemaRef,
    name: &str,
    case: &str,
    fields: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        (s.package.len() + name.len() + case.len() + N * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: s.clone(),
        type_name: name.into(),
        variant: case.into(),
        fields: Vec::from(fields),
    }))
}
fn parts<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    name: &str,
    b: &mut Budget,
) -> Result<(&'a str, &'a [NdfValue]), PortableError<E>> {
    b.charge(Resource::Work, 1)?;
    match v {
        NdfValue::Variant(v) if &v.schema == s && v.type_name == name => {
            Ok((&v.variant, &v.fields))
        }
        _ => Err(PortableError::Shape),
    }
}
macro_rules! unit_enum {($ty:ident,$owner:ident,[$($case:ident),*])=>{impl Value for $ty {
    fn encode<C:FoundationValueCodec>(&self,s:&Context<'_>,_:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {variant(s.$owner,stringify!($ty),match self{$(Self::$case=>stringify!($case)),*},[],b)}
    fn decode<C:FoundationValueCodec>(v:&NdfValue,s:&Context<'_>,_:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {match parts(v,s.$owner,stringify!($ty),b)? { $((stringify!($case),[])=>Ok(Self::$case),)* _=>Err(PortableError::Shape)}}
}};}
unit_enum!(ProviderKind, reader, [Read, Transform, Dependent]);
unit_enum!(
    BuiltinReader,
    reader,
    [Name, Text, Nat, Number, Lang, Trivia]
);
record_value!(ReaderMode,reader,3,[name:0,skip:1,take:2]);
record_value!(SkipRule,reader,1,[reader:0]);
record_value!(TakeRule,reader,2,[reader:0,kind:1]);
impl Value for TokenReader {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Builtin(v) => {
                variant(s.reader, "TokenReader", "Builtin", [v.encode(s, c, b)?], b)
            }
            Self::Rule(v) => variant(s.reader, "TokenReader", "Rule", [v.encode(s, c, b)?], b),
        }
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match parts(v, s.reader, "TokenReader", b)? {
            ("Builtin", [v]) => Ok(Self::Builtin(Value::decode(v, s, c, b)?)),
            ("Rule", [v]) => Ok(Self::Rule(Value::decode(v, s, c, b)?)),
            _ => Err(PortableError::Shape),
        }
    }
}
unit_enum!(
    FallbackRole,
    foundation,
    [Content, Marker, Delimiter, Name, Quantity, Annotation]
);

impl Value for CharClass {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let unit = match self {
            Self::Any => Some("Any"),
            Self::Whitespace => Some("Whitespace"),
            Self::IdentifierStart => Some("IdentifierStart"),
            Self::IdentifierContinue => Some("IdentifierContinue"),
            Self::Digit => Some("Digit"),
            Self::AsciiLetter => Some("AsciiLetter"),
            _ => None,
        };
        if let Some(name) = unit {
            return variant(s.reader, "CharClass", name, [], b);
        }
        match self {
            Self::Chars(v) | Self::Except(v) => variant(
                s.reader,
                "CharClass",
                if matches!(self, Self::Chars(_)) {
                    "Chars"
                } else {
                    "Except"
                },
                [v.encode(s, c, b)?],
                b,
            ),
            Self::Range { lo, hi } => {
                let mut low = [0; 4];
                let mut high = [0; 4];
                variant(
                    s.reader,
                    "CharClass",
                    "Range",
                    [
                        text(lo.encode_utf8(&mut low), b)?,
                        text(hi.encode_utf8(&mut high), b)?,
                    ],
                    b,
                )
            }
            _ => Err(PortableError::Shape),
        }
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.reader, "CharClass", b)? {
            ("Any", []) => Self::Any,
            ("Whitespace", []) => Self::Whitespace,
            ("IdentifierStart", []) => Self::IdentifierStart,
            ("IdentifierContinue", []) => Self::IdentifierContinue,
            ("Digit", []) => Self::Digit,
            ("AsciiLetter", []) => Self::AsciiLetter,
            ("Chars", [v]) => Self::Chars(String::decode(v, s, c, b)?),
            ("Except", [v]) => Self::Except(String::decode(v, s, c, b)?),
            ("Range", [lo, hi]) => {
                fn scalar<E>(v: &NdfValue) -> Result<char, PortableError<E>> {
                    let NdfValue::Text(v) = v else {
                        return Err(PortableError::Shape);
                    };
                    let mut chars = v.chars();
                    let first = chars.next().ok_or(PortableError::Shape)?;
                    if chars.next().is_some() {
                        return Err(PortableError::Shape);
                    }
                    Ok(first)
                }
                let lo = scalar(lo)?;
                let hi = scalar(hi)?;
                if lo > hi {
                    return Err(PortableError::Shape);
                }
                Self::Range { lo, hi }
            }
            _ => return Err(PortableError::Shape),
        })
    }
}

impl Value for ReaderExpr {
    fn encode<C: FoundationValueCodec>(
        &self,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        macro_rules! out {($name:literal,$($value:expr),*)=>{variant(s.reader,"ReaderExpr",$name,[$($value.encode(s,c,b)?),*],b)};}
        match self {
            Self::Literal(v) => out!("Literal", v),
            Self::Scalar(v) => out!("Scalar", v),
            Self::Seq(v) => out!("Seq", v),
            Self::Choice(v) => out!("Choice", v),
            Self::Many(v) => out!("Many", v),
            Self::Some(v) => out!("Some", v),
            Self::Optional(v) => out!("Optional", v),
            Self::Repeat { min, max, body } => out!("Repeat", min, max, body),
            Self::Look(v) => out!("Look", v),
            Self::Not(v) => out!("Not", v),
            Self::Commit(v) => out!("Commit", v),
            Self::Capture { name, body } => out!("Capture", name, body),
            Self::Region { class, body } => out!("Region", class, body),
            Self::Node { kind, body } => out!("Node", kind, body),
            Self::Discard(v) => out!("Discard", v),
            Self::Ref(v) => out!("Ref", v),
            Self::Decode { provider, body } => out!("Decode", provider, body),
            Self::Map { provider, body } => out!("Map", provider, body),
            Self::Then { first, provider } => out!("Then", first, provider),
            Self::Call(v) => out!("Call", v),
            Self::Eof => variant(s.reader, "ReaderExpr", "Eof", [], b),
            Self::TakeCount(v) => out!("TakeCount", v),
            Self::Until(v) => out!("Until", v),
        }
    }
    fn decode<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Context<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        macro_rules! read {
            ($value:expr) => {
                Value::decode($value, s, c, b)?
            };
        }
        Ok(match parts(v, s.reader, "ReaderExpr", b)? {
            ("Literal", [v]) => Self::Literal(read!(v)),
            ("Scalar", [v]) => Self::Scalar(read!(v)),
            ("Seq", [v]) => Self::Seq(read!(v)),
            ("Choice", [v]) => Self::Choice(read!(v)),
            ("Many", [v]) => Self::Many(read!(v)),
            ("Some", [v]) => Self::Some(read!(v)),
            ("Optional", [v]) => Self::Optional(read!(v)),
            ("Repeat", [min, max, body]) => Self::Repeat {
                min: read!(min),
                max: read!(max),
                body: read!(body),
            },
            ("Look", [v]) => Self::Look(read!(v)),
            ("Not", [v]) => Self::Not(read!(v)),
            ("Commit", [v]) => Self::Commit(read!(v)),
            ("Capture", [name, body]) => Self::Capture {
                name: read!(name),
                body: read!(body),
            },
            ("Region", [class, body]) => Self::Region {
                class: read!(class),
                body: read!(body),
            },
            ("Node", [kind, body]) => Self::Node {
                kind: read!(kind),
                body: read!(body),
            },
            ("Discard", [v]) => Self::Discard(read!(v)),
            ("Ref", [v]) => Self::Ref(read!(v)),
            ("Decode", [provider, body]) => Self::Decode {
                provider: read!(provider),
                body: read!(body),
            },
            ("Map", [provider, body]) => Self::Map {
                provider: read!(provider),
                body: read!(body),
            },
            ("Then", [first, provider]) => Self::Then {
                first: read!(first),
                provider: read!(provider),
            },
            ("Call", [v]) => Self::Call(read!(v)),
            ("Eof", []) => Self::Eof,
            ("TakeCount", [v]) => Self::TakeCount(read!(v)),
            ("Until", [v]) => Self::Until(read!(v)),
            _ => return Err(PortableError::Shape),
        })
    }
}
