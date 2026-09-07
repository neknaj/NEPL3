use super::*;
macro_rules! record_value {($ty:ident,$name:literal,$count:literal,[$($field:ident:$index:literal),*])=>{
impl Value for $ty {
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{record(s.engine,$name,[$(self.$field.value(s,c,b)?),*],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>>{let f=fields(v,s.engine,$name,$count)?;Ok(Self {$($field:Value::read(&f[$index],s,c,b)?),*})}
}};}
record_value!(ReferenceOptions,"ReferenceOptions",4,[include_definitions:0,include_imports:1,include_exports:2,include_ambiguous:3]);
record_value!(QuerySelection,"QuerySelection",4,[occurrence:0,span:1,resolution:2,open_input:3]);
record_value!(DefinitionLocation,"DefinitionLocation",3,[uri:0,range:1,selection:2]);
record_value!(DefinitionTarget,"DefinitionTarget",2,[entity:0,location:1]);
record_value!(ReferenceLocation,"ReferenceLocation",5,[occurrence:0,role:1,uri:2,span:3,ambiguous:4]);
record_value!(EntityReferences,"EntityReferences",2,[entity:0,locations:1]);
impl Value for ReferenceResolution {
    fn value<C: FoundationValueCodec>(
        &self,
        _: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        c.encode_reference_resolution(self, b).map_err(boundary)
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        _: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        c.decode_reference_resolution(v, b).map_err(boundary)
    }
}
impl Value for OccurrenceRole {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        _: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        variant(
            s.foundation,
            "OccurrenceRole",
            match self {
                Self::Definition => "Definition",
                Self::Reference => "Reference",
                Self::Import => "Import",
                Self::Export => "Export",
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
        let (tag, f) = parts(v, s.foundation, "OccurrenceRole")?;
        if !f.is_empty() {
            return Err(PortableError::Shape);
        }
        Ok(match tag {
            "Definition" => Self::Definition,
            "Reference" => Self::Reference,
            "Import" => Self::Import,
            "Export" => Self::Export,
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for QueryKind {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Definition => variant(s.engine, "QueryKind", "Definition", [], b),
            Self::References(v) => {
                variant(s.engine, "QueryKind", "References", [v.value(s, c, b)?], b)
            }
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "QueryKind")? {
            ("Definition", []) => Self::Definition,
            ("References", [v]) => Self::References(ReferenceOptions::read(v, s, c, b)?),
            _ => return Err(PortableError::Shape),
        })
    }
}
impl Value for QueryRequest {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        record(
            s.engine,
            "QueryRequest",
            [
                super::super::analysis::key_value(&self.key, s, c, b)?,
                self.source.value(s, c, b)?,
                self.offset.value(s, c, b)?,
                self.kind.value(s, c, b)?,
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
        let f = fields(v, s.engine, "QueryRequest", 4)?;
        Ok(Self {
            key: super::super::analysis::key_read(&f[0], s, c, b)?,
            source: SourceRef::read(&f[1], s, c, b)?,
            offset: u64::read(&f[2], s, c, b)?,
            kind: QueryKind::read(&f[3], s, c, b)?,
        })
    }
}
fn access_value<C: FoundationValueCodec>(
    e: &BindingAccessError,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match e {
        BindingAccessError::Stopped(r) => variant(
            s.engine,
            "BindingAccessError",
            "Stopped",
            [r.value(s, c, b)?],
            b,
        ),
        e => variant(
            s.engine,
            "BindingAccessError",
            match e {
                BindingAccessError::LimitsMismatch => "LimitsMismatch",
                BindingAccessError::StaleAnalysis => "StaleAnalysis",
                BindingAccessError::MissingSource => "MissingSource",
                BindingAccessError::Incomplete => "Incomplete",
                BindingAccessError::Stopped(_) => return Err(PortableError::Shape),
            },
            [],
            b,
        ),
    }
}
fn access_read<C: FoundationValueCodec>(
    v: &NdfValue,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<BindingAccessError, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "BindingAccessError")? {
        ("Stopped", [v]) => BindingAccessError::Stopped(StopReason::read(v, s, c, b)?),
        ("LimitsMismatch", []) => BindingAccessError::LimitsMismatch,
        ("StaleAnalysis", []) => BindingAccessError::StaleAnalysis,
        ("MissingSource", []) => BindingAccessError::MissingSource,
        ("Incomplete", []) => BindingAccessError::Incomplete,
        _ => return Err(PortableError::Shape),
    })
}
fn error_value<C: FoundationValueCodec>(
    e: &QueryError,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match e {
        QueryError::Facts => variant(s.engine, "QueryError", "Facts", [], b),
        QueryError::Access(e) => variant(
            s.engine,
            "QueryError",
            "Access",
            [access_value(e, s, c, b)?],
            b,
        ),
        QueryError::Source(e) => variant(
            s.engine,
            "QueryError",
            "Source",
            [super::super::binding::error::source_value(e, r, b)?],
            b,
        ),
    }
}
fn error_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<QueryError, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "QueryError")? {
        ("Facts", []) => QueryError::Facts,
        ("Access", [v]) => QueryError::Access(access_read(v, s, c, b)?),
        ("Source", [v]) => QueryError::Source(super::super::binding::error::source_read(v, r, b)?),
        _ => return Err(PortableError::Shape),
    })
}
pub(super) fn outcome_value<C: FoundationValueCodec>(
    o: &QueryOutcome,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match o {
        QueryOutcome::Definition { selection, targets } => variant(
            s.engine,
            "QueryOutcome",
            "Definition",
            [selection.value(s, c, b)?, targets.value(s, c, b)?],
            b,
        ),
        QueryOutcome::References { selection, groups } => variant(
            s.engine,
            "QueryOutcome",
            "References",
            [selection.value(s, c, b)?, groups.value(s, c, b)?],
            b,
        ),
        QueryOutcome::Invalid(e) => variant(
            s.engine,
            "QueryOutcome",
            "Invalid",
            [error_value(e, r, s, c, b)?],
            b,
        ),
        QueryOutcome::Stopped(r) => {
            variant(s.engine, "QueryOutcome", "Stopped", [r.value(s, c, b)?], b)
        }
    }
}
pub(super) fn outcome_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<QueryOutcome, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "QueryOutcome")? {
        ("Definition", [selection, targets]) => QueryOutcome::Definition {
            selection: Option::<QuerySelection>::read(selection, s, c, b)?,
            targets: Vec::<DefinitionTarget>::read(targets, s, c, b)?,
        },
        ("References", [selection, groups]) => QueryOutcome::References {
            selection: Option::<QuerySelection>::read(selection, s, c, b)?,
            groups: Vec::<EntityReferences>::read(groups, s, c, b)?,
        },
        ("Invalid", [v]) => QueryOutcome::Invalid(error_read(v, r, s, c, b)?),
        ("Stopped", [v]) => QueryOutcome::Stopped(StopReason::read(v, s, c, b)?),
        _ => return Err(PortableError::Shape),
    })
}
