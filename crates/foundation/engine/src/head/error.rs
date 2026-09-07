use nepl3_core::{budget::StopReason, schema::SchemaError, source::SourceError};
#[derive(Debug, Eq, PartialEq)]
pub enum HeadError {
    Stopped(StopReason),
    Source(SourceError),
    Schema(SchemaError),
    Identity,
    Signature,
    Projection,
    ReplyKind,
    Shape,
    Context,
    Report,
}
impl From<StopReason> for HeadError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<SourceError> for HeadError {
    fn from(v: SourceError) -> Self {
        match v {
            SourceError::Stopped(v) => Self::Stopped(v),
            v => Self::Source(v),
        }
    }
}
impl From<SchemaError> for HeadError {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(v) => Self::Stopped(v),
            v => Self::Schema(v),
        }
    }
}

impl From<nepl3_core::diagnostic::validation::ReportValidationError> for HeadError {
    fn from(value: nepl3_core::diagnostic::validation::ReportValidationError) -> Self {
        use nepl3_core::diagnostic::validation::ReportValidationError;
        match value {
            ReportValidationError::Stopped(v) => Self::Stopped(v),
            ReportValidationError::Source(v) => v.into(),
            ReportValidationError::Schema(v) => v.into(),
            ReportValidationError::Metadata | ReportValidationError::Usage => Self::Report,
        }
    }
}
