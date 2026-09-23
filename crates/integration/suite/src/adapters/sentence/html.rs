//! Native Sentence-to-markup projection. The host supplies the stylesheet and
//! selects foreign adapters; this module performs no guest execution or I/O.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
};
use nepl3_markup::html::*;
use nepl3_sentence_core::{model::*, syntax::SentenceSyntax};

const CLASSES: &[&str] = &[
    "nepl-sentence",
    "nepl-ruby",
    "nepl-base",
    "nepl-reading",
    "nepl-anno",
    "nepl-notes",
    "nepl-note",
];

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Input(nepl3_sentence_core::syntax::Error),
    Markup(HtmlError),
    ForeignAdapterRequired(EmbedRef),
    InternalShape,
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<nepl3_sentence_core::syntax::Error> for Error {
    fn from(error: nepl3_sentence_core::syntax::Error) -> Self {
        match error {
            nepl3_sentence_core::syntax::Error::Stopped(s) => Self::Stopped(s),
            error => Self::Input(error),
        }
    }
}
impl From<HtmlError> for Error {
    fn from(error: HtmlError) -> Self {
        match error {
            HtmlError::Stopped(s) => Self::Stopped(s),
            error => Self::Markup(error),
        }
    }
}

/// Each emitted element or text refers to its semantic cause in the input.
/// Repeated occurrences of a shared node retain distinct output element IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ElementOrigin {
    pub element: u64,
    pub node: u64,
}

/// Immutable output and correspondence bound to the live original syntax.
/// No generated-source Span or transferable validation identity is invented.
pub struct RenderedSentence<'a> {
    input: &'a SentenceSyntax,
    markup: HtmlRequest,
    origins: Vec<ElementOrigin>,
}
impl<'a> RenderedSentence<'a> {
    pub fn input(&self) -> &'a SentenceSyntax {
        self.input
    }
    pub fn markup(&self) -> &HtmlRequest {
        &self.markup
    }
    pub fn origins(&self) -> &[ElementOrigin] {
        &self.origins
    }
    pub fn into_markup(self) -> HtmlRequest {
        self.markup
    }
}

fn push<T>(values: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    values.push(value);
    Ok(())
}
fn copy(text: &str, b: &mut Budget) -> Result<String, Error> {
    b.charge(Resource::Work, text.len() as u64)?;
    b.charge(Resource::AllocationUnits, text.len() as u64)?;
    Ok(text.into())
}
struct Job {
    node: u64,
    parent: u64,
}
struct Builder<'b> {
    nodes: Vec<HtmlNode>,
    depths: Vec<u64>,
    origins: Vec<ElementOrigin>,
    jobs: Vec<Job>,
    b: &'b mut Budget,
}
impl Builder<'_> {
    fn add(&mut self, parent: Option<u64>, cause: u64, node: HtmlNode) -> Result<u64, Error> {
        let depth = match parent {
            Some(p) => self
                .depths
                .get(p as usize)
                .ok_or(Error::InternalShape)?
                .saturating_add(1),
            None => 1,
        };
        self.b.observe_depth(depth)?;
        self.b.charge(Resource::Nodes, 1)?;
        let id = self.nodes.len() as u64;
        push(&mut self.nodes, node, self.b)?;
        push(&mut self.depths, depth, self.b)?;
        push(
            &mut self.origins,
            ElementOrigin {
                element: id,
                node: cause,
            },
            self.b,
        )?;
        if let Some(p) = parent {
            let Some(HtmlNode::Element { children, .. }) = self.nodes.get_mut(p as usize) else {
                return Err(Error::InternalShape);
            };
            push(children, id, self.b)?;
        }
        Ok(id)
    }
    fn element(&mut self, parent: Option<u64>, cause: u64, tag: HtmlTag) -> Result<u64, Error> {
        self.add(
            parent,
            cause,
            HtmlNode::Element {
                tag,
                attributes: Vec::new(),
                children: Vec::new(),
            },
        )
    }
    fn attr(&mut self, id: u64, attr: HtmlAttribute) -> Result<(), Error> {
        let Some(HtmlNode::Element { attributes, .. }) = self.nodes.get_mut(id as usize) else {
            return Err(Error::InternalShape);
        };
        push(attributes, attr, self.b)
    }
    fn class(&mut self, id: u64, name: &str) -> Result<(), Error> {
        let name = copy(name, self.b)?;
        let mut values = Vec::new();
        push(&mut values, name, self.b)?;
        self.attr(id, HtmlAttribute::Class { values })
    }
    fn job(&mut self, node: u64, parent: u64) -> Result<(), Error> {
        push(&mut self.jobs, Job { node, parent }, self.b)
    }
    fn text(&mut self, parent: u64, cause: u64, value: &str) -> Result<(), Error> {
        let text = copy(value, self.b)?;
        self.add(Some(parent), cause, HtmlNode::Text { text })?;
        Ok(())
    }
}

