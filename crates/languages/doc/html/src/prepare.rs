use crate::*;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    model::*,
    prepare::{self, DocPreparationPlan, PreparationError},
};

#[derive(Debug, Eq, PartialEq)]
pub enum LocalPreparationError<'a, E> {
    Input(PreparationError<'a, E>),
    Stopped(StopReason),
    NeedsResolution(DocPreparationPlan),
    Language,
    MissingVariant { node: u64 },
    ListStart { node: u64, start: u64 },
}
impl<E> From<StopReason> for LocalPreparationError<'_, E> {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
pub struct PreparedLocalArticle<'a>(pub(crate) PreparedRendering<'a>);
/// A standalone Sentence with local labels and no unresolved external inputs.
pub struct PreparedLocalSentence<'a>(pub(crate) PreparedRendering<'a>);
/// An Inline fragment with its own labels and all local requirements satisfied.
pub struct PreparedLocalInline<'a>(pub(crate) PreparedRendering<'a>);
/// An Inline fragment whose only remaining requirements are explicit guests.
/// The immutable document owns every closure passed to the rendering callback.
pub struct PreparedInlineWithForeign<'a>(pub(crate) PreparedRendering<'a>);
/// An Article whose remaining requirements are explicit guest slots.
/// Guest meaning validation remains the selected adapter's responsibility,
/// including content hidden by language selection.
pub struct PreparedArticleWithForeign<'a>(pub(crate) PreparedRendering<'a>);
pub(crate) fn supports_foreign(kind: EmbedKind) -> bool {
    matches!(
        kind,
        EmbedKind::Sentence
            | EmbedKind::SentenceInline
            | EmbedKind::InlineMath
            | EmbedKind::DisplayMath
            | EmbedKind::Code
            | EmbedKind::CircuitFigure
    )
}
pub(crate) struct PreparedRendering<'a> {
    pub(crate) document: &'a DocumentSyntax,
    pub(crate) options: &'a RenderOptions,
    pub(crate) selections: Vec<Option<VariantRef>>,
    pub(crate) identity: Digest,
}
/// Prepare the local-only subset. Any external requirement is returned intact,
/// never resolved by a placeholder. This is not the full suite prepare API.
pub fn prepare_local<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedLocalArticle<'a>, LocalPreparationError<'a, C::Error>> {
    let plan = prepare::inspect(document, registry, codec, budget).map_err(|e| match e {
        PreparationError::Stopped(s) => LocalPreparationError::Stopped(s),
        e => LocalPreparationError::Input(e),
    })?;
    if !plan.requirements.is_empty() {
        return Err(LocalPreparationError::NeedsResolution(plan));
    }
    prepare_rendering(document, options, plan.document_digest, budget).map(PreparedLocalArticle)
}
/// Prepare a Sentence without importing the surrounding Article's namespace.
/// Links, assets and foreign guests remain explicit resolution requirements.
pub fn prepare_local_sentence<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedLocalSentence<'a>, LocalPreparationError<'a, C::Error>> {
    let plan =
        prepare::inspect_sentence(document, registry, codec, budget).map_err(|e| match e {
            PreparationError::Stopped(s) => LocalPreparationError::Stopped(s),
            e => LocalPreparationError::Input(e),
        })?;
    if !plan.requirements.is_empty() {
        return Err(LocalPreparationError::NeedsResolution(plan));
    }
    prepare_rendering(document, options, plan.document_digest, budget).map(PreparedLocalSentence)
}
/// Prepare an Inline fragment in its own namespace. External dependencies are
/// returned as a resolution plan; the host supplies document-level composition.
pub fn prepare_local_inline<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedLocalInline<'a>, LocalPreparationError<'a, C::Error>> {
    let plan = prepare::inspect_inline(document, registry, codec, budget).map_err(|e| match e {
        PreparationError::Stopped(s) => LocalPreparationError::Stopped(s),
        e => LocalPreparationError::Input(e),
    })?;
    if !plan.requirements.is_empty() {
        return Err(LocalPreparationError::NeedsResolution(plan));
    }
    prepare_rendering(document, options, plan.document_digest, budget).map(PreparedLocalInline)
}

