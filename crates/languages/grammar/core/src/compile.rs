//! Production Grammar lowering. No provider is executed by compilation.
use crate::model::{CheckedDocument, ModelError, NatLiteral, NodeId};
use alloc::boxed::Box;
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    value::{KindRef, SchemaRef},
    view::PresentationClass,
};
use nepl3_reader::plan::{PlanError, ProviderSignature};
pub mod diagnostic;
pub mod package;
pub mod reader;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReaderImport {
    pub provider: String,
    pub signature: ProviderSignature,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedClass {
    pub name: String,
    pub class: PresentationClass,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedView {
    pub name: String,
    pub kind: KindRef,
}
pub struct ReaderContext<'a> {
    pub schema: &'a SchemaRef,
    pub state_type: &'a TypeDescriptor,
    pub imports: &'a [ReaderImport],
    pub classes: &'a [NamedClass],
    pub views: &'a [NamedView],
    pub registry: &'a SchemaRegistry,
}
#[derive(Debug, Eq, PartialEq)]
pub enum CompileError {
    Located(Box<diagnostic::LocatedCompileError>),
    Stopped(StopReason),
    Model(ModelError),
    Schema(SchemaError),
    Reader(PlanError),
    WrongConstructor(NodeId),
    NaturalOverflow,
    InvalidRange,
    InvalidRepeat,
    MissingProvider,
    ProviderKind,
    MissingClass,
    MissingView,
    MissingRule,
    DuplicateRule,
    OutputType,
    InvalidCatalogName(Catalog),
    DuplicateCatalogName(Catalog),
    Package(nepl3_engine::package::PackageError),
    Declaration {
        node: NodeId,
        related: Option<NodeId>,
        reason: DeclarationError,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationError {
    KindShape,
    EmptyName,
    DuplicateName,
    MissingExtension,
    Signature,
    MissingReader,
    MissingToken,
    MissingClass,
    MissingView,
    InvalidBuiltin,
    InvalidRead,
    InvalidBinding,
    InvalidSelector,
}
impl From<nepl3_engine::package::PackageError> for CompileError {
    fn from(v: nepl3_engine::package::PackageError) -> Self {
        match v {
            nepl3_engine::package::PackageError::Stopped(r) => Self::Stopped(r),
            v => Self::Package(v),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Catalog {
    Imports,
    Classes,
    Views,
}
impl ReaderContext<'_> {
    fn validate(&self, budget: &mut Budget) -> Result<(), CompileError> {
        fn names<'a>(
            values: impl Iterator<Item = &'a str> + Clone,
            catalog: Catalog,
            budget: &mut Budget,
        ) -> Result<(), CompileError> {
            for (i, name) in values.clone().enumerate() {
                budget.charge(Resource::Work, name.len() as u64 + 1)?;
                if name.is_empty() {
                    return Err(CompileError::InvalidCatalogName(catalog));
                }
                for previous in values.clone().take(i) {
                    budget.charge(Resource::Work, name.len().min(previous.len()) as u64 + 1)?;
                    if name == previous {
                        return Err(CompileError::DuplicateCatalogName(catalog));
                    }
                }
            }
            Ok(())
        }
        names(
            self.imports.iter().map(|v| v.provider.as_str()),
            Catalog::Imports,
            budget,
        )?;
        names(
            self.classes.iter().map(|v| v.name.as_str()),
            Catalog::Classes,
            budget,
        )?;
        names(
            self.views.iter().map(|v| v.name.as_str()),
            Catalog::Views,
            budget,
        )
    }
}
impl From<StopReason> for CompileError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<ModelError> for CompileError {
    fn from(v: ModelError) -> Self {
        match v {
            ModelError::Stopped(s) => Self::Stopped(s),
            v => Self::Model(v),
        }
    }
}
impl From<SchemaError> for CompileError {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(s) => Self::Stopped(s),
            v => Self::Schema(v),
        }
    }
}
impl From<PlanError> for CompileError {
    fn from(v: PlanError) -> Self {
        match v {
            PlanError::Stopped(s) => Self::Stopped(s),
            v => Self::Reader(v),
        }
    }
}
/// Grammar uses arbitrary precision Nat literals. Only fields whose operation
/// contract is U64 perform this checked conversion; no truncation or saturating cast.
pub fn natural_u64(value: &NatLiteral, budget: &mut Budget) -> Result<u64, CompileError> {
    budget.charge(Resource::Work, 1)?;
    if value.value.is_negative() || value.value.as_bigint().bits() > 64 {
        return Err(CompileError::NaturalOverflow.at(&value.span, None, budget));
    }
    budget.charge(Resource::AllocationUnits, 8)?;
    let (_, bytes) = value.value.canonical_parts();
    budget.charge(Resource::Work, bytes.len() as u64)?;
    Ok(bytes.into_iter().fold(0u64, |n, b| (n << 8) | u64::from(b)))
}
fn text(value: &str, budget: &mut Budget) -> Result<String, CompileError> {
    budget.charge(Resource::Work, value.len() as u64 + 1)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(value.into())
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), CompileError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    values.push(value);
    Ok(())
}
