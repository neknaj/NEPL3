//! Private consuming transfer to the closed native Doc import path.
use super::*;
use crate::doc::annotations::MathRecord;

pub(in crate::doc::math) struct Metadata<'c, 'r> {
    pub doc_node: u64,
    pub display: Display,
    pub tex: TexPreparation,
    pub representation: Representation,
    pub fallback: Option<FallbackReason>,
    pub generated: Option<GeneratedRange>,
    pub stylesheet: String,
    pub assets: Option<&'r PreparedAssets>,
    pub selection: Option<RendererSelection<'c>>,
    pub scope: String,
    pub observations: Option<Observations>,
    pub termination_failure: Option<bool>,
}
pub(in crate::doc::math) struct Parts<'c, 'r> {
    pub markup: HtmlRequest,
    pub math: MathRecord,
    pub metadata: Metadata<'c, 'r>,
}
impl<'c, 'r> ComposedMath<'c, 'r> {
    pub(in crate::doc::math) fn into_import_parts(self) -> Parts<'c, 'r> {
        let Self {
            math,
            doc_node,
            display,
            tex,
            representation,
            fallback,
            generated,
            stylesheet,
            assets,
            selection,
            scope,
            observations,
            termination_failure,
        } = self;
        let RenderedHtmlMath {
            syntax,
            markup,
            node_roots,
            annotation_roots,
            annotations,
        } = math;
        Parts {
            markup,
            math: MathRecord {
                syntax,
                node_roots,
                annotation_roots,
                annotations,
            },
            metadata: Metadata {
                doc_node,
                display,
                tex,
                representation,
                fallback,
                generated,
                stylesheet,
                assets,
                selection,
                scope,
                observations,
                termination_failure,
            },
        }
    }
}
