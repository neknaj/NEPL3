//! Persistent parser choices, including dynamic heads, owned by the engine result.
use crate::{
    package::{BindingId, EntryContext, FieldSpec, ReadSpecId, SelectionRule, StyleRule},
    recovery::ForeignStep,
};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    source::Digest,
    syntax::NodeRef,
    value::{KindRef, OperationRef},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadProviderRef {
    pub shape: OperationRef,
    pub child_context: OperationRef,
}
/// All reads/bindings refer to the owning selected package's concrete arenas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeadShape {
    pub kind: KindRef,
    pub fields: Vec<FieldSpec>,
    pub binding: BindingId,
    pub styles: Vec<StyleRule>,
    pub selection_rules: Vec<SelectionRule>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeSelection {
    Form {
        index: u64,
    },
    Leaf {
        index: u64,
    },
    Builtin {
        read: ReadSpecId,
    },
    List {
        read: ReadSpecId,
        cons: bool,
    },
    Dynamic {
        provider: HeadProviderRef,
        shape: Box<HeadShape>,
        /// Final, ordered context_for_child results. Partial frames keep their
        /// own shorter prefix; a completed selection has one entry per field.
        child_contexts: Vec<EntryContext>,
    },
    Recovery,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeSelection {
    pub node: NodeRef,
    pub entry: EntryContext,
    pub execution_digest: Digest,
    pub shape: ShapeSelection,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleContext {
    pub path: Vec<ForeignStep>,
    pub nodes: Vec<NodeSelection>,
}
