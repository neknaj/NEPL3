//! Discover the external inputs required by an Article. This is a preparation
//! plan, never a PreparedArticle or evidence that a requested asset exists.
use crate::{
    labels,
    model::*,
    portable::{self, PortableError},
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::Digest,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

pub const DOCUMENT_DOMAIN: &[u8] = b"NEPL3.Doc.Prepare.Document.v1\0";
pub const GUEST_DOMAIN: &[u8] = b"NEPL3.Doc.Prepare.Guest.v1\0";
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocRequirement {
    Link {
        node: u64,
        target: LinkTarget,
    },
    Asset {
        node: u64,
        asset: AssetRef,
    },
    Foreign {
        embed: EmbedRef,
        kind: EmbedKind,
        guest_digest: Digest,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocPreparationPlan {
    pub document_digest: Digest,
    pub requirements: Vec<DocRequirement>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum PreparationError<'a, E> {
    Stopped(StopReason),
    Boundary(PortableError<E>),
    Label(labels::LabelError<'a>),
}
impl<E> From<StopReason> for PreparationError<'_, E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<PortableError<E>> for PreparationError<'_, E> {
    fn from(e: PortableError<E>) -> Self {
        match e {
            PortableError::Stopped(s) => Self::Stopped(s),
            e => Self::Boundary(e),
        }
    }
}
impl<'a, E> From<labels::LabelError<'a>> for PreparationError<'a, E> {
    fn from(e: labels::LabelError<'a>) -> Self {
        match e {
            labels::LabelError::Stopped(s) => Self::Stopped(s),
            e => Self::Label(e),
        }
    }
}
fn clone_text(s: &str, b: &mut Budget) -> Result<alloc::string::String, StopReason> {
    b.charge(Resource::Work, s.len() as u64)?;
    b.charge(Resource::AllocationUnits, s.len() as u64)?;
    Ok(s.into())
}
fn target(v: &LinkTarget, b: &mut Budget) -> Result<LinkTarget, StopReason> {
    let option = |v: &Option<alloc::string::String>, b: &mut Budget| {
        v.as_ref().map(|s| clone_text(s, b)).transpose()
    };
    Ok(match v {
        LinkTarget::Page { page, fragment } => LinkTarget::Page {
            page: clone_text(page, b)?,
            fragment: option(fragment, b)?,
        },
        LinkTarget::Relative { path, fragment } => LinkTarget::Relative {
            path: clone_text(path, b)?,
            fragment: option(fragment, b)?,
        },
    })
}
fn boundary<'a, E: FoundationCodecError>(e: E) -> PreparationError<'a, E> {
    match e.stop_reason() {
        Some(s) => PreparationError::Stopped(s),
        None => PreparationError::Boundary(PortableError::Foundation(e)),
    }
}
fn push(
    v: &mut Vec<DocRequirement>,
    item: DocRequirement,
    b: &mut Budget,
) -> Result<(), StopReason> {
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<DocRequirement>() as u64,
    )?;
    v.push(item);
    Ok(())
}
/// Validate source structure and Article-local labels, then enumerate distinct
/// semantic node requirements in arena order, followed by embeds in owner order.
/// All Parallel variants are included for whole-Article preparation; discovery
/// does not select a display language or execute a guest operation.
pub fn inspect<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocPreparationPlan, PreparationError<'a, C::Error>> {
    let checked = labels::check(document, registry, b, c.source_admission())?;
    let value = portable::to_value(document, registry, c, b)?;
    let document_digest = c
        .canonical_value_digest(DOCUMENT_DOMAIN, &value, b)
        .map_err(boundary)?;
    Ok(DocPreparationPlan {
        document_digest,
        requirements: requirements(&checked, &value, registry, c, b)?,
    })
}

/// Discover requirements for a standalone Sentence, retaining every language
/// variant and foreign closure. This checks only Sentence-local labels, never
/// resolves external inputs or proves the Sentence ready for rendering.
pub fn inspect_sentence<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocPreparationPlan, PreparationError<'a, C::Error>> {
    let checked = labels::check_sentence(document, registry, b, c.source_admission())?;
    let value = portable::to_value(document, registry, c, b)?;
    let document_digest = c
        .canonical_value_digest(DOCUMENT_DOMAIN, &value, b)
        .map_err(boundary)?;
    Ok(DocPreparationPlan {
        document_digest,
        requirements: discover(checked.document(), &value, registry, c, b)?,
    })
}

