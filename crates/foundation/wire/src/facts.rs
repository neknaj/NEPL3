//! Typed facts boundary. Transport decoding is followed by source/graph and
//! delta-authority checks; a structural NDF proof alone is insufficient.
use crate::{WireError, boundary::*, source::*, view::*};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    facts::*,
    origin::{Mapping, Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore, Span},
    value::{NdfValue, Record, SchemaRef, TypedValue, Variant},
};
trait Codec: Sized {
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
macro_rules! id_codec {
    ($ty:ident,$name:literal) => {
        impl Codec for $ty {
            fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
                id_value(self.0, $name, s, b)
            }
            fn from(
                v: &NdfValue,
                s: &SchemaRef,
                _: &SourceStore,
                _: &mut Budget,
            ) -> Result<Self, WireError> {
                Ok(Self(id_from(v, $name, s)?))
            }
        }
    };
}
id_codec!(ScopeId, "ScopeId");
id_codec!(EntityId, "EntityId");
id_codec!(OccurrenceId, "OccurrenceId");
id_codec!(RelationId, "RelationId");
id_codec!(NamespaceRef, "NamespaceRef");
id_codec!(OriginId, "OriginRef");
macro_rules! record_codec{($ty:ident,$name:literal,$count:literal,[$($field:ident:$index:literal),*])=>{impl Codec for $ty{fn value(&self,s:&SchemaRef,b:&mut Budget)->Result<NdfValue,WireError>{record(s,$name,[$(self.$field.value(s,b)?),*],b)}fn from(v:&NdfValue,s:&SchemaRef,store:&SourceStore,b:&mut Budget)->Result<Self,WireError>{let f=fields(v,s,$name,$count)?;Ok(Self{$($field:Codec::from(&f[$index],s,store,b)?),*})}}};}
record_codec!(FactNamespace,"FactNamespace",4,[schema:0,name:1,policy:2,root:3]);
record_codec!(Scope,"Scope",3,[id:0,parent:1,origin:2]);
record_codec!(Entity,"Entity",7,[id:0,scope:1,namespace:2,name:3,definition:4,selection:5,origin:6]);
record_codec!(Occurrence,"Occurrence",8,[id:0,scope:1,namespace:2,name:3,role:4,span:5,origin:6,resolution:7]);
record_codec!(Relation,"Relation",4,[id:0,source:1,target:2,payload:3]);
record_codec!(ResolutionUpdate,"ResolutionUpdate",2,[occurrence:0,resolution:1]);
record_codec!(IdRange,"IdRange",2,[start:0,end:1]);
record_codec!(FactReservation,"FactReservation",4,[scopes:0,entities:1,occurrences:2,relations:3]);
record_codec!(FactAuthority,"FactAuthority",8,[analysis_id:0,current_scope:1,namespaces:2,writable_scopes:3,import_scopes:4,resolution_updates:5,relation_sources:6,reservation:7]);
macro_rules! enum_codec{($ty:ident,$name:literal,[$($case:ident),*])=>{impl Codec for $ty{fn value(&self,s:&SchemaRef,b:&mut Budget)->Result<NdfValue,WireError>{variant(s,$name,match self{$(Self::$case=>stringify!($case)),*},[],b)}fn from(v:&NdfValue,s:&SchemaRef,_:&SourceStore,_:&mut Budget)->Result<Self,WireError>{let(case,f)=variant_parts(v,s,$name)?;if !f.is_empty(){return Err(WireError::InvalidType);}Ok(match case{$(stringify!($case)=>Self::$case),*,_=>return Err(WireError::InvalidType)})}}};}
enum_codec!(
    NamespacePolicy,
    "FactNamespacePolicy",
    [Lexical, Global, Open]
);
enum_codec!(
    OccurrenceRole,
    "OccurrenceRole",
    [Definition, Reference, Import, Export]
);
macro_rules! sum_codec{($ty:ident,$name:literal,[$($case:ident),*])=>{impl Codec for $ty{fn value(&self,s:&SchemaRef,b:&mut Budget)->Result<NdfValue,WireError>{match self{$(Self::$case(v)=>variant(s,$name,stringify!($case),[v.value(s,b)?],b)),*}}fn from(v:&NdfValue,s:&SchemaRef,store:&SourceStore,b:&mut Budget)->Result<Self,WireError>{let(case,f)=variant_parts(v,s,$name)?;let [value]=f else{return Err(WireError::InvalidType);};Ok(match case{$(stringify!($case)=>Self::$case(Codec::from(value,s,store,b)?)),*,_=>return Err(WireError::InvalidType)})}}};}
sum_codec!(
    ReferenceResolution,
    "ReferenceResolution",
    [Resolved, Unresolved, Ambiguous, Deferred]
);
sum_codec!(
    FactTarget,
    "FactTarget",
    [Scope, Entity, Occurrence, Source]
);
impl Codec for ScopeEdge {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        match self {
            Self::Import {
                from,
                to,
                namespace,
            } => variant(
                s,
                "ScopeEdge",
                "Import",
                [from.value(s, b)?, to.value(s, b)?, namespace.value(s, b)?],
                b,
            ),
            Self::Export { from, to, entity } => variant(
                s,
                "ScopeEdge",
                "Export",
                [from.value(s, b)?, to.value(s, b)?, entity.value(s, b)?],
                b,
            ),
        }
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let (case, f) = variant_parts(v, s, "ScopeEdge")?;
        let [from, to, last] = f else {
            return Err(WireError::InvalidType);
        };
        Ok(match case {
            "Import" => Self::Import {
                from: Codec::from(from, s, store, b)?,
                to: Codec::from(to, s, store, b)?,
                namespace: Codec::from(last, s, store, b)?,
            },
            "Export" => Self::Export {
                from: Codec::from(from, s, store, b)?,
                to: Codec::from(to, s, store, b)?,
                entity: Codec::from(last, s, store, b)?,
            },
            _ => return Err(WireError::InvalidType),
        })
    }
}
fn selected(registry: &SchemaRegistry) -> Result<&SchemaRef, WireError> {
    registry
        .selected("nepl3.foundation", 1)
        .ok_or(nepl3_core::schema::SchemaError::UnknownSchema.into())
}
pub fn encode_set(
    set: &FactSet,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    set.validate(registry, b, admission)?;
    let s = selected(registry)?;
    let value = record(
        s,
        "FactSet",
        [
            set.analysis_id.value(s, b)?,
            set.namespaces.value(s, b)?,
            set.scopes.value(s, b)?,
            set.entities.value(s, b)?,
            set.occurrences.value(s, b)?,
            set.relations.value(s, b)?,
            set.edges.value(s, b)?,
            sources_value(&set.sources, s, admission, b)?,
            set.origins.value(s, b)?,
            set.source_maps.value(s, b)?,
        ],
        b,
    )?;
    crate::encode_checked(&value, &expected("FactSet"), registry, b)
}
pub fn decode_set(
    bytes: &[u8],
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<FactSet, WireError> {
    let s = selected(registry)?;
    let value = crate::decode_checked(bytes, &expected("FactSet"), registry, b)?;
    let f = fields(value.value(), s, "FactSet", 10)?;
    let sources = sources_from(&f[7], s, admission, b)?;
    let store = store(&sources, b)?;
    let value = FactSet {
        analysis_id: Codec::from(&f[0], s, &store, b)?,
        namespaces: Codec::from(&f[1], s, &store, b)?,
        scopes: Codec::from(&f[2], s, &store, b)?,
        entities: Codec::from(&f[3], s, &store, b)?,
        occurrences: Codec::from(&f[4], s, &store, b)?,
        relations: Codec::from(&f[5], s, &store, b)?,
        edges: Codec::from(&f[6], s, &store, b)?,
        sources,
        origins: Codec::from(&f[8], s, &store, b)?,
        source_maps: Codec::from(&f[9], s, &store, b)?,
    };
    value.validate(registry, b, admission)?;
    Ok(value)
}
pub fn encode_delta(
    delta: &FactDelta,
    base: &CheckedFactSet<'_>,
    authority: &FactAuthority,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    let current_base = base.value().validate(registry, b, admission)?;
    delta.validate(&current_base, authority, b, admission)?;
    let s = selected(registry)?;
    let value = record(
        s,
        "FactDelta",
        [
            delta.analysis_id.value(s, b)?,
            delta.origin_base.value(s, b)?,
            delta.scopes.value(s, b)?,
            delta.entities.value(s, b)?,
            delta.occurrences.value(s, b)?,
            delta.relations.value(s, b)?,
            delta.edges.value(s, b)?,
            delta.resolutions.value(s, b)?,
            sources_value(&delta.sources, s, admission, b)?,
            delta.origins.value(s, b)?,
            delta.source_maps.value(s, b)?,
        ],
        b,
    )?;
    crate::encode_checked(&value, &expected("FactDelta"), registry, b)
}
pub fn decode_delta(
    bytes: &[u8],
    base: &CheckedFactSet<'_>,
    authority: &FactAuthority,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<FactDelta, WireError> {
    let current_base = base.value().validate(registry, b, admission)?;
    let s = selected(registry)?;
    let value = crate::decode_checked(bytes, &expected("FactDelta"), registry, b)?;
    let f = fields(value.value(), s, "FactDelta", 11)?;
    let sources = sources_from(&f[8], s, admission, b)?;
    let mut store = store(&base.value().sources, b)?;
    for source in &sources {
        store.insert(source.clone_with_budget(b)?)?;
    }
    let value = FactDelta {
        analysis_id: Codec::from(&f[0], s, &store, b)?,
        origin_base: Codec::from(&f[1], s, &store, b)?,
        scopes: Codec::from(&f[2], s, &store, b)?,
        entities: Codec::from(&f[3], s, &store, b)?,
        occurrences: Codec::from(&f[4], s, &store, b)?,
        relations: Codec::from(&f[5], s, &store, b)?,
        edges: Codec::from(&f[6], s, &store, b)?,
        resolutions: Codec::from(&f[7], s, &store, b)?,
        sources,
        origins: Codec::from(&f[9], s, &store, b)?,
        source_maps: Codec::from(&f[10], s, &store, b)?,
    };
    value.validate(&current_base, authority, b, admission)?;
    Ok(value)
}
