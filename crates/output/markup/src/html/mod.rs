//! A closed HTML fragment model. Validation is structural; resource availability
//! and CSS provenance are host preparation obligations, not inferred from paths.
mod check;
mod ruby;
mod serialize;
mod uri;
use alloc::{string::String, vec::Vec};
pub use check::{HtmlError, ValidatedHtml, validate};
pub use serialize::serialize;

macro_rules! tags {
    ($($tag:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum HtmlTag { $($tag),+ }
        impl HtmlTag { pub fn name(self)->&'static str { match self { $(Self::$tag => $name),+ } } }
    }
}
tags!(Article=>"article",Section=>"section",Div=>"div",P=>"p",Span=>"span",
    H1=>"h1",H2=>"h2",H3=>"h3",H4=>"h4",H5=>"h5",H6=>"h6",
    Ruby=>"ruby",Rt=>"rt",Rp=>"rp",Em=>"em",Strong=>"strong",Br=>"br",
    Pre=>"pre",Code=>"code",Figure=>"figure",Figcaption=>"figcaption",A=>"a",
    Ul=>"ul",Ol=>"ol",Li=>"li",Table=>"table",Caption=>"caption",Thead=>"thead",
    Tbody=>"tbody",Tr=>"tr",Th=>"th",Td=>"td",Img=>"img");
impl HtmlTag {
    pub fn is_void(self) -> bool {
        matches!(self, Self::Br | Self::Img)
    }
    pub fn is_phrasing(self) -> bool {
        matches!(
            self,
            Self::Span
                | Self::Ruby
                | Self::Em
                | Self::Strong
                | Self::Br
                | Self::Code
                | Self::A
                | Self::Img
        )
    }
    pub fn is_flow(self) -> bool {
        self.is_phrasing()
            || matches!(
                self,
                Self::Article
                    | Self::Section
                    | Self::Div
                    | Self::P
                    | Self::H1
                    | Self::H2
                    | Self::H3
                    | Self::H4
                    | Self::H5
                    | Self::H6
                    | Self::Pre
                    | Self::Figure
                    | Self::Ul
                    | Self::Ol
                    | Self::Table
            )
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HtmlSlot {
    Block,
    Phrasing,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HtmlRole {
    Heading,
    Img,
    Group,
    Note,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellScope {
    Row,
    Col,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HtmlHref {
    /// Two paths in the same artifact root. Host preparation must bind source
    /// to the actual document route and verify target/fragment availability.
    BetweenArtifacts {
        source: String,
        target: String,
        fragment: Option<String>,
    },
    Fragment {
        id: String,
    },
    /// Artifact-relative URL segments; no network-path, dot segments or escapes.
    Artifact {
        path: String,
        fragment: Option<String>,
    },
    /// Canonical ASCII URI in the constrained http/https/mailto output profile.
    External {
        uri: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HtmlAttribute {
    Id { value: String },
    Class { values: Vec<String> },
    Lang { value: String },
    Role { value: HtmlRole },
    AriaLabel { value: String },
    AriaLevel { value: u64 },
    DataId { value: String },
    DataGroup { value: String },
    Href { value: HtmlHref },
    Src { path: String },
    Alt { value: String },
    Width { value: u64 },
    Height { value: u64 },
    Start { value: u64 },
    Scope { value: CellScope },
}
impl HtmlAttribute {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Id { .. } => "id",
            Self::Class { .. } => "class",
            Self::Lang { .. } => "lang",
            Self::Role { .. } => "role",
            Self::AriaLabel { .. } => "aria-label",
            Self::AriaLevel { .. } => "aria-level",
            Self::DataId { .. } => "data-nepl-id",
            Self::DataGroup { .. } => "data-nepl-group",
            Self::Href { .. } => "href",
            Self::Src { .. } => "src",
            Self::Alt { .. } => "alt",
            Self::Width { .. } => "width",
            Self::Height { .. } => "height",
            Self::Start { .. } => "start",
            Self::Scope { .. } => "scope",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HtmlNode {
    Text {
        text: String,
    },
    Element {
        tag: HtmlTag,
        attributes: Vec<HtmlAttribute>,
        children: Vec<u64>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlFragment {
    pub root: u64,
    pub nodes: Vec<HtmlNode>,
}
/// These names come from a prepared backend stylesheet. A raw policy received
/// over the wire does not prove stylesheet ownership or asset integrity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlPolicy {
    pub classes: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlRequest {
    pub fragment: HtmlFragment,
    pub slot: HtmlSlot,
    pub policy: HtmlPolicy,
}
