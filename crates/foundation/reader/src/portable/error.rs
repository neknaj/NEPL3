use super::*;
use crate::plan::PlanError;
use nepl3_core::{origin::OriginError, value_codec::FoundationCodecError};

impl<E: FoundationCodecError> PortableError<E> {
    /// Return only a nested typed resource cause, independently of any unrelated
    /// stopped Budget. Semantic preflight rejection is not a resource stop.
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            PortableError::Stopped(s)
            | PortableError::Schema(SchemaError::Stopped(s))
            | PortableError::Source(SourceError::Stopped(s))
            | PortableError::Plan(PlanError::Stopped(s))
            | PortableError::Plan(PlanError::Schema(SchemaError::Stopped(s))) => Some(*s),
            PortableError::Boundary(e) => e.stop_reason(),
            PortableError::Reader(e) => e.stop_reason(),
            PortableError::Context(e) => match e {
                ContextError::Stopped(s)
                | ContextError::Schema(SchemaError::Stopped(s))
                | ContextError::Origin(OriginError::Stopped(s))
                | ContextError::Origin(OriginError::Source(SourceError::Stopped(s)))
                | ContextError::Source(SourceError::Stopped(s)) => Some(*s),
                ContextError::Boundary(e) => e.stop_reason(),
                _ => None,
            },
            _ => None,
        }
    }
}
