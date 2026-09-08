//! Typed arena references make recursive paragraphs and annotations shallow to
//! drop. A public reference is raw data until the category/graph checks succeed.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    origin::{Mapping, Origin, OriginId},
    source::{Digest, SourceSnapshot, Span},
    syntax::ForeignClosure,
    view::ViewBundle,
};
macro_rules! references {
    ($($name:ident),+ $(,)?) => {$ (
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name(pub u64);
    )+};
}
references!(
    ArticleRef,
    BodyRef,
    BlockRef,
    FlowRef,
    SentenceRef,
    InlineRef,
    VariantRef,
    RowRef,
    ListItemRef,
    EmbedRef,
    AlignmentRef,
    ListStyleRef,
    CheckRef,
    TargetRef,
    AssetValueRef,
    OptionalRowRef,
    OptionalSentenceRef,
    OptionalTextRef,
    GuestRef
);

/// The semantic name operand, distinct from its enclosing definition cover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocField {
    SectionId,
    AnchorId,
    ReferenceTarget,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocFieldLocation {
    pub field: DocField,
    pub origin: Option<OriginId>,
    pub span: Option<Span>,
}
/// One edge in the ordered semantic child sequence of a Doc node. Paths
/// distinguish repeated presentation occurrences without inventing source spans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocPathStep {
    pub owner: u64,
    pub child: u64,
    pub target: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabelOccurrencePaths {
    pub first: Vec<DocPathStep>,
    pub second: Vec<DocPathStep>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabelDiagnosticArguments {
    pub name: String,
    pub paths: Option<LabelOccurrencePaths>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocRoot {
    Article(ArticleRef),
    Body(BodyRef),
    Block(BlockRef),
    Flow(FlowRef),
    Sentence(SentenceRef),
    Inline(InlineRef),
    Variant(VariantRef),
    Row(RowRef),
    ListItem(ListItemRef),
    Alignment(AlignmentRef),
    ListStyle(ListStyleRef),
    Check(CheckRef),
    Target(TargetRef),
    Asset(AssetValueRef),
    OptionalRow(OptionalRowRef),
    OptionalSentence(OptionalSentenceRef),
    OptionalText(OptionalTextRef),
    Guest(GuestRef),
    MathGuest(GuestRef),
    CircuitGuest(GuestRef),
}
/// Explicit standard Doc surface wrapper, never inferred from a schema alias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestLanguage {
    Math,
    Circuit,
    Grammar,
    Doc,
}
/// Resource identities are data, not permission to load or render an asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetRef {
    pub id: String,
    pub digest: Option<Digest>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkTarget {
    Page {
        page: String,
        fragment: Option<String>,
    },
    Relative {
        path: String,
        fragment: Option<String>,
    },
    External {
        uri: String,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alignment {
    Default,
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListKind {
    Unordered,
    Ordered { start: u64 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmbedKind {
    InlineMath,
    DisplayMath,
    CircuitFigure,
    Code,
    Guest,
}

/// Closed Doc node kinds. Category checks distinguish references to, for
/// example, an Inline from those to a Sentence even though both use an arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocKind {
    /// A standalone foreign wrapper retains syntax and its concrete surface
    /// entry. Embedded operands continue to be owned by their parent slot.
    Guest {
        language: GuestLanguage,
        syntax: EmbedRef,
    },
    // Surface auxiliary categories have standalone fragment roots. When used
    // as a parent constructor operand, lowering stores their typed value there.
    Alignment {
        alignment: Alignment,
    },
    ListStyle {
        style: ListKind,
    },
    Check {
        checked: Option<bool>,
    },
    Target {
        target: LinkTarget,
    },
    Asset {
        asset: AssetRef,
    },
    OptionalRow {
        row: Option<RowRef>,
    },
    OptionalSentence {
        sentence: Option<SentenceRef>,
    },
    OptionalText {
        text: Option<String>,
    },
    Article {
        language: String,
        title: SentenceRef,
        body: BodyRef,
    },
    Body {
        blocks: Vec<BlockRef>,
    },
    Paragraph {
        items: Vec<FlowRef>,
    },
    Section {
        id: String,
        title: SentenceRef,
        body: BodyRef,
    },
    Sentence {
        inlines: Vec<InlineRef>,
    },
    Parallel {
        variants: Vec<VariantRef>,
    },
    Variant {
        language: String,
        sentence: SentenceRef,
    },
    Text {
        text: String,
    },
    Concat {
        inlines: Vec<InlineRef>,
    },
    Ruby {
        base: InlineRef,
        reading: InlineRef,
    },
    Anno {
        base: InlineRef,
        notes: Vec<InlineRef>,
    },
    InlineMath {
        syntax: EmbedRef,
    },
    Anchor {
        id: String,
        label: InlineRef,
    },
    Reference {
        target: String,
        label: InlineRef,
    },
    Emphasis {
        inline: InlineRef,
    },
    Strong {
        inline: InlineRef,
    },
    Break,
    DisplayMath {
        syntax: EmbedRef,
    },
    CircuitFigure {
        caption: SentenceRef,
        syntax: EmbedRef,
    },
    Code {
        syntax: EmbedRef,
    },
    // Inventory DG01-DG06: preserve these distinctions rather than encoding
    // tables/lists/links/code/assets as ordinary paragraphs or source text.
    Table {
        columns: Vec<Alignment>,
        header: Option<RowRef>,
        rows: Vec<RowRef>,
    },
    Row {
        cells: Vec<SentenceRef>,
    },
    List {
        kind: ListKind,
        items: Vec<ListItemRef>,
    },
    ListItem {
        checked: Option<bool>,
        body: BodyRef,
    },
    Link {
        target: LinkTarget,
        label: InlineRef,
    },
    InlineCode {
        text: String,
    },
    RawCode {
        language_hint: Option<String>,
        text: String,
    },
    Image {
        asset: AssetRef,
        alt: SentenceRef,
        caption: Option<SentenceRef>,
    },
    InlineImage {
        asset: AssetRef,
        alt: SentenceRef,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocNode {
    /// Empty or explicitly absent positions represent source-less constructors.
    pub locations: Vec<DocFieldLocation>,
    pub kind: DocKind,
    pub origin: Option<OriginId>,
    pub span: Option<Span>,
}
/// Code embeds, including Doc-to-Doc, retain the parsed guest source. This data
/// never implies that the guest has been lowered, compiled or evaluated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocEmbed {
    pub kind: EmbedKind,
    pub closure: ForeignClosure,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocValue {
    pub root: DocRoot,
    pub nodes: Vec<DocNode>,
    pub embeds: Vec<DocEmbed>,
}
/// ViewRef values and relations remain local to this original owner head.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocView {
    pub head: Span,
    pub view: ViewBundle,
}
/// Source presentation is separate from semantic normalization. Views and
/// origins retain individual spelling/escape ranges when Text nodes coalesce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSyntax {
    pub value: DocValue,
    pub sources: Vec<SourceSnapshot>,
    pub origins: Vec<Origin>,
    pub views: Vec<DocView>,
    pub source_maps: Vec<Mapping>,
}
