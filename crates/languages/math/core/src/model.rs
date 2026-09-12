//! Source expressions retain notation and never contain an implicit evaluation.
//! Flat references keep cleanup of deeply nested input independent of stack size.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    origin::{Mapping, Origin, OriginId},
    source::{SourceSnapshot, Span},
    syntax::ForeignClosure,
    value::Rational,
    view::ViewBundle,
};

macro_rules! references {
    ($($name:ident),+ $(,)?) => {$ (
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name(pub u64);
    )+};
}
references!(ExprRef, RowRef, DocGuestRef, EmbedRef);

/// Preorder occurrence numbers distinguish shared nodes visited in different scopes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathBinding {
    pub occurrence: u64,
    pub node: ExprRef,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathSymbolUse {
    pub occurrence: u64,
    pub node: ExprRef,
    /// Binder occurrence, not an arena node ID. None denotes a free symbol.
    pub binding: Option<u64>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathBindings {
    pub definitions: Vec<MathBinding>,
    pub uses: Vec<MathSymbolUse>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathRoot {
    Expr(ExprRef),
    Row(RowRef),
    DocGuest(DocGuestRef),
}

/// A mathematical symbol's selection, distinct from the enclosing expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathField {
    SymbolName,
    LetName,
    SumIndex,
    IntegralIndex,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathFieldLocation {
    pub field: MathField,
    pub origin: Option<OriginId>,
    pub span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathKind {
    /// Only finite decimal rationals are valid Number constructors. Spelling
    /// points to retained source; it is not automatically trusted for printing.
    Number {
        value: Rational,
        spelling: Option<Span>,
    },
    /// Bare names and explicit `symbol` share the same mathematical meaning.
    Symbol {
        name: String,
    },
    Add {
        left: ExprRef,
        right: ExprRef,
    },
    Sub {
        left: ExprRef,
        right: ExprRef,
    },
    Mul {
        left: ExprRef,
        right: ExprRef,
    },
    Frac {
        left: ExprRef,
        right: ExprRef,
    },
    Pow {
        left: ExprRef,
        right: ExprRef,
    },
    Equal {
        left: ExprRef,
        right: ExprRef,
    },
    Lt {
        left: ExprRef,
        right: ExprRef,
    },
    Le {
        left: ExprRef,
        right: ExprRef,
    },
    Subscript {
        left: ExprRef,
        right: ExprRef,
    },
    Superscript {
        left: ExprRef,
        right: ExprRef,
    },
    Neg {
        value: ExprRef,
    },
    Sqrt {
        value: ExprRef,
    },
    Transpose {
        value: ExprRef,
    },
    Det {
        value: ExprRef,
    },
    Root {
        degree: ExprRef,
        radicand: ExprRef,
    },
    Scripts {
        base: ExprRef,
        sub: ExprRef,
        sup: ExprRef,
    },
    Fence {
        open: String,
        close: String,
        value: ExprRef,
    },
    Sequence {
        values: Vec<ExprRef>,
    },
    Text {
        text: String,
    },
    Vector {
        values: Vec<ExprRef>,
    },
    Matrix {
        rows: Vec<RowRef>,
    },
    Let {
        name: String,
        init: ExprRef,
        body: ExprRef,
    },
    Sum {
        index: String,
        lower: ExprRef,
        upper: ExprRef,
        body: ExprRef,
    },
    Integral {
        index: String,
        lower: ExprRef,
        upper: ExprRef,
        body: ExprRef,
    },
    Call {
        function: ExprRef,
        arguments: Vec<ExprRef>,
    },
    Label {
        value: ExprRef,
        annotation: DocGuestRef,
    },
    Row {
        values: Vec<ExprRef>,
    },
    DocGuest {
        syntax: EmbedRef,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathNode {
    pub kind: MathKind,
    pub origin: Option<OriginId>,
    pub span: Option<Span>,
    pub locations: Vec<MathFieldLocation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathValue {
    pub root: MathRoot,
    pub nodes: Vec<MathNode>,
    /// Doc Sentence syntax with its owner environment and source closure. A
    /// display annotation is not a dependency on Doc meaning or evaluation.
    pub embeds: Vec<ForeignClosure>,
}
/// View IDs remain local to the original token owner head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathView {
    pub head: Span,
    pub view: ViewBundle,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathSyntax {
    pub value: MathValue,
    pub sources: Vec<SourceSnapshot>,
    pub origins: Vec<Origin>,
    pub views: Vec<MathView>,
    pub source_maps: Vec<Mapping>,
}
