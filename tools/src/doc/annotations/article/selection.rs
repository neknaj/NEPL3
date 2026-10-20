use super::super::SentenceAnnotationRenderer;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::{StructuralOccurrence, StructureError},
    model::*,
};
use nepl3_sentence_core::syntax::SentenceSyntax;
use nepl3_suite::adapters::sentence::document_guests::OwnerId;

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Structure(StructureError),
    ExpectedArticle,
    Lower(super::super::Error<E>),
    InternalShape,
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct Owner {
    embed: EmbedRef,
    syntax: SentenceSyntax,
}
impl Owner {
    pub fn embed(&self) -> EmbedRef {
        self.embed
    }
    pub fn syntax(&self) -> &SentenceSyntax {
        &self.syntax
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentenceOccurrence {
    /// Index into Selection::structure(), not an HTML or Doc arena index.
    pub occurrence: usize,
    /// Index into this exact selection's unique Sentence owner table.
    pub owner: OwnerId,
}
pub struct Selection<'a> {
    document: &'a DocumentSyntax,
    structure: Vec<StructuralOccurrence>,
    owners: Vec<Owner>,
    sentences: Vec<SentenceOccurrence>,
}
/// Owned selection data for a host that separately retains the exact outer
/// DocumentSyntax. This container is not a validation or rendering proof.
pub struct SelectionParts {
    pub structure: Vec<StructuralOccurrence>,
    pub owners: Vec<Owner>,
    pub sentences: Vec<SentenceOccurrence>,
}
impl<'a> Selection<'a> {
    pub fn into_parts(self) -> SelectionParts {
        SelectionParts {
            structure: self.structure,
            owners: self.owners,
            sentences: self.sentences,
        }
    }
    pub fn document(&self) -> &'a DocumentSyntax {
        self.document
    }
    pub fn structure(&self) -> &[StructuralOccurrence] {
        &self.structure
    }
    pub fn owners(&self) -> &[Owner] {
        &self.owners
    }
    pub fn sentences(&self) -> &[SentenceOccurrence] {
        &self.sentences
    }
}
fn push<T>(items: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(value);
    Ok(())
}
/// Validate the complete outer structure, then decode each explicitly selected
/// Sentence owner once at its deepest structural occurrence. Sources remain in
/// the same operation's admission ledger. Equal numeric EmbedRefs in another
/// Article do not denote these owners.
///
/// All variants and image alt subtrees remain present. These records are not
/// visible placements or imported namespace members. In particular, callers
/// must not zip this structural sequence with the renderer's callbacks.
/// Foreign guests inside Sentence are retained, never recursively imported.
pub fn collect_structural<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<Selection<'a>, Error<C::Error>> {
    b.poll()?;
    let result = collect_inner(document, host, b);
    b.poll()?;
    result
}
fn collect_inner<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<Selection<'a>, Error<C::Error>> {
    let checked = document
        .validate_structure(host.registry, b, host.codec.source_admission())
        .map_err(|error| match error {
            StructureError::Stopped(reason) => Error::Stopped(reason),
            error => Error::Structure(error),
        })?;
    if !matches!(document.value.root, DocRoot::Article(_)) {
        return Err(Error::ExpectedArticle);
    }
    let structure = checked.shape().structural_occurrences(b)?;
    let mut depths = Vec::new();
    let mut mapping = Vec::new();
    for _ in 0..document.value.embeds.len() {
        b.charge(Resource::Work, 1)?;
        push(&mut depths, 0_u64, b)?;
        push(&mut mapping, None, b)?;
    }
    for occurrence in &structure {
        b.charge(Resource::Work, 1)?;
        if let DocKind::SentenceGuest { syntax } =
            document.value.nodes[occurrence.node as usize].kind
        {
            let depth = depths
                .get_mut(syntax.0 as usize)
                .ok_or(Error::InternalShape)?;
            *depth = (*depth).max(occurrence.depth);
        }
    }
    let base = b.current_depth();
    let mut owners = Vec::new();
    for (embed, depth) in depths.into_iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if depth == 0 {
            continue;
        }
        let view = document
            .value
            .embeds
            .get(embed)
            .ok_or(Error::InternalShape)?;
        let depth = base
            .checked_add(depth)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        let syntax = b
            .with_depth_at_least(depth, |b| host.lower_view(&view.closure, b))
            .map_err(|error| match error {
                super::super::Error::Stopped(reason) => Error::Stopped(reason),
                error => Error::Lower(error),
            })?;
        let owner = OwnerId(owners.len());
        push(
            &mut owners,
            Owner {
                embed: EmbedRef(embed as u64),
                syntax,
            },
            b,
        )?;
        mapping[embed] = Some(owner);
    }
    let mut sentences = Vec::new();
    for (index, occurrence) in structure.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if let DocKind::SentenceGuest { syntax } =
            document.value.nodes[occurrence.node as usize].kind
        {
            let owner = mapping
                .get(syntax.0 as usize)
                .copied()
                .flatten()
                .ok_or(Error::InternalShape)?;
            push(
                &mut sentences,
                SentenceOccurrence {
                    occurrence: index,
                    owner,
                },
                b,
            )?;
        }
    }
    b.poll()?;
    Ok(Selection {
        document,
        structure,
        owners,
        sentences,
    })
}
