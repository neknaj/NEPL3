//! Host-selected Doc occurrences in one independent Sentence display.
use super::*;
use nepl3_core::value::SchemaRef;
use nepl3_doc_core::labels::namespace as labels;
use nepl3_sentence_core::{model::EmbedRef, syntax::SentenceSyntax};

pub struct Selected {
    pub embed: EmbedRef,
    pub document: Arc<DocumentSyntax>,
}
fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(item);
    Ok(())
}
pub fn collect<C: FoundationValueCodec>(
    sentence: &SentenceSyntax,
    surface: Option<&SchemaRef>,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Vec<Selected>, Error<C::Error>> {
    let Some(surface) = surface else {
        return Ok(Vec::new());
    };
    let selection = nepl3_suite::adapters::sentence::document_guests::collect(
        sentence, surface, registry, codec, b,
    )
    .map_err(|error| match error {
        nepl3_suite::adapters::sentence::document_guests::Error::Stopped(reason) => {
            Error::Stopped(reason)
        }
        error => Error::Guests(error),
    })?;
    let (unique, occurrences) = selection.into_parts();
    let mut documents = Vec::new();
    for document in unique {
        b.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<DocumentSyntax>() + 2 * core::mem::size_of::<usize>()) as u64,
        )?;
        push(&mut documents, Arc::new(document), b)?;
    }
    let mut selected = Vec::new();
    for occurrence in occurrences {
        let document = Arc::clone(&documents[occurrence.document.index()]);
        push(
            &mut selected,
            Selected {
                embed: occurrence.embed,
                document,
            },
            b,
        )?;
    }
    Ok(selected)
}

pub fn inspect<'a, C: FoundationValueCodec>(
    selected: &'a [Selected],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Vec<labels::Member<'a>>, Error<C::Error>> {
    let mut members = Vec::new();
    for item in selected {
        let member = labels::inspect(&item.document, registry, b, codec.source_admission())
            .map_err(|error| match error {
                labels::Error::Stopped(reason) => Error::Stopped(reason),
                labels::Error::Root(category) => Error::ExpectedRoot(category),
                labels::Error::Input(error) => preparation_error(
                    LocalPreparationError::Input(PreparationError::Label(error)),
                    &item.document,
                    registry,
                    codec,
                    b,
                ),
                _ => Error::Selection,
            })?;
        push(&mut members, member, b)?;
    }
    Ok(members)
}
pub fn member_refs<'a>(
    members: &'a [labels::Member<'a>],
    b: &mut Budget,
) -> Result<Vec<&'a labels::Member<'a>>, StopReason> {
    let mut refs = Vec::new();
    for member in members {
        push(&mut refs, member, b)?;
    }
    Ok(refs)
}
pub fn resolution_error<C: FoundationValueCodec>(
    error: labels::Error<'_>,
    selected: &[Selected],
    members: &[&labels::Member<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Error<C::Error> {
    match error {
        labels::Error::Stopped(reason) => Error::Stopped(reason),
        labels::Error::Unresolved { reference } => {
            let Some(owner) = selected.get(reference.member.0 as usize) else {
                return Error::Selection;
            };
            preparation_error(
                LocalPreparationError::Input(PreparationError::Label(LabelError::Unresolved {
                    reference: reference.site,
                })),
                &owner.document,
                registry,
                codec,
                b,
            )
        }
        labels::Error::Duplicate {
            definition,
            previous,
        } => {
            let diagnostic = match (labels::Error::Duplicate {
                definition,
                previous,
            })
            .diagnostic(members, registry, codec, b)
            {
                Ok(diagnostic) => diagnostic,
                Err(error) => return Error::LabelDiagnostic(error),
            };
            if let Err(reason) = b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Diagnostic>() as u64,
            ) {
                return Error::Stopped(reason);
            }
            let owner = |site: labels::NamespaceSite<'_>| {
                selected
                    .get(site.member.0 as usize)
                    .map(|owner| NamespaceOwner {
                        member: site.member,
                        node: site.site.node,
                        document: Arc::clone(&owner.document),
                    })
            };
            match (owner(definition), owner(previous)) {
                (Some(definition), Some(previous)) => Error::NamespaceDuplicate {
                    definition,
                    previous,
                    diagnostic: Box::new(diagnostic),
                },
                _ => Error::Selection,
            }
        }
        labels::Error::Input(_) | labels::Error::Root(_) | labels::Error::Order(_) => {
            Error::Selection
        }
    }
}
pub fn prepare<'a, C: FoundationValueCodec>(
    checked: &labels::CheckedNamespace<'_, 'a>,
    options: &'a nepl3_doc_html::RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<nepl3_doc_html::namespace::PreparedForeignNamespace<'a>, Error<C::Error>> {
    nepl3_doc_html::namespace::prepare_with_foreign(checked, options, registry, codec, b).map_err(
        |error| match error {
            LocalPreparationError::Stopped(reason) => Error::Stopped(reason),
            LocalPreparationError::NeedsResolution(plan) => Error::NeedsResolution(plan),
            LocalPreparationError::InvalidExternalUri { node } => {
                Error::InvalidExternalUri { node }
            }
            LocalPreparationError::Language => Error::LanguageOptions,
            LocalPreparationError::MissingVariant { node } => Error::MissingVariant { node },
            LocalPreparationError::ListStart { node, start } => Error::ListStart { node, start },
            LocalPreparationError::Input(PreparationError::Stopped(reason)) => {
                Error::Stopped(reason)
            }
            LocalPreparationError::Input(PreparationError::Boundary(error)) => {
                Error::Boundary(error)
            }
            // Label resolution has already succeeded for exactly these members.
            LocalPreparationError::Input(PreparationError::Label(_)) => Error::Selection,
        },
    )
}

