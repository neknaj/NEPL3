use super::*;
use alloc::vec::Vec;
impl Value for RenameRequest {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::Work, self.new_name.len() as u64)?;
        record(
            s.engine,
            "RenameRequest",
            [
                super::super::analysis::key_value(&self.key, s, c, b)?,
                self.source.value(s, c, b)?,
                self.offset.value(s, c, b)?,
                self.new_name.value(s, c, b)?,
                self.writable.value(s, c, b)?,
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
        let f = fields(v, s.engine, "RenameRequest", 5)?;
        if let NdfValue::Text(value) = &f[3] {
            b.charge(Resource::Work, value.len() as u64)?;
        }
        Ok(Self {
            key: super::super::analysis::key_read(&f[0], s, c, b)?,
            source: SourceRef::read(&f[1], s, c, b)?,
            offset: u64::read(&f[2], s, c, b)?,
            new_name: alloc::string::String::read(&f[3], s, c, b)?,
            writable: Vec::read(&f[4], s, c, b)?,
        })
    }
}
impl Value for TextEdit {
    fn value<C: FoundationValueCodec>(
        &self,
        s: &Schemas<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        b.charge(Resource::Work, self.replacement.len() as u64)?;
        record(
            s.foundation,
            "TextEdit",
            [
                self.span.value(s, c, b)?,
                self.expected_digest.value(s, c, b)?,
                self.replacement.value(s, c, b)?,
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
        let f = fields(v, s.foundation, "TextEdit", 3)?;
        if let NdfValue::Text(value) = &f[2] {
            b.charge(Resource::Work, value.len() as u64)?;
        }
        Ok(Self {
            span: Value::read(&f[0], s, c, b)?,
            expected_digest: Digest::read(&f[1], s, c, b)?,
            replacement: Value::read(&f[2], s, c, b)?,
        })
    }
}
fn error_value<C: FoundationValueCodec>(
    e: &RenameError,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match e {
        RenameError::Stopped(v) => {
            variant(s.engine, "RenameError", "Stopped", [v.value(s, c, b)?], b)
        }
        RenameError::Access(v) => variant(
            s.engine,
            "RenameError",
            "Access",
            [super::super::query::value::access_value(v, s, c, b)?],
            b,
        ),
        RenameError::Source(v) => variant(
            s.engine,
            "RenameError",
            "Source",
            [super::super::binding::error::source_value(v, r, b)?],
            b,
        ),
        RenameError::Origin(v) => variant(
            s.engine,
            "RenameError",
            "Origin",
            [super::super::binding::error::origin_value(v, r, b)?],
            b,
        ),
        e => variant(
            s.engine,
            "RenameError",
            match e {
                RenameError::NoOccurrence => "NoOccurrence",
                RenameError::Unresolved => "Unresolved",
                RenameError::Ambiguous => "Ambiguous",
                RenameError::Deferred => "Deferred",
                RenameError::NoLocation => "NoLocation",
                RenameError::InvalidName => "InvalidName",
                RenameError::NotWritable => "NotWritable",
                RenameError::RenameNotInvertible => "RenameNotInvertible",
                RenameError::RequestMismatch => "RequestMismatch",
                RenameError::ShapeChanged => "ShapeChanged",
                RenameError::ResolutionChanged => "ResolutionChanged",
                RenameError::Collision => "Collision",
                _ => return Err(PortableError::Shape),
            },
            [],
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
) -> Result<RenameError, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "RenameError")? {
        ("Stopped", [v]) => RenameError::Stopped(StopReason::read(v, s, c, b)?),
        ("Access", [v]) => {
            RenameError::Access(super::super::query::value::access_read(v, s, c, b)?)
        }
        ("Source", [v]) => RenameError::Source(super::super::binding::error::source_read(v, r, b)?),
        ("Origin", [v]) => RenameError::Origin(super::super::binding::error::origin_read(v, r, b)?),
        ("NoOccurrence", []) => RenameError::NoOccurrence,
        ("Unresolved", []) => RenameError::Unresolved,
        ("Ambiguous", []) => RenameError::Ambiguous,
        ("Deferred", []) => RenameError::Deferred,
        ("NoLocation", []) => RenameError::NoLocation,
        ("InvalidName", []) => RenameError::InvalidName,
        ("NotWritable", []) => RenameError::NotWritable,
        ("RenameNotInvertible", []) => RenameError::RenameNotInvertible,
        ("RequestMismatch", []) => RenameError::RequestMismatch,
        ("ShapeChanged", []) => RenameError::ShapeChanged,
        ("ResolutionChanged", []) => RenameError::ResolutionChanged,
        ("Collision", []) => RenameError::Collision,
        _ => return Err(PortableError::Shape),
    })
}
pub(super) fn outcome_value<C: FoundationValueCodec>(
    o: &RenameOutcome,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    match o {
        RenameOutcome::Complete { new_key, edits } => variant(
            s.engine,
            "RenameOutcome",
            "Complete",
            [
                super::super::analysis::key_value(new_key, s, c, b)?,
                edits.value(s, c, b)?,
            ],
            b,
        ),
        RenameOutcome::Invalid(e) => variant(
            s.engine,
            "RenameOutcome",
            "Invalid",
            [error_value(e, r, s, c, b)?],
            b,
        ),
        RenameOutcome::Stopped(v) => {
            variant(s.engine, "RenameOutcome", "Stopped", [v.value(s, c, b)?], b)
        }
    }
}
pub(super) fn outcome_read<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenameOutcome, PortableError<C::Error>> {
    Ok(match parts(v, s.engine, "RenameOutcome")? {
        ("Complete", [key, edits]) => RenameOutcome::Complete {
            new_key: super::super::analysis::key_read(key, s, c, b)?,
            edits: Vec::read(edits, s, c, b)?,
        },
        ("Invalid", [e]) => RenameOutcome::Invalid(error_read(e, r, s, c, b)?),
        ("Stopped", [r]) => RenameOutcome::Stopped(StopReason::read(r, s, c, b)?),
        _ => return Err(PortableError::Shape),
    })
}
