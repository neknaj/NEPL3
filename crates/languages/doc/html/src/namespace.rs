//! HTML composition of an explicitly resolved Doc namespace.
//! Pending parts carry structural checks; complete output requires final validation.
use crate::{LocalPreparationError, RenderError, RenderOptions, build, prepare};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::Digest,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::labels::namespace::{CheckedNamespace, MemberId};
use nepl3_markup::html::*;

pub struct PreparedNamespace<'a> {
    members: Vec<prepare::PreparedRendering<'a>>,
}
/// Namespace with explicit Sentence, Math, Code and Circuit operations selected by a host.
pub struct PreparedForeignNamespace<'a>(PreparedNamespace<'a>);
/// Structurally checked member and per-occurrence guest placements. Namespace
/// identities remain pending until the composing renderer validates its output.
pub struct PendingForeignPart {
    pub part: PendingPart,
    pub foreign: Vec<crate::ForeignPlacement>,
}
#[derive(Debug)]
pub enum ForeignPartError<E> {
    Member(MemberId),
    Render(crate::ForeignRenderError<E>),
}
/// Render one selected occurrence, invoking only its explicit guest operations.
/// Guest HTML must satisfy its Doc slot: Sentence/InlineMath require Phrasing;
/// DisplayMath/Code/CircuitFigure require Block. Doc references to other namespace
/// members remain pending in the returned part.
pub fn render_part_with_foreign<E>(
    prepared: &PreparedForeignNamespace<'_>,
    member: MemberId,
    adapter: &mut impl FnMut(
        &nepl3_doc_core::model::DocEmbed,
        nepl3_doc_core::model::EmbedRef,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<PendingForeignPart, ForeignPartError<E>> {
    budget
        .poll()
        .map_err(|reason| ForeignPartError::Render(reason.into()))?;
    let input = usize::try_from(member.0)
        .ok()
        .and_then(|index| prepared.0.members.get(index))
        .ok_or(ForeignPartError::Member(member))?;
    let rendered = build::namespace_member_with_foreign(input, adapter, budget)
        .map_err(ForeignPartError::Render)?;
    check_part(
        &rendered.fragment.markup.fragment,
        rendered.fragment.markup.slot,
        &rendered.fragment.markup.policy,
        budget,
    )
    .map_err(|error| ForeignPartError::Render(error.into()))?;
    Ok(PendingForeignPart {
        part: PendingPart {
            member,
            fragment: rendered.fragment,
        },
        foreign: rendered.foreign,
    })
}
/// Structurally checked markup with namespace references still pending.
/// A composing renderer must validate the final complete HTML. This value is
/// not accepted by the HTML serializers and carries no complete-output proof.
pub struct PendingPart {
    member: MemberId,
    fragment: crate::RenderedFragment,
}
impl PendingPart {
    pub fn into_parts(self) -> (MemberId, Digest, HtmlRequest, Vec<crate::ElementOrigin>) {
        (
            self.member,
            self.fragment.document_digest,
            self.fragment.markup,
            self.fragment.origins,
        )
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum PartError {
    Member(MemberId),
    Render(RenderError),
}
/// Render one exact selected occurrence for insertion by a composing host.
/// Its local identities remain pending until the destination completes the
/// namespace; malformed markup is rejected before it crosses this boundary.
pub fn render_part(
    prepared: &PreparedNamespace<'_>,
    member: MemberId,
    budget: &mut Budget,
) -> Result<PendingPart, PartError> {
    budget
        .poll()
        .map_err(|reason| PartError::Render(RenderError::Stopped(reason)))?;
    let input = usize::try_from(member.0)
        .ok()
        .and_then(|index| prepared.members.get(index))
        .ok_or(PartError::Member(member))?;
    let fragment = build::namespace_member(input, budget).map_err(PartError::Render)?;
    check_part(
        &fragment.markup.fragment,
        fragment.markup.slot,
        &fragment.markup.policy,
        budget,
    )
    .map_err(|error| PartError::Render(error.into()))?;
    Ok(PendingPart { member, fragment })
}
/// Prepare every selected member. Links, assets and foreign operations retain
/// explicit resolution requirements; this entry handles the local Doc subset.
pub fn prepare<'a, C: FoundationValueCodec>(
    namespace: &CheckedNamespace<'_, 'a>,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedNamespace<'a>, LocalPreparationError<'a, C::Error>> {
    prepare_members(namespace, options, registry, codec, budget, false)
}
/// Prepare a namespace whose outstanding requirements are supported guest slots.
/// Generic guests, page links and assets require their own explicit resolution.
/// Preparation validates all member sources together and invokes no guests.
pub fn prepare_with_foreign<'a, C: FoundationValueCodec>(
    namespace: &CheckedNamespace<'_, 'a>,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedForeignNamespace<'a>, LocalPreparationError<'a, C::Error>> {
    prepare_members(namespace, options, registry, codec, budget, true).map(PreparedForeignNamespace)
}
fn prepare_members<'a, C: FoundationValueCodec>(
    namespace: &CheckedNamespace<'_, 'a>,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    foreign: bool,
) -> Result<PreparedNamespace<'a>, LocalPreparationError<'a, C::Error>> {
    let plans = nepl3_doc_core::prepare::inspect_namespace(namespace, registry, codec, budget)
        .map_err(|error| match error {
            nepl3_doc_core::prepare::PreparationError::Stopped(reason) => {
                LocalPreparationError::Stopped(reason)
            }
            error => LocalPreparationError::Input(error),
        })?;
    let mut members = Vec::new();
    for (document, plan) in namespace.documents().zip(plans) {
        for requirement in &plan.requirements {
            budget.charge(Resource::Work, 1)?;
            if !foreign
                || !matches!(
                    requirement,
                    nepl3_doc_core::prepare::DocRequirement::Foreign { kind, .. }
                        if prepare::supports_foreign(*kind)
                )
            {
                return Err(LocalPreparationError::NeedsResolution(plan));
            }
        }
        let member = prepare::prepare_rendering(document, options, plan.document_digest, budget)?;
        build::push(&mut members, member, budget)?;
    }
    Ok(PreparedNamespace { members })
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origin {
    pub member: MemberId,
    pub node: u64,
    pub element: u64,
}
pub struct RenderedNamespace {
    pub markup: HtmlRequest,
    /// Same order as the explicitly selected namespace occurrences.
    pub documents: Vec<Digest>,
    /// Generated namespace wrapper has no fabricated Doc owner.
    pub origins: Vec<Origin>,
}
/// One guest occurrence in the complete namespace's HTML arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceForeignPlacement {
    pub member: MemberId,
    pub embed: nepl3_doc_core::model::EmbedRef,
    pub first_element: u64,
    pub elements: u64,
}
pub struct RenderedForeignNamespace {
    pub namespace: RenderedNamespace,
    pub foreign: Vec<NamespaceForeignPlacement>,
}
#[derive(Debug)]
pub enum ForeignNamespaceError<E> {
    Namespace(Error),
    Foreign(E),
}
impl<E> From<Error> for ForeignNamespaceError<E> {
    fn from(error: Error) -> Self {
        Self::Namespace(error)
    }
}
impl<E> From<RenderError> for ForeignNamespaceError<E> {
    fn from(error: RenderError) -> Self {
        Self::Namespace(error.into())
    }
}
impl<E> From<StopReason> for ForeignNamespaceError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Namespace(reason.into())
    }
}
impl<E> From<HtmlError> for ForeignNamespaceError<E> {
    fn from(error: HtmlError) -> Self {
        Self::Namespace(error.into())
    }
}
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Render(RenderError),
    /// HTML element in the assembled namespace; not a Doc arena index.
    OutputDepth {
        element: u64,
    },
}
impl From<RenderError> for Error {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Render(RenderError::Stopped(reason))
    }
}
impl From<HtmlError> for Error {
    fn from(error: HtmlError) -> Self {
        Self::Render(error.into())
    }
}
pub fn render(
    prepared: &PreparedNamespace<'_>,
    budget: &mut Budget,
) -> Result<RenderedNamespace, Error> {
    match render_members(
        prepared,
        &mut |_, member, _, b| {
            build::namespace_member(member, b)
                .map_err(ForeignNamespaceError::<core::convert::Infallible>::from)
        },
        budget,
    ) {
        Ok(output) => Ok(output),
        Err(ForeignNamespaceError::Namespace(error)) => Err(error),
        Err(ForeignNamespaceError::Foreign(never)) => match never {},
    }
}
/// Render all selected occurrences, then validate their combined namespace.
/// The callback receives the exact member owner of each immutable guest. Local
/// Doc references can cross members; guest markup obeys the same slot checks
/// as `render_part_with_foreign`. No guest is retried on failure. Doc origins
/// and guest placements use element indices in the final combined arena.
pub fn render_with_foreign<E>(
    prepared: &PreparedForeignNamespace<'_>,
    adapter: &mut impl FnMut(
        MemberId,
        &nepl3_doc_core::model::DocEmbed,
        nepl3_doc_core::model::EmbedRef,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<RenderedForeignNamespace, ForeignNamespaceError<E>> {
    let mut foreign = Vec::new();
    let namespace = render_members(
        &prepared.0,
        &mut |member, input, offset, b| {
            let output = build::namespace_member_with_foreign(
                input,
                &mut |guest, embed, b| adapter(member, guest, embed, b),
                b,
            )
            .map_err(|error| match error {
                crate::ForeignRenderError::Render(error) => ForeignNamespaceError::from(error),
                crate::ForeignRenderError::Foreign(error) => ForeignNamespaceError::Foreign(error),
            })?;
            for placement in output.foreign {
                b.charge(Resource::Work, 1)?;
                let first_element = offset
                    .checked_add(placement.first_element)
                    .ok_or(RenderError::InternalShape)?;
                build::push(
                    &mut foreign,
                    NamespaceForeignPlacement {
                        member,
                        embed: placement.embed,
                        first_element,
                        elements: placement.elements,
                    },
                    b,
                )?;
            }
            Ok(output.fragment)
        },
        budget,
    )?;
    Ok(RenderedForeignNamespace { namespace, foreign })
}
fn render_members<E>(
    prepared: &PreparedNamespace<'_>,
    render: &mut impl FnMut(
        MemberId,
        &prepare::PreparedRendering<'_>,
        u64,
        &mut Budget,
    ) -> Result<crate::RenderedFragment, ForeignNamespaceError<E>>,
    budget: &mut Budget,
) -> Result<RenderedNamespace, ForeignNamespaceError<E>> {
    budget.poll()?;
    let mut nodes = Vec::new();
    budget.charge(Resource::Nodes, 1)?;
    build::push(
        &mut nodes,
        HtmlNode::Element {
            tag: HtmlTag::Div,
            attributes: vec![],
            children: vec![],
        },
        budget,
    )?;
    let mut classes = Vec::new();
    let mut documents = Vec::new();
    let mut origins = Vec::new();
    for (number, member) in prepared.members.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        // Include the enclosing namespace node in the caller's Depth budget.
        let offset = nodes.len() as u64;
        let output = budget.with_depth(|b| render(MemberId(number as u64), member, offset, b))?;
        let root = offset
            .checked_add(output.markup.fragment.root)
            .ok_or(RenderError::InternalShape)?;
        for mut node in output.markup.fragment.nodes {
            match &mut node {
                HtmlNode::Element { children, .. } | HtmlNode::MathElement { children, .. } => {
                    for child in children {
                        budget.charge(Resource::Work, 1)?;
                        *child = offset
                            .checked_add(*child)
                            .ok_or(RenderError::InternalShape)?;
                    }
                }
                HtmlNode::Text { .. } => {}
            }
            build::push(&mut nodes, node, budget)?;
        }
        let Some(HtmlNode::Element { children, .. }) = nodes.first_mut() else {
            return Err(RenderError::InternalShape.into());
        };
        build::push(children, root, budget)?;
        for class in output.markup.policy.classes {
            let mut present = false;
            for prior in &classes {
                let prior: &alloc::string::String = prior;
                budget.charge(Resource::Work, prior.len().min(class.len()) as u64 + 1)?;
                if prior == &class {
                    present = true;
                    break;
                }
            }
            if !present {
                build::push(&mut classes, class, budget)?;
            }
        }
        for origin in output.origins {
            budget.charge(Resource::Work, 1)?;
            let element = offset
                .checked_add(origin.element)
                .ok_or(RenderError::InternalShape)?;
            build::push(
                &mut origins,
                Origin {
                    member: MemberId(number as u64),
                    node: origin.node,
                    element,
                },
                budget,
            )?;
        }
        build::push(&mut documents, output.document_digest, budget)?;
    }
    let markup = HtmlRequest {
        fragment: HtmlFragment { root: 0, nodes },
        slot: HtmlSlot::Block,
        policy: HtmlPolicy { classes },
    };
    validate(&markup.fragment, markup.slot, &markup.policy, budget)?;
    // Apply the Doc backend envelope to the complete assembled output.
    let mut pending = Vec::new();
    build::push(&mut pending, (markup.fragment.root, 1u64), budget)?;
    while let Some((index, depth)) = pending.pop() {
        budget.charge(Resource::Work, 1)?;
        if depth > 256 {
            return Err(Error::OutputDepth { element: index }.into());
        }
        match &markup.fragment.nodes[index as usize] {
            HtmlNode::Element { children, .. } | HtmlNode::MathElement { children, .. } => {
                for child in children {
                    build::push(&mut pending, (*child, depth + 1), budget)?;
                }
            }
            HtmlNode::Text { .. } => {}
        }
    }
    Ok(RenderedNamespace {
        markup,
        documents,
        origins,
    })
}