/// Render standard Sentence/Inline content to a checked phrasing fragment.
/// The class names use the existing NEPL Ruby/Anno layout contract; the host
/// remains responsible for stylesheet provenance. Foreign values require a
/// role-specific adapter and are reported explicitly without partial output.
pub fn render<'a>(
    input: &'a SentenceSyntax,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<RenderedSentence<'a>, Error> {
    input.validate(registry, b, admission)?;
    let root = match input.value.root {
        Root::Sentence(r) => r.0,
        Root::Inline(r) => r.0,
    };
    let mut builder = Builder {
        nodes: Vec::new(),
        depths: Vec::new(),
        origins: Vec::new(),
        jobs: Vec::new(),
        b,
    };
    let output_root = builder.element(None, root, HtmlTag::Span)?;
    builder.class(output_root, "nepl-sentence")?;
    builder.job(root, output_root)?;
    while let Some(Job { node, parent }) = builder.jobs.pop() {
        builder.b.charge(Resource::Work, 1)?;
        match input
            .value
            .nodes
            .get(node as usize)
            .ok_or(Error::InternalShape)?
        {
            Kind::Text { text } => builder.text(parent, node, text)?,
            Kind::Sentence { inlines } | Kind::Concat { inlines } => {
                let target = builder.element(Some(parent), node, HtmlTag::Span)?;
                for child in inlines.iter().rev() {
                    builder.job(child.0, target)?;
                }
            }
            Kind::Code { text } => {
                let target = builder.element(Some(parent), node, HtmlTag::Code)?;
                builder.text(target, node, text)?;
            }
            Kind::Emphasis { inline } | Kind::Strong { inline } => {
                let tag = if matches!(input.value.nodes[node as usize], Kind::Emphasis { .. }) {
                    HtmlTag::Em
                } else {
                    HtmlTag::Strong
                };
                let target = builder.element(Some(parent), node, tag)?;
                builder.job(inline.0, target)?;
            }
            Kind::Break => {
                builder.element(Some(parent), node, HtmlTag::Br)?;
            }
            Kind::ExternalLink { uri, label } => {
                let target = builder.element(Some(parent), node, HtmlTag::A)?;
                let uri = copy(uri, builder.b)?;
                builder.attr(
                    target,
                    HtmlAttribute::Href {
                        value: HtmlHref::External { uri },
                    },
                )?;
                builder.job(label.0, target)?;
            }
            Kind::Ruby { base, reading } => {
                let target = builder.element(Some(parent), node, HtmlTag::Span)?;
                builder.class(target, "nepl-ruby")?;
                let base_target = builder.element(Some(target), node, HtmlTag::Span)?;
                builder.class(base_target, "nepl-base")?;
                let reading_target = builder.element(Some(target), node, HtmlTag::Span)?;
                builder.class(reading_target, "nepl-reading")?;
                builder.job(reading.0, reading_target)?;
                builder.job(base.0, base_target)?;
            }
            Kind::InlineAnno { base, notes } => {
                let target = builder.element(Some(parent), node, HtmlTag::Span)?;
                builder.class(target, "nepl-anno")?;
                let base_target = builder.element(Some(target), node, HtmlTag::Span)?;
                builder.class(base_target, "nepl-base")?;
                let notes_target = builder.element(Some(target), node, HtmlTag::Span)?;
                builder.class(notes_target, "nepl-notes")?;
                let start = builder.jobs.len();
                for note in notes {
                    let target = builder.element(Some(notes_target), node, HtmlTag::Span)?;
                    builder.class(target, "nepl-note")?;
                    builder.job(note.0, target)?;
                }
                builder
                    .b
                    .charge(Resource::Work, (builder.jobs.len() - start) as u64)?;
                builder.jobs[start..].reverse();
                builder.job(base.0, base_target)?;
            }
            Kind::ForeignInline { syntax } => return Err(Error::ForeignAdapterRequired(*syntax)),
        }
    }
    let mut classes = Vec::new();
    for name in CLASSES {
        let name = copy(name, builder.b)?;
        push(&mut classes, name, builder.b)?;
    }
    let markup = HtmlRequest {
        fragment: HtmlFragment {
            root: output_root,
            nodes: builder.nodes,
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes },
    };
    validate(&markup.fragment, markup.slot, &markup.policy, builder.b)?;
    Ok(RenderedSentence {
        input,
        markup,
        origins: builder.origins,
    })
}
