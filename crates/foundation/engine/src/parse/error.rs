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
