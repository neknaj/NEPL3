//! Explicit Article namespace adoption, not visibility selection or rendering.
use super::Selection;
mod schedule;
use crate::doc::annotations::{SentenceAnnotationRenderer, document};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Diagnostic,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::Category,
    labels::{DocLabelId, LabelDiagnosticError, namespace as labels},
    model::DocumentSyntax,
    prepare::PreparationError,
};
use nepl3_doc_html::LocalPreparationError;
use nepl3_sentence_core::model::EmbedRef;
use nepl3_suite::adapters::sentence::document_guests::OwnerId;

/// Index into the exact Selection::sentences() sequence, not a Doc node or owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentenceOccurrenceId(pub usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Site {
    pub member: labels::MemberId,
    pub node: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reference {
    pub reference: Site,
    pub target: DocLabelId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberOrigin {
    Article,
    SentenceInline {
        sentence: SentenceOccurrenceId,
        owner: OwnerId,
        embed: EmbedRef,
        /// Position in that Sentence's directly selected Doc Inline sequence.
        inline_occurrence: usize,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureSite {
    pub site: Site,
    pub origin: MemberOrigin,
}
#[derive(Debug)]
pub struct LabelFailure {
    pub diagnostic: Diagnostic,
    pub primary: FailureSite,
    pub related: Option<FailureSite>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentContext {
    Owner(OwnerId),
    Member(MemberOrigin),
}
#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    AdoptionIndex {
        position: usize,
        index: SentenceOccurrenceId,
    },
    AdoptionOrder {
        position: usize,
        previous: SentenceOccurrenceId,
        current: SentenceOccurrenceId,
    },
    Document {
        context: DocumentContext,
        error: Box<document::Error<E>>,
    },
    Root(Category),
    Order(labels::OrderError),
    Label(Box<LabelFailure>),
    LabelDiagnostic(LabelDiagnosticError<E>),
    SentenceSelection {
        owner: OwnerId,
    },
    InternalShape,
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
struct Guest {
    origin: MemberOrigin,
    selected: document::namespace::Selected,
}
/// Operation-local access to retained guest owners. The checked namespace
/// separately exposes every member's exact immutable document.
#[derive(Clone, Copy)]
pub struct Members<'a> {
    guests: &'a [Guest],
}
impl Members<'_> {
    pub fn guest(
        &self,
        member: labels::MemberId,
    ) -> Option<(MemberOrigin, &document::namespace::Selected)> {
        self.guests
            .get(usize::try_from(member.0.checked_sub(1)?).ok()?)
            .map(|guest| (guest.origin, &guest.selected))
    }
}
#[derive(Debug)]
pub enum OperationError<E, F> {
    Resolve(Error<E>),
    Operation(F),
}

/// An owned summary for the exact retained inputs and explicit adoption list.
/// A later rendering operation must revalidate/re-resolve using its own Budget;
/// this is not a CheckedNamespace or permission to serialize arbitrary markup.
pub struct Resolution<'s, 'a> {
    input: &'s Selection<'a>,
    adopted: Vec<SentenceOccurrenceId>,
    guests: Vec<Guest>,
    definitions: Vec<Site>,
    references: Vec<Reference>,
    definition_order: Vec<labels::DefinitionIndex>,
    reference_order: Vec<labels::ReferenceIndex>,
}
impl<'s, 'a> Resolution<'s, 'a> {
    pub(crate) fn members(&self) -> Members<'_> {
        Members {
            guests: &self.guests,
        }
    }
    pub(crate) fn orders(&self) -> (&[labels::DefinitionIndex], &[labels::ReferenceIndex]) {
        (&self.definition_order, &self.reference_order)
    }
    /// Reinspect immutable member documents within the same owning host operation.
    /// The caller must first resolve owner selection with that operation's Budget.
    pub(crate) fn inspect_members<C: FoundationValueCodec>(
        &self,
        host: &mut SentenceAnnotationRenderer<'_, C>,
        b: &mut Budget,
    ) -> Result<Vec<labels::Member<'_>>, Error<C::Error>> {
        let mut members = Vec::new();
        push(
            &mut members,
            inspect(self.input.document(), MemberOrigin::Article, host, b)?,
            b,
        )?;
        for guest in &self.guests {
            push(
                &mut members,
                inspect(&guest.selected.document, guest.origin, host, b)?,
                b,
            )?;
        }
        Ok(members)
    }
    pub fn input(&self) -> &'s Selection<'a> {
        self.input
    }
    pub fn adopted(&self) -> &[SentenceOccurrenceId] {
        &self.adopted
    }
    pub fn definitions(&self) -> &[Site] {
        &self.definitions
    }
    pub fn references(&self) -> &[Reference] {
        &self.references
    }
    pub fn document(&self, member: labels::MemberId) -> Option<&DocumentSyntax> {
        if member.0 == 0 {
            Some(self.input.document())
        } else {
            self.guests
                .get(usize::try_from(member.0.checked_sub(1)?).ok()?)
                .map(|g| g.selected.document.as_ref())
        }
    }
    pub fn origin(&self, member: labels::MemberId) -> Option<MemberOrigin> {
        if member.0 == 0 {
            Some(MemberOrigin::Article)
        } else {
            self.guests
                .get(usize::try_from(member.0.checked_sub(1)?).ok()?)
                .map(|g| g.origin)
        }
    }
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
fn document_error<E>(
    error: document::Error<E>,
    context: DocumentContext,
    b: &mut Budget,
) -> Error<E> {
    match b.charge(
        Resource::AllocationUnits,
        core::mem::size_of_val(&error) as u64,
    ) {
        Ok(()) => Error::Document {
            context,
            error: Box::new(error),
        },
        Err(reason) => Error::Stopped(reason),
    }
}
fn inspect<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    origin: MemberOrigin,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<labels::Member<'a>, Error<C::Error>> {
    labels::inspect(document, host.registry, b, host.codec.source_admission()).map_err(|error| {
        match error {
            labels::Error::Stopped(reason) => Error::Stopped(reason),
            labels::Error::Root(category) => Error::Root(category),
            labels::Error::Input(error) => {
                let error = document::preparation_error(
                    LocalPreparationError::Input(PreparationError::Label(error)),
                    document,
                    host.registry,
                    host.codec,
                    b,
                );
                document_error(error, DocumentContext::Member(origin), b)
            }
            labels::Error::Order(error) => Error::Order(error),
            _ => Error::InternalShape,
        }
    })
}
struct Range {
    occurrence: usize,
    start: usize,
    end: usize,
}
/// Adopt exactly the supplied Sentence occurrences in strictly increasing
/// structural order. Unadopted guest definitions never enter the namespace.
/// Repeated owners at different occurrences remain separate members, so their
/// duplicate declarations are rejected. No Math annotation scope is imported.
pub fn resolve<'s, 'a, C: FoundationValueCodec>(
    input: &'s Selection<'a>,
    adoption: &[SentenceOccurrenceId],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<Resolution<'s, 'a>, Error<C::Error>> {
    b.poll()?;
    let result = resolve_inner(input, adoption, host, b, |_, _, _, _| {
        Ok::<_, core::convert::Infallible>(())
    })
    .map(|(resolved, _)| resolved);
    b.poll()?;
    result
}
/// Re-resolve the exact inputs and run one scoped operation while the checked
/// namespace and retained member owners are alive. Temporary proof storage
/// cannot escape through the callback's output or error. A stopped Budget takes
/// precedence over either an operation result or error.
pub fn resolve_with<'s, 'a, C: FoundationValueCodec, R, F>(
    input: &'s Selection<'a>,
    adoption: &[SentenceOccurrenceId],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
    operation: impl FnOnce(
        &labels::CheckedNamespace<'_, '_>,
        Members<'_>,
        &mut SentenceAnnotationRenderer<'_, C>,
        &mut Budget,
    ) -> Result<R, F>,
) -> Result<(Resolution<'s, 'a>, R), OperationError<C::Error, F>> {
    b.poll()
        .map_err(|reason| OperationError::Resolve(Error::Stopped(reason)))?;
    let result = resolve_inner(input, adoption, host, b, operation);
    b.poll()
        .map_err(|reason| OperationError::Resolve(Error::Stopped(reason)))?;
    let (resolution, output) = result.map_err(OperationError::Resolve)?;
    Ok((resolution, output.map_err(OperationError::Operation)?))
}
type OperationResult<'s, 'a, R, F> = (Resolution<'s, 'a>, Result<R, F>);
fn resolve_inner<'s, 'a, C: FoundationValueCodec, R, F>(
    input: &'s Selection<'a>,
    adoption: &[SentenceOccurrenceId],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
    operation: impl FnOnce(
        &labels::CheckedNamespace<'_, '_>,
        Members<'_>,
        &mut SentenceAnnotationRenderer<'_, C>,
        &mut Budget,
    ) -> Result<R, F>,
) -> Result<OperationResult<'s, 'a, R, F>, Error<C::Error>> {
    let mut depths = Vec::new();
    for _ in input.owners() {
        push(&mut depths, 0_u64, b)?;
    }
    let mut adopted = Vec::new();
    let mut previous: Option<SentenceOccurrenceId> = None;
    for (position, index) in adoption.iter().copied().enumerate() {
        b.charge(Resource::Work, 1)?;
        let sentence = input
            .sentences()
            .get(index.0)
            .ok_or(Error::AdoptionIndex { position, index })?;
        if let Some(previous) = previous
            && previous.0 >= index.0
        {
            return Err(Error::AdoptionOrder {
                position,
                previous,
                current: index,
            });
        }
        let occurrence = input
            .structure()
            .get(sentence.occurrence)
            .ok_or(Error::InternalShape)?;
        let depth = depths
            .get_mut(sentence.owner.0)
            .ok_or(Error::InternalShape)?;
        *depth = (*depth).max(occurrence.depth);
        push(&mut adopted, index, b)?;
        previous = Some(index);
    }
    let root = inspect(input.document(), MemberOrigin::Article, host, b)?;
    let base = b.current_depth();
    let mut local = Vec::new();
    for (number, (owner, depth)) in input.owners().iter().zip(depths).enumerate() {
        if depth == 0 {
            push(&mut local, Vec::new(), b)?;
            continue;
        }
        let closure = input
            .document()
            .value
            .embeds
            .get(owner.embed().0 as usize)
            .ok_or(Error::InternalShape)?;
        b.charge(
            Resource::Work,
            (closure.closure.syntax().schema.package.len()
                + host.surface.package.len()
                + closure.closure.syntax().category.len()) as u64
                + 64,
        )?;
        if &closure.closure.syntax().schema != host.surface
            || closure.closure.syntax().category != "Sentence"
        {
            return Err(Error::SentenceSelection {
                owner: OwnerId(number),
            });
        }
        let depth = base
            .checked_add(depth)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        let selected = b
            .with_depth_at_least(depth, |b| {
                let selected = document::namespace::collect(
                    owner.syntax(),
                    host.doc_surface,
                    host.registry,
                    host.codec,
                    b,
                )?;
                document::namespace::check_content(
                    owner.syntax(),
                    &selected,
                    host.math_surface,
                    host.registry,
                    host.codec,
                    b,
                )?;
                Ok::<_, document::Error<C::Error>>(selected)
            })
            .map_err(|error| document_error(error, DocumentContext::Owner(OwnerId(number)), b))?;
        push(&mut local, selected, b)?;
    }
    let mut guests = Vec::new();
    let mut ranges = Vec::new();
    for index in &adopted {
        let sentence = &input.sentences()[index.0];
        let start = guests.len();
        for (inline_occurrence, selected) in local
            .get(sentence.owner.0)
            .ok_or(Error::InternalShape)?
            .iter()
            .enumerate()
        {
            push(
                &mut guests,
                Guest {
                    origin: MemberOrigin::SentenceInline {
                        sentence: *index,
                        owner: sentence.owner,
                        embed: selected.embed,
                        inline_occurrence,
                    },
                    selected: document::namespace::Selected {
                        embed: selected.embed,
                        document: std::sync::Arc::clone(&selected.document),
                    },
                },
                b,
            )?;
        }
        push(
            &mut ranges,
            Range {
                occurrence: sentence.occurrence,
                start,
                end: guests.len(),
            },
            b,
        )?;
    }
    let mut members = Vec::new();
    push(&mut members, root, b)?;
    for guest in &guests {
        let member = inspect(&guest.selected.document, guest.origin, host, b)?;
        push(&mut members, member, b)?;
    }
    let schedule::Order {
        definitions,
        references,
    } = schedule::build::<C::Error>(input, &members, &ranges, b)?;
    let refs = document::namespace::member_refs(&members, b)?;
    let resolved =
        labels::resolve_interleaved(&refs, &definitions, &references, b).map_err(|error| {
            match error {
                labels::Error::Stopped(reason) => Error::Stopped(reason),
                labels::Error::Order(error) => Error::Order(error),
                error @ (labels::Error::Duplicate { .. } | labels::Error::Unresolved { .. }) => {
                    let (primary, related) = match &error {
                        labels::Error::Duplicate {
                            definition,
                            previous,
                        } => (*definition, Some(*previous)),
                        labels::Error::Unresolved { reference } => (*reference, None),
                        _ => return Error::InternalShape,
                    };
                    let owner = |site: labels::NamespaceSite<'_>| -> Option<FailureSite> {
                        let origin = if site.member.0 == 0 {
                            MemberOrigin::Article
                        } else {
                            guests
                                .get(usize::try_from(site.member.0.checked_sub(1)?).ok()?)?
                                .origin
                        };
                        Some(FailureSite {
                            site: Site {
                                member: site.member,
                                node: site.site.node,
                            },
                            origin,
                        })
                    };
                    let Some(primary) = owner(primary) else {
                        return Error::InternalShape;
                    };
                    let related = match related {
                        Some(site) => match owner(site) {
                            Some(site) => Some(site),
                            None => return Error::InternalShape,
                        },
                        None => None,
                    };
                    match error.diagnostic(&refs, host.registry, host.codec, b) {
                        Ok(diagnostic) => match b.charge(
                            Resource::AllocationUnits,
                            core::mem::size_of::<LabelFailure>() as u64,
                        ) {
                            Ok(()) => Error::Label(Box::new(LabelFailure {
                                diagnostic,
                                primary,
                                related,
                            })),
                            Err(reason) => Error::Stopped(reason),
                        },
                        Err(error) => Error::LabelDiagnostic(error),
                    }
                }
                _ => Error::InternalShape,
            }
        })?;
    let mut definition_sites = Vec::new();
    for site in resolved.definitions() {
        push(
            &mut definition_sites,
            Site {
                member: site.member,
                node: site.site.node,
            },
            b,
        )?;
    }
    let mut reference_sites = Vec::new();
    for reference in resolved.references() {
        push(
            &mut reference_sites,
            Reference {
                reference: Site {
                    member: reference.reference.member,
                    node: reference.reference.site.node,
                },
                target: reference.target,
            },
            b,
        )?;
    }
    let output = operation(&resolved, Members { guests: &guests }, host, b);
    Ok((
        Resolution {
            input,
            adopted,
            guests,
            definitions: definition_sites,
            references: reference_sites,
            definition_order: definitions,
            reference_order: references,
        },
        output,
    ))
}
