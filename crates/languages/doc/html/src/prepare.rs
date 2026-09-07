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