/// Guest content is selected from the semantic owner, never inferred from HTML.
/// Math Expr is an explicit content-producing role if its later rendering succeeds.
pub fn check_content<C: FoundationValueCodec>(
    sentence: &SentenceSyntax,
    selected: &[Selected],
    math_surface: Option<&SchemaRef>,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<(), Error<C::Error>> {
    let shape = sentence
        .validate(registry, b, codec.source_admission())
        .map_err(|error| match error {
            nepl3_sentence_core::syntax::Error::Stopped(reason) => Error::Stopped(reason),
            nepl3_sentence_core::syntax::Error::Shape(error) => Error::Content(error),
            error => Error::Guests(
                nepl3_suite::adapters::sentence::document_guests::Error::Sentence(error),
            ),
        })?;
    let count = sentence.value.embeds.len();
    b.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(core::mem::size_of::<Option<&DocumentSyntax>>() as u64),
    )?;
    let mut contents = Vec::new();
    contents
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    contents.resize(count, None);
    for item in selected {
        b.charge(Resource::Work, 1)?;
        let slot = contents
            .get_mut(item.embed.0 as usize)
            .ok_or(Error::Selection)?;
        if slot.is_none() {
            *slot = Some(item.document.as_ref());
        }
    }
    shape
        .shape()
        .content_with_foreign(
            &mut |embed, b| {
                if let Some(document) = contents.get(embed.0 as usize).copied().flatten() {
                    return document
                        .value
                        .validate_shape(b)
                        .map(|shape| shape.annotation_content())
                        .map_err(|error| match error {
                            check::ShapeError::Stopped(reason) => Error::Stopped(reason),
                            error => Error::Structure(check::StructureError::Shape(error)),
                        })?
                        .ok_or(Error::Selection);
                }
                let guest = sentence
                    .value
                    .embeds
                    .entry(embed.0 as usize)
                    .ok_or(Error::Selection)?;
                let surface = math_surface.ok_or(Error::Selection)?;
                b.charge(Resource::Work, guest.syntax().category.len() as u64)?;
                if guest.syntax().schema.equal_with_budget(surface, b)?
                    && guest.syntax().category == "Expr"
                {
                    Ok(true)
                } else {
                    Err(Error::Selection)
                }
            },
            b,
        )
        .map_err(|error| match error {
            nepl3_sentence_core::check::ContentError::Sentence(
                nepl3_sentence_core::check::Error::Stopped(reason),
            ) => Error::Stopped(reason),
            nepl3_sentence_core::check::ContentError::Sentence(error) => Error::Content(error),
            nepl3_sentence_core::check::ContentError::Foreign(error) => error,
        })?;
    Ok(())
}
