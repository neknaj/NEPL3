use alloc::{string::String, vec::Vec};
use nepl3_core::{
    origin::{Mapping, Origin, OriginId},
    schema::TypeDescriptor,
    source::{Digest, SourceSnapshot},
    value::{KindRef, OperationRef, SchemaRef},
    view::PresentationClass,
};
use nepl3_reader::{builtin::BuiltinReader, plan::ReaderPlan, tokenizer::ReaderMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadSpecId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingId(pub u64);

/// Direct arena edges are acyclic. Recursive categories are named Local references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadSpec {
    Builtin {
        reader: BuiltinReader,
        kind: KindRef,
        token_kind: KindRef,
    },
    Local {
        category: String,
    },
    Foreign {
        alias: String,
        category: String,
    },
    WithMode {
        mode: String,
        read: ReadSpecId,
    },
    ListOf {
        element: ReadSpecId,
        cons: KindRef,
        nil: KindRef,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldSpec {
    pub name: String,
    pub read: ReadSpecId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Category {
    pub name: String,
    pub mode: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Form {
    pub category: String,
    pub kind: KindRef,
    pub spelling: String,
    pub fields: Vec<FieldSpec>,
    pub binding: BindingId,
    pub styles: Vec<StyleRule>,
    pub selection_rules: Vec<SelectionRule>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Leaf {
    pub category: String,
    pub kind: KindRef,
    pub token_kind: KindRef,
    pub payload: TypeDescriptor,
    pub binding: BindingId,
    pub styles: Vec<StyleRule>,
    pub selection_rules: Vec<SelectionRule>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NamespacePolicy {
    Lexical,
    Global,
    Open,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Namespace {
    pub name: String,
    pub policy: NamespacePolicy,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NameSelector {
    SelfValue,
    Field(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Binding {
    None,
    Visit(String),
    Group(Vec<BindingId>),
    Scope(Vec<BindingId>),
    Bind {
        namespace: String,
        name: NameSelector,
    },
    Reference {
        namespace: String,
        name: NameSelector,
    },
    Export {
        namespace: String,
        name: NameSelector,
    },
    Import(String),
    Propagate(String),
    Sequential {
        declarations: String,
        body: String,
    },
    Recursive {
        declarations: String,
        body: String,
    },
    Custom(OperationRef),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StyleSelector {
    Head,
    SelfValue,
    Field(String),
    Capture(String),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleRule {
    pub selector: StyleSelector,
    pub class: PresentationClass,
}

/// Higher priority wins only after the smallest containing source range.
/// An owner with no matching declaration has the defined priority zero.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionRule {
    pub selector: StyleSelector,
    pub priority: u64,
}

/// A versioned contract known to the compiler; this does not contain an executable callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRequirement {
    pub alias: String,
    pub provider: String,
    pub signature: String,
    pub operation: OperationRef,
    pub input: TypeDescriptor,
    pub output: TypeDescriptor,
    pub pure: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationKind {
    Category,
    Mode,
    Reader,
    Form,
    Leaf,
    Namespace,
    Extension,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationOrigin {
    pub kind: DeclarationKind,
    pub name: String,
    pub category: Option<String>,
    pub origin: OriginId,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageProvenance {
    pub sources: Vec<SourceSnapshot>,
    pub origins: Vec<Origin>,
    pub source_maps: Vec<Mapping>,
    pub declarations: Vec<DeclarationOrigin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguagePackage {
    pub schema: SchemaRef,
    pub payload_schemas: Vec<SchemaRef>,
    pub root: String,
    pub reader: ReaderPlan,
    pub modes: Vec<ReaderMode>,
    pub categories: Vec<Category>,
    pub reads: Vec<ReadSpec>,
    pub forms: Vec<Form>,
    pub leaves: Vec<Leaf>,
    pub namespaces: Vec<Namespace>,
    pub bindings: Vec<Binding>,
    pub extensions: Vec<ExtensionRequirement>,
    pub recovery: crate::recovery::RecoveryPlan,
    pub provenance: PackageProvenance,
}
/// Package behavior identity is separate from the surface type descriptor digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageIdentity {
    pub schema: SchemaRef,
    pub semantic_digest: Digest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryContext {
    pub package: PackageIdentity,
    pub alias: String,
    pub category: String,
    pub mode: String,
}