pub fn prepare_inline_with_foreign<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedInlineWithForeign<'a>, LocalPreparationError<'a, C::Error>> {
    let plan = prepare::inspect_inline(document, registry, codec, budget).map_err(|e| match e {
        PreparationError::Stopped(s) => LocalPreparationError::Stopped(s),
        e => LocalPreparationError::Input(e),
    })?;
    for requirement in &plan.requirements {
        budget.charge(Resource::Work, 1)?;
        if !matches!(requirement, prepare::DocRequirement::Foreign { .. }) {
            return Err(LocalPreparationError::NeedsResolution(plan));
        }
    }
    prepare_rendering(document, options, plan.document_digest, budget)
        .map(PreparedInlineWithForeign)
}

/// Inspect the complete Article before invoking any rendering adapter.
/// Sentence and InlineMath slots accept phrasing output; DisplayMath, Code and
/// CircuitFigure accept block output. Assets and page links require resolution.
pub fn prepare_article_with_foreign<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedArticleWithForeign<'a>, LocalPreparationError<'a, C::Error>> {
    let plan = prepare::inspect(document, registry, codec, budget).map_err(|e| match e {
        PreparationError::Stopped(s) => LocalPreparationError::Stopped(s),
        e => LocalPreparationError::Input(e),
    })?;
    for requirement in &plan.requirements {
        budget.charge(Resource::Work, 1)?;
        if !matches!(
            requirement,
            prepare::DocRequirement::Foreign { kind, .. } if supports_foreign(*kind)
        ) {
            return Err(LocalPreparationError::NeedsResolution(plan));
        }
    }
    prepare_rendering(document, options, plan.document_digest, budget)
        .map(PreparedArticleWithForeign)
}

pub(crate) fn prepare_rendering<'a, E>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
    identity: Digest,
    budget: &mut Budget,
) -> Result<PreparedRendering<'a>, LocalPreparationError<'a, E>> {
    if let ParallelMode::Single {
        language,
        fallbacks,
    } = &options.parallel
    {
        for (i, lang) in core::iter::once(language).chain(fallbacks).enumerate() {
            if !nepl3_core::lexical::language::well_formed(lang, budget)? {
                return Err(LocalPreparationError::Language);
            }
            for prior in core::iter::once(language).chain(fallbacks).take(i) {
                budget.charge(Resource::Work, (prior.len() + lang.len()) as u64)?;
                if prior.eq_ignore_ascii_case(lang) {
                    return Err(LocalPreparationError::Language);
                }
            }
        }
    }
    let mut selections = Vec::new();
    for (index, node) in document.value.nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if let DocKind::List {
            kind: ListKind::Ordered { start },
            ..
        } = node.kind
            && start > i32::MAX as u64
        {
            return Err(LocalPreparationError::ListStart {
                node: index as u64,
                start,
            });
        }
        let selected = if let DocKind::Parallel { variants } = &node.kind
            && let ParallelMode::Single {
                language,
                fallbacks,
            } = &options.parallel
        {
            let mut found = None;
            'languages: for wanted in core::iter::once(language).chain(fallbacks) {
                for variant in variants {
                    budget.charge(Resource::Work, 1)?;
                    if let DocKind::Variant { language, .. } =
                        &document.value.nodes[variant.0 as usize].kind
                    {
                        budget.charge(Resource::Work, (wanted.len() + language.len()) as u64)?;
                        if wanted.eq_ignore_ascii_case(language) {
                            found = Some(*variant);
                            break 'languages;
                        }
                    }
                }
            }
            Some(found.ok_or(LocalPreparationError::MissingVariant { node: index as u64 })?)
        } else {
            None
        };
        budget.charge(
            Resource::AllocationUnits,
            2 * core::mem::size_of::<Option<VariantRef>>() as u64,
        )?;
        selections.push(selected);
    }
    Ok(PreparedRendering {
        document,
        options,
        selections,
        identity,
    })
}
