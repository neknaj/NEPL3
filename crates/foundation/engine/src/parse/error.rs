use crate::{package::PackageError, profile::ProfileError};
use nepl3_core::{
    budget::StopReason, schema::SchemaError, source::SourceError, syntax::SyntaxError,
};
use nepl3_reader::runtime::ReaderError;
#[derive(Debug, Eq, PartialEq)]
pub enum ParseError {
    Stopped(StopReason),
    Package(PackageError),
    Profile(ProfileError),
    Schema(SchemaError),
    Source(SourceError),
    Syntax(SyntaxError),
    Reader(ReaderError),
    Tree(crate::tree::TreeError),
    Head(crate::head::HeadError),
    LimitsMismatch,
    Context,
    State,
    Reference,
    Busy,
    Closed,
    NoPending,
    Continuation,
}
macro_rules! from {
    ($ty:ty,$variant:ident) => {
        impl From<$ty> for ParseError {
            fn from(v: $ty) -> Self {
                Self::$variant(v)
            }
        }
    };
}
from!(StopReason, Stopped);
from!(PackageError, Package);
from!(ProfileError, Profile);
from!(SchemaError, Schema);
from!(SourceError, Source);
from!(SyntaxError, Syntax);
from!(ReaderError, Reader);

from!(crate::tree::TreeError, Tree);
impl From<crate::head::HeadError> for ParseError {
    fn from(value: crate::head::HeadError) -> Self {
        match value {
            crate::head::HeadError::Stopped(reason) => Self::Stopped(reason),
            value => Self::Head(value),
        }
    }
}

impl ParseError {
    /// The original stop survives public error wrappers, including values
    /// constructed directly by a host rather than through From conversions.
    pub fn stop_reason(&self) -> Option<StopReason> {
        use crate::{head::HeadError, tree::TreeError};
        match self {
            Self::Stopped(r)
            | Self::Schema(SchemaError::Stopped(r))
            | Self::Source(SourceError::Stopped(r)) => Some(*r),
            Self::Reader(e) => e.stop_reason(),
            Self::Syntax(e) => e.stop_reason(),
            Self::Package(e) => package_stop(e),
            Self::Profile(e) => profile_stop(e),
            Self::Tree(e) => match e {
                TreeError::Stopped(r) => Some(*r),
                TreeError::Syntax(e) => e.stop_reason(),
                TreeError::Package(e) => package_stop(e),
                TreeError::Profile(e) => profile_stop(e),
                _ => None,
            },
            Self::Head(
                HeadError::Stopped(r)
                | HeadError::Source(SourceError::Stopped(r))
                | HeadError::Schema(SchemaError::Stopped(r)),
            ) => Some(*r),
            _ => None,
        }
    }
}
fn package_stop(e: &PackageError) -> Option<StopReason> {
    use nepl3_core::origin::OriginError;
    use nepl3_reader::plan::PlanError;
    match e {
        PackageError::Stopped(r)
        | PackageError::Schema(SchemaError::Stopped(r))
        | PackageError::Source(SourceError::Stopped(r))
        | PackageError::Origin(
            OriginError::Stopped(r) | OriginError::Source(SourceError::Stopped(r)),
        )
        | PackageError::Reader(
            PlanError::Stopped(r) | PlanError::Schema(SchemaError::Stopped(r)),
        ) => Some(*r),
        _ => None,
    }
}
fn profile_stop(e: &ProfileError) -> Option<StopReason> {
    match e {
        ProfileError::Stopped(r) | ProfileError::Schema(SchemaError::Stopped(r)) => Some(*r),
        ProfileError::Package(e) => package_stop(e),
        _ => None,
    }
}
