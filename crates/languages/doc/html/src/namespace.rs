//! Complete HTML composition of an explicitly resolved Doc namespace.
//! Intermediate member markup remains private until whole-output validation.
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
    let plans = nepl3_doc_core::prepare::inspect_namespace(namespace, registry, codec, budget)
        .map_err(|error| match error {
            nepl3_doc_core::prepare::PreparationError::Stopped(reason) => {
                LocalPreparationError::Stopped(reason)
            }
            error => LocalPreparationError::Input(error),
        })?;
    let mut members = Vec::new();
    for (document, plan) in namespace.documents().zip(plans) {
        if !plan.requirements.is_empty() {
            return Err(LocalPreparationError::NeedsResolution(plan));
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
        let output = budget.with_depth(|b| build::namespace_member(member, b))?;
        let offset = nodes.len() as u64;
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
            return Err(Error::OutputDepth { element: index });
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
