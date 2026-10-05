//! Explicit Math capability for a checked Doc page; no schema guessing.
use super::*;
use crate::doc::math::{
    MathDisplayHost,
    markdown::{EmitError, Placement, PrepareError, PreparedMarkdown},
};
use nepl3_core::value::SchemaRef;
use nepl3_doc_core::model::{EmbedKind, EmbedRef};

#[derive(Clone, Copy)]
pub struct MathSurfaces<'a> {
    pub math: &'a SchemaRef,
    pub sentence: Option<&'a SchemaRef>,
    pub doc: Option<&'a SchemaRef>,
}

pub(super) struct MathEntry<'a> {
    pub node: u64,
    pub embed: EmbedRef,
    prepared: PreparedMarkdown<'a>,
}

pub(super) fn prepare<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    surfaces: MathSurfaces<'_>,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<Vec<MathEntry<'a>>, Error>
where
    C::Error: core::fmt::Debug,
{
    let mut host = MathDisplayHost {
        registry,
        math_surface: surfaces.math,
        sentence_surface: surfaces.sentence,
        doc_surface: surfaces.doc,
        codec,
    };
    let mut entries = Vec::new();
    for (node, value) in document.value.nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let embed = match value.kind {
            DocKind::InlineMath { syntax } | DocKind::DisplayMath { syntax } => syntax,
            _ => continue,
        };
        let prepared = host
            .prepare_markdown_node(document, node as u64, budget)
            .map_err(|e| match e {
                PrepareError::Host(crate::doc::math::Error::Stopped(s)) => Error::Stopped(s),
                e => Error::Invalid(format!("Markdown Math: {e:?}")),
            })?;
        push(
            &mut entries,
            MathEntry {
                node: node as u64,
                embed,
                prepared,
            },
            budget,
        )?;
    }
    Ok(entries)
}

pub(super) fn resolves(
    entries: &[MathEntry<'_>],
    requirement: &prepare::DocRequirement,
    budget: &mut Budget,
) -> Result<bool, Error> {
    let prepare::DocRequirement::Foreign {
        embed,
        kind: EmbedKind::InlineMath | EmbedKind::DisplayMath,
        ..
    } = requirement
    else {
        return Ok(false);
    };
    for entry in entries {
        budget.charge(Resource::Work, 1)?;
        if entry.embed == *embed {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Annotated<'_, '_> {
    pub(super) fn math(&mut self, node: u64, placement: Placement) -> Result<(), Error> {
        let mut selected = None;
        for entry in self.math {
            self.plain.budget.charge(Resource::Work, 1)?;
            if entry.node == node {
                selected = Some(entry);
                break;
            }
        }
        let entry = selected.ok_or(Error::NeedsResolution)?;
        let fragment = entry
            .prepared
            .emit(self.plain.doc, node, placement, self.plain.budget)
            .map_err(|e| match e {
                EmitError::Stopped(s) => Error::Stopped(s),
                _ => Error::Unsupported { node },
            })?;
        self.plain.emit(&fragment)?;
        push(&mut self.emitted_math, node, self.plain.budget)
    }
}
