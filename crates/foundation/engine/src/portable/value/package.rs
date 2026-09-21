//! Package declaration values; enclosing package admission supplies source authority.
use super::*;
use crate::{package::*, recovery::*};
use nepl3_core::origin::OriginId;
use nepl3_reader::builtin::BuiltinReader;

impl Value for BuiltinReader {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        variant(
            s.reader,
            "BuiltinReader",
            match self {
                Self::Name => "Name",
                Self::Text => "Text",
                Self::Nat => "Nat",
                Self::Number => "Number",
                Self::Lang => "Lang",
                Self::Trivia => "Trivia",
            },
            [],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        _: &mut C,
        _: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match parts(v, s.reader, "BuiltinReader")? {
            ("Name", []) => Ok(Self::Name),
            ("Text", []) => Ok(Self::Text),
            ("Nat", []) => Ok(Self::Nat),
            ("Number", []) => Ok(Self::Number),
            ("Lang", []) => Ok(Self::Lang),
            ("Trivia", []) => Ok(Self::Trivia),
            _ => Err(PortableError::Shape),
        }
    }
}
impl Value for ReadSpec {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        macro_rules! out{($case:literal,$($v:expr),*)=>{variant(s.engine,"ReadSpec",$case,[$($v.value(s,c,b)?),*],b)};}
        match self {
            Self::Builtin {
                reader,
                kind,
                token_kind,
            } => out!("Builtin", reader, kind, token_kind),
            Self::Local { category } => out!("Local", category),
            Self::Foreign { alias, category } => out!("Foreign", alias, category),
            Self::WithMode { mode, read } => out!("WithMode", mode, read),
            Self::ListOf { element, cons, nil } => out!("ListOf", element, cons, nil),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        macro_rules! read {
            ($v:expr) => {
                Value::read($v, s, c, b)?
            };
        }
        Ok(match parts(v, s.engine, "ReadSpec")? {
            ("Builtin", [reader, kind, token_kind]) => Self::Builtin {
                reader: read!(reader),
                kind: read!(kind),
                token_kind: read!(token_kind),
            },
            ("Local", [category]) => Self::Local {
                category: read!(category),
            },
            ("Foreign", [alias, category]) => Self::Foreign {
                alias: read!(alias),
                category: read!(category),
            },
            ("WithMode", [mode, value]) => Self::WithMode {
                mode: read!(mode),
                read: read!(value),
            },
            ("ListOf", [element, cons, nil]) => Self::ListOf {
                element: read!(element),
                cons: read!(cons),
                nil: read!(nil),
            },
            _ => return Err(PortableError::Shape),
        })
    }
}

impl Value for TypeDescriptor {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        c.encode_type_descriptor(self, b).map_err(boundary)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        c.decode_type_descriptor(v, b).map_err(boundary)
    }
}
id!(OriginId, foundation, "OriginRef");
record_value!(Category,engine,"Category",2,[name:0,mode:1]);
record_value!(Form,engine,"Form",7,[category:0,kind:1,spelling:2,fields:3,binding:4,styles:5,selection_rules:6]);
record_value!(Leaf,engine,"Leaf",7,[category:0,kind:1,token_kind:2,payload:3,binding:4,styles:5,selection_rules:6]);
record_value!(Namespace,engine,"Namespace",2,[name:0,policy:1]);
record_value!(ExtensionRequirement,engine,"ExtensionRequirement",7,[alias:0,provider:1,signature:2,operation:3,input:4,output:5,pure:6]);
record_value!(DeclarationOrigin,engine,"DeclarationOrigin",4,[kind:0,name:1,category:2,origin:3]);
record_value!(SyncToken,engine,"SyncToken",3,[ancestor_category:0,kind:1,spelling:2]);
record_value!(RecoveryRule,engine,"RecoveryRule",3,[category:0,unexpected:1,synchronization:2]);
record_value!(RecoveryPlan,engine,"RecoveryPlan",2,[default_unexpected:0,rules:1]);

macro_rules! enumeration {($ty:ident,[$($case:ident),*])=>{impl Value for $ty {
    fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,_:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {variant(s.engine,stringify!($ty),match self{$(Self::$case=>stringify!($case)),*},[],b)}
    fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,_:&mut C,_:&mut Budget)->Result<Self,PortableError<C::Error>> {let NdfValue::Variant(v)=v else{return Err(PortableError::Shape)};if &v.schema!=s.engine || v.type_name!=stringify!($ty) || !v.fields.is_empty(){return Err(PortableError::Shape)}match v.variant.as_str(){$(stringify!($case)=>Ok(Self::$case),)* _=>Err(PortableError::Shape)}}
}};}
enumeration!(NamespacePolicy, [Lexical, Global, Open]);
enumeration!(
    DeclarationKind,
    [Category, Mode, Reader, Form, Leaf, Namespace, Extension]
);
enumeration!(UnexpectedPolicy, [ConsumeToken, PreserveRemainder]);

fn parts<'a, E>(
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
impl Value for NameSelector {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::SelfValue => variant(s.engine, "NameSelector", "SelfValue", [], b),
            Self::Field(v) => variant(s.engine, "NameSelector", "Field", [v.value(s, c, b)?], b),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        match parts(v, s.engine, "NameSelector")? {
            ("SelfValue", []) => Ok(Self::SelfValue),
            ("Field", [v]) => Ok(Self::Field(Value::read(v, s, c, b)?)),
            _ => Err(PortableError::Shape),
        }
    }
}
impl Value for Binding {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        macro_rules! out{($case:literal,$($v:expr),*)=>{variant(s.engine,"Binding",$case,[$($v.value(s,c,b)?),*],b)};}
        match self {
            Self::None => variant(s.engine, "Binding", "None", [], b),
            Self::Visit(v) => out!("Visit", v),
            Self::Group(v) => out!("Group", v),
            Self::Scope(v) => out!("Scope", v),
            Self::Bind { namespace, name } => out!("Bind", namespace, name),
            Self::Reference { namespace, name } => out!("Reference", namespace, name),
            Self::Export { namespace, name } => out!("Export", namespace, name),
            Self::Import(v) => out!("Import", v),
            Self::Propagate(v) => out!("Propagate", v),
            Self::Sequential { declarations, body } => out!("Sequential", declarations, body),
            Self::Recursive { declarations, body } => out!("Recursive", declarations, body),
            Self::Custom(v) => out!("Custom", v),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        macro_rules! read {
            ($v:expr) => {
                Value::read($v, s, c, b)?
            };
        }
        Ok(match parts(v, s.engine, "Binding")? {
            ("None", []) => Self::None,
            ("Visit", [v]) => Self::Visit(read!(v)),
            ("Group", [v]) => Self::Group(read!(v)),
            ("Scope", [v]) => Self::Scope(read!(v)),
            ("Bind", [namespace, name]) => Self::Bind {
                namespace: read!(namespace),
                name: read!(name),
            },
            ("Reference", [namespace, name]) => Self::Reference {
                namespace: read!(namespace),
                name: read!(name),
            },
            ("Export", [namespace, name]) => Self::Export {
                namespace: read!(namespace),
                name: read!(name),
            },
            ("Import", [v]) => Self::Import(read!(v)),
            ("Propagate", [v]) => Self::Propagate(read!(v)),
            ("Sequential", [declarations, body]) => Self::Sequential {
                declarations: read!(declarations),
                body: read!(body),
            },
            ("Recursive", [declarations, body]) => Self::Recursive {
                declarations: read!(declarations),
                body: read!(body),
            },
            ("Custom", [v]) => Self::Custom(read!(v)),
            _ => return Err(PortableError::Shape),
        })
    }
}
