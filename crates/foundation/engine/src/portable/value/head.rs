use super::*;
use crate::{
    head::*,
    package::{BindingId, FieldSpec, SelectionRule, StyleRule, StyleSelector},
    selection::{HeadProviderRef, HeadShape},
};
use alloc::boxed::Box;
use nepl3_core::{
    budget::{Limits, StopReason, Usage},
    diagnostic::{Severity, TraceOverflow},
    source::{SourceId, SourceRef},
    syntax::EnvironmentRef,
    value::{KindRef, NdfScalar, OperationRef, TypedValue},
    view::{FallbackRole, PresentationClass},
};
impl<T: Value> Value for Option<T> {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        Ok(match self {
            Some(v) => {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<NdfValue>() as u64,
                )?;
                NdfValue::Some(Box::new(v.value(s, c, b)?))
            }
            None => NdfValue::None,
        })
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
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
impl<T: Value> Value for Box<T> {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        self.as_ref().value(s, c, b)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
        Ok(Box::new(T::read(v, s, c, b)?))
    }
}
impl Value for NdfValue {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        Ok(self.clone_with_budget(b)?)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(v.clone_with_budget(b)?)
    }
}
impl Value for TypedValue {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        Ok(match self.clone_with_budget(b)? {
            Self::Record(v) => NdfValue::Record(v),
            Self::Variant(v) => NdfValue::Variant(v),
        })
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        v.charge_clone(b)?;
        match v {
            NdfValue::Record(v) => Ok(Self::Record(v.clone())),
            NdfValue::Variant(v) => Ok(Self::Variant(v.clone())),
            _ => Err(PortableError::Shape),
        }
    }
}
impl Value for Vec<u8> {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::Work, self.len() as u64)?;
        b.charge(Resource::AllocationUnits, self.len() as u64)?;
        Ok(NdfValue::Bytes(self.clone()))
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        if let NdfValue::Bytes(v) = v {
            b.charge(Resource::Work, v.len() as u64)?;
            b.charge(Resource::AllocationUnits, v.len() as u64)?;
            Ok(v.clone())
        } else {
            Err(PortableError::Shape)
        }
    }
}
impl Value for SourceId {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        self.0.value(s, c, b)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(Self(String::read(v, s, c, b)?))
    }
}
impl Value for NdfScalar {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let bytes = match self {
            Self::Text(v) => v.len() as u64,
            Self::Bytes(v) => v.len() as u64,
            Self::Integer(v) => v.as_bigint().bits().div_ceil(8),
            Self::Rational(v) => {
                v.numerator().as_bigint().bits().div_ceil(8) + v.denominator().bits().div_ceil(8)
            }
            _ => 0,
        };
        b.charge(Resource::Work, bytes + 1)?;
        b.charge(Resource::AllocationUnits, bytes)?;
        let value = match self {
            Self::Unit => NdfValue::Unit,
            Self::Bool(v) => NdfValue::Bool(*v),
            Self::U64(v) => NdfValue::U64(*v),
            Self::Integer(v) => NdfValue::Integer(v.clone()),
            Self::Rational(v) => NdfValue::Rational(v.clone()),
            Self::Text(v) => NdfValue::Text(v.clone()),
            Self::Bytes(v) => NdfValue::Bytes(v.clone()),
        };
        let _ = (s, c);
        Ok(value)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        v.charge_clone(b)?;
        Ok(match v {
            NdfValue::Unit => Self::Unit,
            NdfValue::Bool(v) => Self::Bool(*v),
            NdfValue::U64(v) => Self::U64(*v),
            NdfValue::Integer(v) => Self::Integer(v.clone()),
            NdfValue::Rational(v) => Self::Rational(v.clone()),
            NdfValue::Text(v) => Self::Text(v.clone()),
            NdfValue::Bytes(v) => Self::Bytes(v.clone()),
            _ => return Err(PortableError::Shape),
        })
    }
}
macro_rules! enum_value {($ty:ident,$owner:ident,$name:literal,[$($case:ident),*])=>{impl Value for $ty {
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,_:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{variant(s.$owner,$name,match self{$(Self::$case=>stringify!($case)),*},[],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,_:&mut C,_:&mut Budget)->Result<Self,PortableError<C::Error>>{let(case,f)=parts(v,s.$owner,$name)?;if !f.is_empty(){return Err(PortableError::Shape);}Ok(match case{$(stringify!($case)=>Self::$case),*,_=>return Err(PortableError::Shape)})}}};}
enum_value!(
    FallbackRole,
    foundation,
    "FallbackRole",
    [Content, Marker, Delimiter, Name, Quantity, Annotation]
);
enum_value!(
    Severity,
    foundation,
    "Severity",
    [Error, Warning, Information, Hint]
);
enum_value!(
    StopReason,
    foundation,
    "StopReason",
    [
        Cancelled,
        SourceLimit,
        WorkLimit,
        DepthLimit,
        NodeLimit,
        AllocationLimit,
        OutputLimit,
        DiagnosticLimit,
        EventLimit
    ]
);
id!(BindingId, engine, "BindingId");
id!(ProjectedNodeRef, engine, "ProjectedNodeRef");
record_value!(KindRef,foundation,"KindRef",2,[schema:0,local_kind:1]);
record_value!(OperationRef,foundation,"OperationRef",2,[schema:0,name:1]);
record_value!(SourceRef,foundation,"SourceRef",3,[source_id:0,revision:1,digest:2]);
record_value!(EnvironmentRef,foundation,"EnvironmentRef",2,[id:0,digest:1]);
record_value!(PresentationClass,foundation,"PresentationClass",3,[schema:0,name:1,fallback:2]);
record_value!(Usage,foundation,"Usage",8,[source_bytes:0,work:1,depth:2,nodes:3,allocation_units:4,output_bytes:5,diagnostics:6,events:7]);
record_value!(Limits,foundation,"Limits",8,[source_bytes:0,work:1,depth:2,nodes:3,allocation_units:4,output_bytes:5,diagnostics:6,events:7]);
record_value!(TraceOverflow,foundation,"TraceOverflow",1,[dropped:0]);
record_value!(FieldSpec,engine,"FieldSpec",2,[name:0,read:1]);
record_value!(StyleRule,engine,"StyleRule",2,[selector:0,class:1]);
record_value!(SelectionRule,engine,"SelectionRule",2,[selector:0,priority:1]);
record_value!(HeadShape,engine,"HeadShape",5,[kind:0,fields:1,binding:2,styles:3,selection_rules:4]);
record_value!(HeadProviderRef,engine,"HeadProviderRef",2,[shape:0,child_context:1]);
record_value!(ProjectedSpan,engine,"ProjectedSpan",3,[source:0,start:1,end:2]);
record_value!(SourceWindow,engine,"SourceWindow",2,[span:0,bytes:1]);
record_value!(ProjectedToken,engine,"ProjectedToken",3,[kind:0,head:1,payload:2]);
record_value!(ProjectedHead,engine,"ProjectedHead",2,[token:0,window:1]);
record_value!(ProjectedNode,engine,"ProjectedNode",6,[schema:0,kind:1,head:2,cover:3,token:4,fields:5]);
record_value!(ProjectedSyntax,engine,"ProjectedSyntax",3,[nodes:0,roots:1,windows:2]);
record_value!(HeadCallIdentity,engine,"HeadCallIdentity",5,[session_id:0,call_id:1,operation:2,profile_digest:3,execution_digest:4]);
record_value!(HeadCall,engine,"HeadCall",6,[identity:0,depth_base:1,entry:2,environment:3,head:4,request:5]);
record_value!(HeadReply,engine,"HeadReply",3,[identity:0,outcome:1,report:2]);
record_value!(ProjectedRelated,engine,"ProjectedRelated",3,[span:0,code:1,arguments:2]);
record_value!(ProjectedEdit,engine,"ProjectedEdit",3,[span:0,expected_digest:1,replacement:2]);
record_value!(ProjectedFix,engine,"ProjectedFix",2,[id:0,edits:1]);
record_value!(ProjectedDiagnostic,engine,"ProjectedDiagnostic",8,[schema:0,code:1,severity:2,stage:3,arguments:4,primary:5,related:6,fixes:7]);
record_value!(ProjectedEvent,engine,"ProjectedEvent",5,[schema:0,kind:1,operation_path:2,span:3,payload:4]);
record_value!(ProjectedReport,engine,"ProjectedReport",4,[diagnostics:0,events:1,trace_overflow:2,usage:3]);
impl Value for StyleSelector {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Head => variant(s.engine, "StyleSelector", "Head", [], b),
            Self::SelfValue => variant(s.engine, "StyleSelector", "SelfValue", [], b),
            Self::Field(v) => variant(s.engine, "StyleSelector", "Field", [v.value(s, c, b)?], b),
            Self::Capture(v) => {
                variant(s.engine, "StyleSelector", "Capture", [v.value(s, c, b)?], b)
            }
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "StyleSelector")? {
            ("Head", []) => Self::Head,
            ("SelfValue", []) => Self::SelfValue,
            ("Field", [v]) => Self::Field(Value::read(v, s, c, b)?),
            ("Capture", [v]) => Self::Capture(Value::read(v, s, c, b)?),
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for ProjectedFieldValue {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let name = "ProjectedFieldValue";
        match self {
            Self::Atom(v) => variant(s.engine, name, "Atom", [v.value(s, c, b)?], b),
            Self::Child(v) => variant(s.engine, name, "Child", [v.value(s, c, b)?], b),
            Self::Children(v) => variant(s.engine, name, "Children", [v.value(s, c, b)?], b),
            Self::Foreign {
                schema,
                category,
                root,
                environment,
            } => variant(
                s.engine,
                name,
                "Foreign",
                [
                    schema.value(s, c, b)?,
                    category.value(s, c, b)?,
                    root.value(s, c, b)?,
                    environment.value(s, c, b)?,
                ],
                b,
            ),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "ProjectedFieldValue")? {
            ("Atom", [v]) => Self::Atom(Value::read(v, s, c, b)?),
            ("Child", [v]) => Self::Child(Value::read(v, s, c, b)?),
            ("Children", [v]) => Self::Children(Value::read(v, s, c, b)?),
            ("Foreign", [schema, category, root, environment]) => Self::Foreign {
                schema: Value::read(schema, s, c, b)?,
                category: Value::read(category, s, c, b)?,
                root: Value::read(root, s, c, b)?,
                environment: Value::read(environment, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for HeadRequest {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Shape => variant(s.engine, "HeadRequest", "Shape", [], b),
            Self::ChildContext {
                shape,
                index,
                completed,
            } => variant(
                s.engine,
                "HeadRequest",
                "ChildContext",
                [
                    shape.value(s, c, b)?,
                    index.value(s, c, b)?,
                    completed.value(s, c, b)?,
                ],
                b,
            ),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "HeadRequest")? {
            ("Shape", []) => Self::Shape,
            ("ChildContext", [shape, index, completed]) => Self::ChildContext {
                shape: Value::read(shape, s, c, b)?,
                index: Value::read(index, s, c, b)?,
                completed: Value::read(completed, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for HeadOutcome {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Shape { shape } => {
                variant(s.engine, "HeadOutcome", "Shape", [shape.value(s, c, b)?], b)
            }
            Self::ChildContext { context } => variant(
                s.engine,
                "HeadOutcome",
                "ChildContext",
                [context.value(s, c, b)?],
                b,
            ),
            Self::Failed { diagnostic } => variant(
                s.engine,
                "HeadOutcome",
                "Failed",
                [diagnostic.value(s, c, b)?],
                b,
            ),
            Self::Stopped { reason } => variant(
                s.engine,
                "HeadOutcome",
                "Stopped",
                [reason.value(s, c, b)?],
                b,
            ),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "HeadOutcome")? {
            ("Shape", [v]) => Self::Shape {
                shape: Value::read(v, s, c, b)?,
            },
            ("ChildContext", [v]) => Self::ChildContext {
                context: Value::read(v, s, c, b)?,
            },
            ("Failed", [v]) => Self::Failed {
                diagnostic: Value::read(v, s, c, b)?,
            },
            ("Stopped", [v]) => Self::Stopped {
                reason: Value::read(v, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
