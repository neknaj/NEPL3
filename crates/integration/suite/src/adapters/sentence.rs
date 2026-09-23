//! Pure integration bridge from independent Sentence to the current
//! Doc consumer. Neither domain core depends on the other. The original
//! SentenceSyntax remains available with its dense head/cover and View owners.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
};
use nepl3_doc_core::model as d;
use nepl3_sentence_core::{model as s, syntax::SentenceSyntax};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Sentence(nepl3_sentence_core::syntax::Error),
    Document(nepl3_doc_core::check::StructureError),
    ForeignAdapterRequired(s::EmbedRef),
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<nepl3_sentence_core::syntax::Error> for Error {
    fn from(e: nepl3_sentence_core::syntax::Error) -> Self {
        match e {
            nepl3_sentence_core::syntax::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Sentence(e),
        }
    }
}
impl From<nepl3_doc_core::check::StructureError> for Error {
    fn from(e: nepl3_doc_core::check::StructureError) -> Self {
        match e {
            nepl3_doc_core::check::StructureError::Stopped(s) => Self::Stopped(s),
            e => Self::Document(e),
        }
    }
}
fn push<T>(out: &mut Vec<T>, v: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    out.push(v);
    Ok(())
}
fn text(s: &str, b: &mut Budget) -> Result<String, Error> {
    b.charge(Resource::Work, s.len() as u64)?;
    b.charge(Resource::AllocationUnits, s.len() as u64)?;
    Ok(s.into())
}
fn inlines(values: &[s::InlineRef], b: &mut Budget) -> Result<Vec<d::InlineRef>, Error> {
    let mut out = Vec::new();
    for v in values {
        b.charge(Resource::Work, 1)?;
        push(&mut out, d::InlineRef(v.0), b)?;
    }
    Ok(out)
}
/// Preserve local arena indices and ordered content. URLs remain data, Code is
/// inline code (not Doc's foreign Code), and foreign syntax needs a role-specific
/// adapter instead of guessing a guest language. This does not resolve links,
/// prepare HTML, or claim that Doc's old Sentence ownership has been removed.
pub fn document(
    input: &SentenceSyntax,
    r: &SchemaRegistry,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<d::DocumentSyntax, Error> {
    input.validate(r, b, a)?;
    let mut nodes = Vec::new();
    for (kind, location) in input.value.nodes.iter().zip(&input.locations) {
        b.charge(Resource::Work, 1)?;
        let kind = match kind {
            s::Kind::Sentence { inlines: v } => d::DocKind::Sentence {
                inlines: inlines(v, b)?,
            },
            s::Kind::Text { text: v } => d::DocKind::Text { text: text(v, b)? },
            s::Kind::Concat { inlines: v } => d::DocKind::Concat {
                inlines: inlines(v, b)?,
            },
            s::Kind::Ruby { base, reading } => d::DocKind::Ruby {
                base: d::InlineRef(base.0),
                reading: d::InlineRef(reading.0),
            },
            s::Kind::InlineAnno { base, notes } => d::DocKind::Anno {
                base: d::InlineRef(base.0),
                notes: inlines(notes, b)?,
            },
            s::Kind::Code { text: v } => d::DocKind::InlineCode { text: text(v, b)? },
            s::Kind::Emphasis { inline } => d::DocKind::Emphasis {
                inline: d::InlineRef(inline.0),
            },
            s::Kind::Strong { inline } => d::DocKind::Strong {
                inline: d::InlineRef(inline.0),
            },
            s::Kind::Break => d::DocKind::Break,
            s::Kind::ExternalLink { uri, label } => d::DocKind::Link {
                target: d::LinkTarget::External { uri: text(uri, b)? },
                label: d::InlineRef(label.0),
            },
            s::Kind::ForeignInline { syntax } => {
                return Err(Error::ForeignAdapterRequired(*syntax));
            }
        };
        let span = location
            .cover
            .as_ref()
            .map(|span| {
                b.charge(Resource::Work, span.snapshot_ref().source.0.len() as u64)?;
                b.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of_val(span) + span.snapshot_ref().source.0.len()) as u64,
                )?;
                Ok::<_, Error>(span.clone())
            })
            .transpose()?;
        b.charge(Resource::Nodes, 1)?;
        push(
            &mut nodes,
            d::DocNode {
                kind,
                span,
                origin: Some(location.origin),
                locations: Vec::new(),
            },
            b,
        )?;
    }
    let root = match input.value.root {
        s::Root::Sentence(v) => d::DocRoot::Sentence(d::SentenceRef(v.0)),
        s::Root::Inline(v) => d::DocRoot::Inline(d::InlineRef(v.0)),
    };
    let mut out = d::DocumentSyntax {
        value: d::DocValue {
            root,
            nodes,
            embeds: Vec::new(),
        },
        sources: Vec::new(),
        origins: Vec::new(),
        views: Vec::new(),
        source_maps: Vec::new(),
    };
    for s in &input.sources {
        push(&mut out.sources, s.clone_with_budget(b)?, b)?;
    }
    for o in &input.origins {
        push(&mut out.origins, o.clone_with_budget(b)?, b)?;
    }
    for m in &input.source_maps {
        push(&mut out.source_maps, m.clone_with_budget(b)?, b)?;
    }
    for v in &input.views {
        b.charge(Resource::Work, v.head.snapshot_ref().source.0.len() as u64)?;
        b.charge(
            Resource::AllocationUnits,
            (core::mem::size_of_val(&v.head) + v.head.snapshot_ref().source.0.len()) as u64,
        )?;
        let view = d::DocView {
            head: v.head.clone(),
            view: v.view.clone_with_budget(b)?,
        };
        push(&mut out.views, view, b)?;
    }
    out.validate_structure(r, b, a)?;
    Ok(out)
}
