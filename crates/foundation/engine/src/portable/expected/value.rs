use super::*;
macro_rules! record_value {($ty:ident,$count:literal,[$($field:ident:$index:literal),*])=>{
impl Value for $ty {
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{record(s.engine,stringify!($ty),[$(self.$field.value(s,c,b)?),*],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>>{let f=fields(v,s.engine,stringify!($ty),$count)?;Ok(Self {$($field:Value::read(&f[$index],s,c,b)?),*})}
}};}
record_value!(ExpectedReadRequest,3,[key:0,source:1,offset:2]);
record_value!(ExpectedRead,5,[bundle:0,node:1,path:2,expected:3,origin:4]);
impl Value for AnalysisKey {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        super::super::analysis::key_value(self, s, c, b)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        super::super::analysis::key_read(v, s, c, b)
    }
}
impl Value for ExpectedReadStep {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        let (case, field) = match self {
            Self::Child { field } => ("Child", field),
            Self::Foreign { field } => ("Foreign", field),
        };
        variant(
            s.engine,
            "ExpectedReadStep",
            case,
            [field.value(s, c, b)?],
            b,
        )
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "ExpectedReadStep")? {
            ("Child", [v]) => Self::Child {
                field: Value::read(v, s, c, b)?,
            },
            ("Foreign", [v]) => Self::Foreign {
                field: Value::read(v, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for ExpectedReadOrigin {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Root => variant(s.engine, "ExpectedReadOrigin", "Root", [], b),
            Self::Field {
                parent_bundle,
                parent_node,
                field,
                owner,
                declared,
                resolved_read,
                foreign,
            } => variant(
                s.engine,
                "ExpectedReadOrigin",
                "Field",
                [
                    parent_bundle.value(s, c, b)?,
                    parent_node.value(s, c, b)?,
                    field.value(s, c, b)?,
                    owner.value(s, c, b)?,
                    declared.value(s, c, b)?,
                    resolved_read.value(s, c, b)?,
                    foreign.value(s, c, b)?,
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
        Ok(match parts(v, s.engine, "ExpectedReadOrigin")? {
            ("Root", []) => Self::Root,
            ("Field", [a, d, f, o, r, x, g]) => Self::Field {
                parent_bundle: Value::read(a, s, c, b)?,
                parent_node: Value::read(d, s, c, b)?,
                field: Value::read(f, s, c, b)?,
                owner: Value::read(o, s, c, b)?,
                declared: Value::read(r, s, c, b)?,
                resolved_read: Value::read(x, s, c, b)?,
                foreign: Value::read(g, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        })
    }
}
pub(in crate::portable) fn error_value<C: FoundationValueCodec>(
    error: &ExpectedReadError,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match error {
        ExpectedReadError::Owner => variant(s.engine, "ExpectedReadError", "Owner", [], b),
        ExpectedReadError::Access(e) => variant(
            s.engine,
            "ExpectedReadError",
            "Access",
            [super::super::query::value::access_value(e, s, c, b)?],
            b,
        ),
        ExpectedReadError::Source(e) => variant(
            s.engine,
            "ExpectedReadError",
            "Source",
            [super::super::binding::error::source_value(e, r, b)?],
            b,
        ),
    }
}
pub(in crate::portable) fn error_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<ExpectedReadError, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "ExpectedReadError")? {
        ("Owner", []) => ExpectedReadError::Owner,
        ("Access", [e]) => {
            ExpectedReadError::Access(super::super::query::value::access_read(e, s, c, b)?)
        }
        ("Source", [e]) => {
            ExpectedReadError::Source(super::super::binding::error::source_read(e, r, b)?)
        }
        _ => return Err(PortableError::Shape),
    })
}
pub(super) fn outcome_value<C: FoundationValueCodec>(
    value: &ExpectedReadOutcome,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match value {
        ExpectedReadOutcome::Complete(value) => variant(
            s.engine,
            "ExpectedReadOutcome",
            "Complete",
            [value.value(s, c, b)?],
            b,
        ),
        ExpectedReadOutcome::Invalid(error) => variant(
            s.engine,
            "ExpectedReadOutcome",
            "Invalid",
            [error_value(error, r, s, c, b)?],
            b,
        ),
        ExpectedReadOutcome::Stopped(reason) => variant(
            s.engine,
            "ExpectedReadOutcome",
            "Stopped",
            [reason.value(s, c, b)?],
            b,
        ),
    }
}
pub(super) fn outcome_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<ExpectedReadOutcome, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "ExpectedReadOutcome")? {
        ("Complete", [v]) => ExpectedReadOutcome::Complete(Value::read(v, s, c, b)?),
        ("Invalid", [v]) => ExpectedReadOutcome::Invalid(error_read(v, r, s, c, b)?),
        ("Stopped", [v]) => ExpectedReadOutcome::Stopped(Value::read(v, s, c, b)?),
        _ => return Err(PortableError::Shape),
    })
}
