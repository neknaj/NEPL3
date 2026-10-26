//! Static SVG for exact, explicitly adopted PageSet namespaces.
//! This is native preparation, not a CLI asset registry or portable proof.
use super::*;
use crate::assets::{AssetError, SvgMode};
use crate::namespace::assets::MemberAssets;
use alloc::string::String;

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
    prepare_mode(
        set,
        namespaces,
        options,
        inputs,
        SvgMode::Embedded,
        registry,
        codec,
        budget,
    )
}

/// Freshly checked external image sources. This is native admission evidence,
/// not a completed package or proof that a host emitted any file.
/// Paths are relative to each page, not to the PageSet's output root.
pub struct ExternalAsset<'i> {
    page: u64,
    member: MemberId,
    path: String,
    svg: &'i str,
}
impl<'i> ExternalAsset<'i> {
    pub fn page(&self) -> u64 {
        self.page
    }
    pub fn member(&self) -> MemberId {
        self.member
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn svg(&self) -> &'i str {
        self.svg
    }
}

/// Retains the exact prepared pages and their admitted external bindings.
/// Bindings are not deduplicated: packaging must resolve page-relative paths,
/// reject collisions and charge physical output separately.
pub struct PreparedExternalPages<'a, 'i> {
    prepared: PreparedPages<'a>,
    assets: Vec<ExternalAsset<'i>>,
}
impl<'a, 'i> PreparedExternalPages<'a, 'i> {
    pub fn pages(&self) -> &PreparedPages<'a> {
        &self.prepared
    }
    pub fn assets(&self) -> &[ExternalAsset<'i>] {
        &self.assets
    }
    /// Transfer the already admitted evidence without cloning or re-admission.
    /// The caller must still preserve page-relative routing when packaging.
    pub fn into_parts(self) -> (PreparedPages<'a>, Vec<ExternalAsset<'i>>) {
        (self.prepared, self.assets)
    }
}

/// Admit exact page/member SVG inputs, including hidden members, and retain
/// immutable bytes alongside typed external image paths. All ordinary routing,
/// digest, static-SVG, alt and unused-input checks run before evidence is exposed.
/// A successful preparation is not an emission receipt or a filesystem grant.
#[allow(clippy::too_many_arguments)]
pub fn prepare_external<'a, 'i, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &[&CheckedNamespace<'_, 'a>],
    options: &'a RenderOptions,
    inputs: &[PageMemberAssets<'i>],
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedExternalPages<'a, 'i>, PreparationError<'a, C::Error>> {
    let prepared = prepare_mode(
        set,
        namespaces,
        options,
        inputs,
        SvgMode::External,
        registry,
        codec,
        budget,
    )?;
    let mut assets = Vec::new();
    for input in inputs {
        for svg in input.assets.inputs {
            budget.charge(Resource::Work, svg.svg.len() as u64)?;
            let digest = nepl3_core::source::Digest::of(svg.svg.as_bytes());
            budget.charge(Resource::AllocationUnits, 75)?;
            let mut path = String::from("assets/");
            for byte in digest.0 {
                path.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
                path.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
            }
            path.push_str(".svg");
            build::push(
                &mut assets,
                ExternalAsset {
                    page: input.page,
                    member: input.assets.member,
                    path,
                    svg: svg.svg,
                },
                budget,
            )?;
        }
    }
    budget.poll()?;
    Ok(PreparedExternalPages { prepared, assets })
}

#[allow(clippy::too_many_arguments)]
fn prepare_mode<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    namespaces: &[&CheckedNamespace<'_, 'a>],
    options: &'a RenderOptions,
    inputs: &[PageMemberAssets<'_>],
    mode: SvgMode,
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
                mode,
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
