//! Embedded static SVG for exact, explicitly adopted PageSet namespaces.
//! This is native preparation, not a CLI asset registry or portable proof.
use super::*;
use crate::assets::{AssetError, SvgMode};
use crate::namespace::assets::MemberAssets;

/// Page and member indices are occurrence-local. Equal asset IDs or equal
/// document digests in another page/member do not grant access to these bytes.
pub struct PageMemberAssets<'a> {
    pub page: u64,
    pub assets: MemberAssets<'a>,
}
#[derive(Debug)]
pub enum PreparationError<'a, E> {
    Stopped(StopReason),
    Routing(routing::Error<'a, E>),
    InvalidPage(u64),
    InvalidMember {
        page: u64,
        member: MemberId,
    },
    DuplicateMember {
        page: u64,
        member: MemberId,
    },
    Member {
        page: u64,
        member: MemberId,
        error: AssetError<'a, E>,
    },
    InternalShape,
}
impl<E> From<StopReason> for PreparationError<'_, E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Freshly revalidate all namespaces and routes with one codec/admission and
/// cumulative budget. Static SVG is embedded, so no external image artifact
/// route or packaging authority is implied. Root and adopted-member alt data
/// remain explicit host projections, bound by the existing plain-text checks.
/// Rendering through `pages::render` still verifies emitted target anchors.
#[allow(clippy::too_many_arguments)]
pub fn prepare_embedded<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &[&CheckedNamespace<'_, 'a>],
    options: &'a RenderOptions,
    inputs: &[PageMemberAssets<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedPages<'a>, PreparationError<'a, C::Error>> {
    budget.poll()?;
    let checked =
        routing::resolve(set, namespaces, registry, codec, budget).map_err(
            |error| match error {
                routing::Error::Stopped(reason) => PreparationError::Stopped(reason),
                error => PreparationError::Routing(error),
            },
        )?;
    for (index, input) in inputs.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let page = usize::try_from(input.page)
            .ok()
            .and_then(|page| namespaces.get(page))
            .ok_or(PreparationError::InvalidPage(input.page))?;
        if page.document(input.assets.member).is_none() {
            return Err(PreparationError::InvalidMember {
                page: input.page,
                member: input.assets.member,
            });
        }
        for prior in &inputs[..index] {
            budget.charge(Resource::Work, 1)?;
            if prior.page == input.page && prior.assets.member == input.assets.member {
                return Err(PreparationError::DuplicateMember {
                    page: input.page,
                    member: input.assets.member,
                });
            }
        }
    }
    let mut pages = Vec::new();
    let mut pending_links = checked.links();
    for (page, namespace) in namespaces.iter().enumerate() {
        let mut members = Vec::new();
        for (index, document) in namespace.documents().enumerate() {
            budget.charge(Resource::Work, 1)?;
            let member = MemberId(index as u64);
            let plan = checked
                .member_plan(page as u64, member)
                .ok_or(PreparationError::InternalShape)?;
            // PageSet Code remains root-only. SVG must not broaden the guest
            // callback capabilities of routed namespace display.
            if member != MemberId(0) {
                for requirement in &plan.requirements {
                    budget.charge(Resource::Work, 1)?;
                    if matches!(
                        requirement,
                        DocRequirement::Foreign {
                            kind: nepl3_doc_core::model::EmbedKind::Code,
                            ..
                        }
                    ) {
                        return Err(PreparationError::Member {
                            page: page as u64,
                            member,
                            error: AssetError::Unsupported,
                        });
                    }
                }
            }
            let mut selected = None;
            for input in inputs {
                budget.charge(Resource::Work, 1)?;
                if input.page == page as u64 && input.assets.member == member {
                    selected = Some(&input.assets);
                    break;
                }
            }
            let (svg, alt) = selected.map_or((&[][..], &[][..]), |input| {
                (input.inputs, input.resolved_alt)
            });
            let links = member_links(set, page, member, &mut pending_links, budget)?;
            let prepared = crate::assets::prepare_svg_routed_plan(
                document,
                options,
                svg,
                SvgMode::Embedded,
                registry,
                codec,
                budget,
                Some(true),
                alt,
                plan,
                links,
            )
            .map_err(|error| match error {
                AssetError::Stopped(reason) => PreparationError::Stopped(reason),
                error => PreparationError::Member {
                    page: page as u64,
                    member,
                    error,
                },
            })?;
            build::push(&mut members, prepared, budget)?;
        }
        build::push(
            &mut pages,
            PreparedDisplayNamespace(PreparedNamespace { members }),
            budget,
        )?;
    }
    if !pending_links.is_empty() {
        return Err(PreparationError::InternalShape);
    }
    budget.poll()?;
    Ok(PreparedPages { set, pages })
}
