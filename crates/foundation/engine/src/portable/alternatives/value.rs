use super::*;
macro_rules! record_value {($ty:ident,$count:literal,[$($field:ident:$index:literal),*])=>{
impl Value for $ty {
fn value<C:FoundationValueCodec>(&self,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>>{record(s.engine,stringify!($ty),[$(self.$field.value(s,c,b)?),*],b)}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&Schemas<'_>,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>>{let f=fields(v,s.engine,stringify!($ty),$count)?;Ok(Self {$($field:Value::read(&f[$index],s,c,b)?),*})}
}};}
record_value!(DeclaredFormAlternative,2,[index:0,spelling:1]);
record_value!(DeclaredAlternativeSet,2,[read:0,alternatives:1]);
impl Value for DeclaredReadAlternatives {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Category {
                forms,
                leaf_declarations,
                dynamic_fallback_registered,
            } => variant(
                s.engine,
                "DeclaredReadAlternatives",
                "Category",
                [
                    forms.value(s, c, b)?,
                    leaf_declarations.value(s, c, b)?,
                    dynamic_fallback_registered.value(s, c, b)?,
                ],
                b,
            ),
            Self::Builtin { reader } => variant(
                s.engine,
                "DeclaredReadAlternatives",
                "Builtin",
                [reader.value(s, c, b)?],
                b,
            ),
            Self::List => variant(s.engine, "DeclaredReadAlternatives", "List", [], b),
        }
    }
    fn read<C: FoundationValueCodec>(
        v: &NdfValue,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<Self, PortableError<C::Error>> {
        Ok(match parts(v, s.engine, "DeclaredReadAlternatives")? {
            ("Category", [f, l, d]) => Self::Category {
                forms: Value::read(f, s, c, b)?,
                leaf_declarations: Value::read(l, s, c, b)?,
                dynamic_fallback_registered: Value::read(d, s, c, b)?,
            },
            ("Builtin", [r]) => Self::Builtin {
                reader: Value::read(r, s, c, b)?,
            },
            ("List", []) => Self::List,
            _ => return Err(PortableError::Shape),
        })
    }
}
pub(super) fn outcome_value<C: FoundationValueCodec>(
    value: &DeclaredAlternativesOutcome,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match value {
        DeclaredAlternativesOutcome::Complete(value) => variant(
            s.engine,
            "DeclaredAlternativesOutcome",
            "Complete",
            [value.value(s, c, b)?],
            b,
        ),
        DeclaredAlternativesOutcome::Invalid(error) => variant(
            s.engine,
            "DeclaredAlternativesOutcome",
            "Invalid",
            [super::super::expected::value::error_value(
                error, r, s, c, b,
            )?],
            b,
        ),
        DeclaredAlternativesOutcome::Stopped(reason) => variant(
            s.engine,
            "DeclaredAlternativesOutcome",
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
) -> Result<DeclaredAlternativesOutcome, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "DeclaredAlternativesOutcome")? {
        ("Complete", [v]) => DeclaredAlternativesOutcome::Complete(Value::read(v, s, c, b)?),
        ("Invalid", [v]) => DeclaredAlternativesOutcome::Invalid(
            super::super::expected::value::error_read(v, r, s, c, b)?,
        ),
        ("Stopped", [v]) => DeclaredAlternativesOutcome::Stopped(Value::read(v, s, c, b)?),
        _ => return Err(PortableError::Shape),
    })
}
