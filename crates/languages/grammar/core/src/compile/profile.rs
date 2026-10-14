//! Owned compiler inputs. These catalogs describe contracts, not runtime authority.
pub mod portable;
use super::{NamedClass, NamedView, ReaderImport, package::PackageContext};
use alloc::{string::String, vec::Vec};
use nepl3_core::schema::TypeDescriptor;
use nepl3_engine::package::ExtensionRequirement;

/// Owned counterpart of the native compiler's borrowed context.
///
/// Construction does not validate the catalogs or register providers. The native
/// compiler retains its existing name, selected-contract, and output checks.
/// Resource limits and runtime implementations belong to the invoking host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileProfile {
    pub package: String,
    pub state_type: TypeDescriptor,
    pub reader_imports: Vec<ReaderImport>,
    pub extensions: Vec<ExtensionRequirement>,
    pub classes: Vec<NamedClass>,
    pub views: Vec<NamedView>,
}
impl CompileProfile {
    /// Borrow the exact catalogs without copying, reordering, or resolving them.
    pub fn as_context(&self) -> PackageContext<'_> {
        PackageContext {
            package: &self.package,
            state_type: &self.state_type,
            reader_imports: &self.reader_imports,
            extensions: &self.extensions,
            classes: &self.classes,
            views: &self.views,
        }
    }
}