/// Discover an Inline fragment's dependencies after validating its own labels.
/// The returned plan retains unresolved links, assets and foreign closures.
pub fn inspect_inline<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocPreparationPlan, PreparationError<'a, C::Error>> {
    let checked = labels::check_inline(document, registry, b, c.source_admission())?;
    let value = portable::to_value(document, registry, c, b)?;
    let document_digest = c
        .canonical_value_digest(DOCUMENT_DOMAIN, &value, b)
        .map_err(boundary)?;
    Ok(DocPreparationPlan {
        document_digest,
        requirements: discover(checked.document(), &value, registry, c, b)?,
    })
}

/// Internal discovery from an Article-local proof. The caller separately owns
/// the canonical boundary value and its digest; external plans are never proofs.
pub(crate) fn requirements<'a, C: FoundationValueCodec>(
    checked: &labels::CheckedLabels<'a>,
    encoded: &nepl3_core::value::NdfValue,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<DocRequirement>, PreparationError<'a, C::Error>> {
    discover(checked.document(), encoded, registry, c, b)
}
/// Discover all members of one resolved namespace in its exact occurrence order.
/// Each document is rechecked against this codec/registry and shared source
/// admission before its digest and requirements can become rendering inputs.
/// Reserve and charge the complete plan array before inspecting members. A
/// reservation failure publishes no plan and performs no member processing.
pub fn inspect_namespace<'a, C: FoundationValueCodec>(
    namespace: &labels::namespace::CheckedNamespace<'_, 'a>,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<DocPreparationPlan>, PreparationError<'a, C::Error>> {
    inspect_namespace_inner(namespace, None, registry, c, b)
}
/// Reuse the root's value produced by the current enclosing PageSet encoder.
/// Only the identical immutable document can use it; other members are encoded
/// normally. This is crate-private and accepts no portable validation shortcut.
pub(crate) fn inspect_namespace_encoded_root<'a, C: FoundationValueCodec>(
    namespace: &labels::namespace::CheckedNamespace<'_, 'a>,
    root: (&'a DocumentSyntax, &nepl3_core::value::NdfValue),
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<DocPreparationPlan>, PreparationError<'a, C::Error>> {
    inspect_namespace_inner(namespace, Some(root), registry, c, b)
}
fn inspect_namespace_inner<'a, C: FoundationValueCodec>(
    namespace: &labels::namespace::CheckedNamespace<'_, 'a>,
    root: Option<(&'a DocumentSyntax, &nepl3_core::value::NdfValue)>,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<DocPreparationPlan>, PreparationError<'a, C::Error>> {
    b.poll()?;
    let mut plans = Vec::new();
    let documents = namespace.documents();
    let count = documents.len();
    let bytes = count
        .checked_mul(core::mem::size_of::<DocPreparationPlan>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes)?;
    plans
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    for document in documents {
        b.charge(Resource::Work, 1)?;
        let generated;
        let value = match root {
            Some((owner, value)) if core::ptr::eq(owner, document) => value,
            _ => {
                generated = portable::to_value(document, registry, c, b)?;
                &generated
            }
        };
        let document_digest = c
            .canonical_value_digest(DOCUMENT_DOMAIN, value, b)
            .map_err(boundary)?;
        let requirements = discover(document, value, registry, c, b)?;
        plans.push(DocPreparationPlan {
            document_digest,
            requirements,
        });
    }
    Ok(plans)
}
fn discover<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    encoded: &nepl3_core::value::NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<DocRequirement>, PreparationError<'a, C::Error>> {
    let mut requirements = Vec::new();
    for (index, node) in document.value.nodes.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let requirement = match &node.kind {
            DocKind::Link { target: v, .. } => Some(DocRequirement::Link {
                node: index as u64,
                target: target(v, b)?,
            }),
            DocKind::Image { asset, .. } | DocKind::InlineImage { asset, .. } => {
                Some(DocRequirement::Asset {
                    node: index as u64,
                    asset: AssetRef {
                        id: clone_text(&asset.id, b)?,
                        digest: asset.digest,
                    },
                })
            }
            _ => None,
        };
        if let Some(r) = requirement {
            push(&mut requirements, r, b)?;
        }
    }
    let values = portable::embedded_values(encoded, r, b)?;
    if values.len() != document.value.embeds.len() {
        return Err(PortableError::Shape.into());
    }
    for (index, (embed, guest)) in document.value.embeds.iter().zip(values).enumerate() {
        b.charge(Resource::Work, 1)?;
        let guest_digest = c
            .canonical_value_digest(GUEST_DOMAIN, guest, b)
            .map_err(boundary)?;
        push(
            &mut requirements,
            DocRequirement::Foreign {
                embed: EmbedRef(index as u64),
                kind: embed.kind,
                guest_digest,
            },
            b,
        )?;
    }
    Ok(requirements)
}
