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
references!(ExprRef, RowRef, SentenceGuestRef, EmbedRef);

/// One exact, case-sensitive free-symbol assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathAssignment {
    pub name: String,
    pub value: MathExactValue,
}

/// Canonical ascending UTF-8 names, with no duplicates. Use environment::check
/// before lookup; unused assignments are permitted but must also be valid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingEnvironment {
    pub assignments: Vec<MathAssignment>,
}

/// Evaluation values are separate from source notation. Rational values need
/// not have finite decimal expansions. Matrix storage is row-major.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathExactValue {
    Scalar {
        value: Rational,
    },
    Vector {
        values: Vec<Rational>,
    },
    Matrix {
        rows: u64,
        cols: u64,
        values: Vec<Rational>,
    },
    Truth {
        value: bool,
    },
}

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

/// One required free name, with all its unbound occurrence IDs in preorder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathFreeSymbol {
    pub name: String,
    pub occurrences: Vec<u64>,
}

/// Exact, case-sensitive names sorted by UTF-8 lexical order. No normalization
/// or invented definition/source position is attached to a free name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathFreeSymbols {
    pub symbols: Vec<MathFreeSymbol>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathRoot {
    Expr(ExprRef),
    Row(RowRef),
    SentenceGuest(SentenceGuestRef),
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
        annotation: SentenceGuestRef,
    },
    Row {
        values: Vec<ExprRef>,
    },
    SentenceGuest {
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
    /// Independent Sentence syntax with its owner environment and source closure. A
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathEvaluationReason {
    MissingSymbol,
    NotationOnly,
    NonIntegralExponent,
    AlgebraicValueRequired,
    ComplexValueRequired,
    UnsupportedExactDomain,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathEvaluationRequirement {
    pub expression: ExprRef,
    pub reason: MathEvaluationReason,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathEvaluationOutcome {
    Exact(MathExactValue),
    Symbolic(Vec<MathEvaluationRequirement>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathEvaluationFailureKind {
    OperandShapeMismatch,
    NotSquare,
    DivisionByZero,
    InvalidRootDegree,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathEvaluationFailure {
    pub expression: ExprRef,
    pub kind: MathEvaluationFailureKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathEvaluationResult {
    Success { outcome: MathEvaluationOutcome },
    Failure { failure: MathEvaluationFailure },
}
/// Exact Math surface category used by shape checking and source reparsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathCategory {
    Expr,
    Row,
    SentenceGuest,
}

/// Generated source without an invented saved source identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathSourceArtifact {
    pub text: alloc::string::String,
    pub entry: MathCategory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathPrintedGuest {
    pub syntax_digest: nepl3_core::source::Digest,
    pub guest_digest: nepl3_core::source::Digest,
    pub embed: EmbedRef,
    pub text: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathPrintIdentity {
    pub syntax_digest: nepl3_core::source::Digest,
    pub guests: Vec<nepl3_core::source::Digest>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathPrintRequest {
    pub syntax: MathSyntax,
    pub sentence_schema: Option<nepl3_core::value::SchemaRef>,
    pub guests: Vec<MathPrintedGuest>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathPrintFailure {
    InvalidGuestIdentity { entry: u64 },
    DuplicateGuest { embed: EmbedRef },
    MissingBinding { embed: EmbedRef },
    GuestCategory { embed: EmbedRef },
    UnresolvedGuest { embed: EmbedRef },
    EmptyGuest { embed: EmbedRef },
    UnprintableName { node: u64 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MathPrintResult {
    Complete {
        artifact: MathSourceArtifact,
    },
    Invalid {
        failure: MathPrintFailure,
    },
    Stopped {
        reason: nepl3_core::budget::StopReason,
    },
}
