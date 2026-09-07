use super::*;
use nepl3_core::schema::SchemaRegistry;
macro_rules! record_value {($ty:ident,$count:literal,[$($field:ident:$index:literal),*])=>{
impl Value for $ty {
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{record(s.engine,stringify!($ty),[$(self.$field.value(s,c,b)?),*],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>>{let f=fields(v,s.engine,stringify!($ty),$count)?;Ok(Self {$($field:Value::read(&f[$index],s,c,b)?),*})}
}};}
record_value!(RegionRequest,3,[key:0,source:1,offset:2]);
record_value!(ViewStep,2,[field:0,child:1]);
record_value!(RegionTarget,3,[bundle:0,node:1,part:2]);
record_value!(SourceRegion,8,[target:0,span:1,logical_span:2,mapping:3,priority:4,depth:5,declaration_order:6,classes:7]);
macro_rules! enumeration {($ty:ident,$($case:ident),*)=>{
impl Value for $ty {
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,_:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{variant(s.engine,stringify!($ty),match self{$(Self::$case=>stringify!($case)),*},[],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,_:&mut C,_:&mut Budget)->Result<Self,PortableError<C::Error>>{let (case,args)=parts(v,s.engine,stringify!($ty))?;if !args.is_empty(){return Err(PortableError::Shape);}Ok(match case{$(stringify!($case)=>Self::$case,)*_=>return Err(PortableError::Shape)})}
}};}
enumeration!(RegionCapability, SyntaxOnly, ReaderFacts);
enumeration!(RegionMapping, Direct, Exact, ExactFragment, Transformed);
impl Value for RegionKey {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "RegionKey",
            [
                super::super::analysis::key_value(&self.analysis, s, c, b)?,
                self.reader_facts_digest.value(s, c, b)?,
            ],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let f = fields(v, s.engine, "RegionKey", 2)?;
        Ok(Self {
            analysis: super::super::analysis::key_read(&f[0], s, c, b)?,
            reader_facts_digest: Value::read(&f[1], s, c, b)?,
        })
    }
}
impl Value for RegionPart {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        Ok(match self {
            Self::Node => variant(s.engine, "RegionPart", "Node", [], b)?,
            Self::Head => variant(s.engine, "RegionPart", "Head", [], b)?,
            Self::Recovery => variant(s.engine, "RegionPart", "Recovery", [], b)?,
            Self::Field { field, element } => variant(
                s.engine,
                "RegionPart",
                "Field",
                [field.value(s, c, b)?, element.value(s, c, b)?],
                b,
            )?,
            Self::View { root, path } => variant(
                s.engine,
                "RegionPart",
                "View",
                [root.value(s, c, b)?, path.value(s, c, b)?],
                b,
            )?,
            Self::Capture { batch, fact } => variant(
                s.engine,
                "RegionPart",
                "Capture",
                [batch.value(s, c, b)?, fact.value(s, c, b)?],
                b,
            )?,
            Self::Presentation { batch, fact } => variant(
                s.engine,
                "RegionPart",
                "Presentation",
                [batch.value(s, c, b)?, fact.value(s, c, b)?],
                b,
            )?,
        })
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        let (case, f) = parts(v, s.engine, "RegionPart")?;
        Ok(match (case, f) {
            ("Node", []) => Self::Node,
            ("Head", []) => Self::Head,
            ("Recovery", []) => Self::Recovery,
            ("Field", [a, d]) => Self::Field {
                field: Value::read(a, s, c, b)?,
                element: Value::read(d, s, c, b)?,
            },
            ("View", [a, d]) => Self::View {
                root: Value::read(a, s, c, b)?,
                path: Value::read(d, s, c, b)?,
            },
            ("Capture", [a, d]) => Self::Capture {
                batch: Value::read(a, s, c, b)?,
                fact: Value::read(d, s, c, b)?,
            },
            ("Presentation", [a, d]) => Self::Presentation {
                batch: Value::read(a, s, c, b)?,
                fact: Value::read(d, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
pub(super) fn error_value<C: FoundationValueCodec>(
    error: &RegionError,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match error {
        RegionError::Access(e) => variant(
            s.engine,
            "RegionError",
            "Access",
            [super::super::query::value::access_value(e, s, c, b)?],
            b,
        ),
        RegionError::Source(e) => variant(
            s.engine,
            "RegionError",
            "Source",
            [super::super::binding::error::source_value(e, r, b)?],
            b,
        ),
        RegionError::Owner => variant(s.engine, "RegionError", "Owner", [], b),
        RegionError::Sidecar => variant(s.engine, "RegionError", "Sidecar", [], b),
        RegionError::Mapping => variant(s.engine, "RegionError", "Mapping", [], b),
    }
}
pub(super) fn error_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionError, PortableError<C::Error>> {
    let (case, f) = parts(v, s.engine, "RegionError")?;
    Ok(match (case, f) {
        ("Access", [e]) => {
            RegionError::Access(super::super::query::value::access_read(e, s, c, b)?)
        }
        ("Source", [e]) => RegionError::Source(super::super::binding::error::source_read(e, r, b)?),
        ("Owner", []) => RegionError::Owner,
        ("Sidecar", []) => RegionError::Sidecar,
        ("Mapping", []) => RegionError::Mapping,
        _ => return Err(PortableError::Shape),
    })
}
pub(super) fn outcome_value<C: FoundationValueCodec>(
    v: &RegionOutcome,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match v {
        RegionOutcome::Complete { selection, regions } => variant(
            s.engine,
            "RegionOutcome",
            "Complete",
            [selection.value(s, c, b)?, regions.value(s, c, b)?],
            b,
        ),
        RegionOutcome::Invalid(e) => variant(
            s.engine,
            "RegionOutcome",
            "Invalid",
            [error_value(e, r, s, c, b)?],
            b,
        ),
        RegionOutcome::Stopped(e) => {
            variant(s.engine, "RegionOutcome", "Stopped", [e.value(s, c, b)?], b)
        }
    }
}
pub(super) fn outcome_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<RegionOutcome, PortableError<C::Error>> {
    let (case, f) = parts(v, s.engine, "RegionOutcome")?;
    Ok(match (case, f) {
        ("Complete", [a, d]) => RegionOutcome::Complete {
            selection: Value::read(a, s, c, b)?,
            regions: Value::read(d, s, c, b)?,
        },
        ("Invalid", [e]) => RegionOutcome::Invalid(error_read(e, r, s, c, b)?),
        ("Stopped", [e]) => RegionOutcome::Stopped(Value::read(e, s, c, b)?),
        _ => return Err(PortableError::Shape),
    })
}
