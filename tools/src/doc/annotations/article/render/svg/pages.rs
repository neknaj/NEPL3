//! Root-only typed SVG composition for exact externally retained PageSet plans.
//! Returns native fragments and owners, not packaged HTML or an aggregate identity.
use super::super::{SentenceRecord, push};
use super::*;
use crate::doc::annotations::article::display::Plan;
use nepl3_doc_core::pages::{PageError, PageSet, namespace as routing};
use nepl3_doc_html::{RenderOptions, namespace::pages as routed};
use routed::assets::{self as page_assets, PageMemberAssets};
mod failure;

pub struct RootAssets<'a> {
    pub page: u64,
    pub inputs: &'a [SvgInput<'a>],
}
#[derive(Debug)]
pub enum RoutingFailure<E> {
    Registration(Box<PageError<'static, E>>),
    Count,
    Scope,
    Root {
        page: u64,
    },
    Destination {
        member: MemberId,
        origin: Option<MemberOrigin>,
        error: Box<PageError<'static, E>>,
    },
    MissingFragment {
        site: routing::Site,
        target: u64,
        origin: MemberOrigin,
    },
}
#[derive(Debug)]
pub enum Error<E, F> {
    Stopped(StopReason),
    PlanCount,
    RootScope {
        page: u64,
    },
    OptionScope {
        page: u64,
    },
    InvalidInputPage(u64),
    DuplicateInputPage(u64),
    Page {
        page: u64,
        error: Box<super::Error<E, F>>,
    },
    Routing(Box<RoutingFailure<E>>),
    Render(Box<routed::RenderError<super::super::Error<E, F>>>),
    InternalShape,
}
impl<E, F> From<StopReason> for Error<E, F> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct Rendered<'s, 'a> {
    pub pages: routed::RenderedPages,
    pub resolutions: Vec<namespace::Resolution<'s, 'a>>,
    pub sentences: Vec<Vec<SentenceRecord>>,
    /// One actual BaseOnly projection per page, including empty projections.
    pub alt_projections: Vec<alt::Projection>,
}

/// Plans must borrow exactly the corresponding PageSet root and this same
/// immutable options object. Numeric owner/member/occurrence IDs are page-local.
/// Root assets cannot satisfy adopted guest assets, including hidden guests.
/// The caller retains the selections; returned resolutions retain adopted Doc
/// owners, while alt projections retain the separately lowered Sentence owners.
/// Root Code/direct Doc Math use page-aware callbacks; selected Sentence and
/// nested Doc Math retain the Sentence host. No shell, CSS/CSP, filesystem,
/// retry ledger or legacy asset-insensitive PageSet identity is produced here.
#[allow(clippy::too_many_arguments)]
pub fn render<'s, 'a, C: FoundationValueCodec, F>(
    set: &'a PageSet,
    options: &'s RenderOptions,
    plans: &[Plan<'s, 'a>],
    inputs: &[RootAssets<'_>],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(
        u64,
        &DocEmbedView<'_>,
        EmbedRef,
        &mut C,
        &mut Budget,
    ) -> Result<HtmlRequest, F>,
    b: &mut Budget,
) -> Result<Rendered<'s, 'a>, Error<C::Error, F>> {
    let result = render_inner(set, options, plans, inputs, host, other, None, b);
    b.poll()?;
    result
}
#[allow(clippy::too_many_arguments)]
fn render_inner<'s, 'a, 'i, C: FoundationValueCodec, F>(
    set: &'a PageSet,
    options: &'s RenderOptions,
    plans: &[Plan<'s, 'a>],
    inputs: &[RootAssets<'i>],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(
        u64,
        &DocEmbedView<'_>,
        EmbedRef,
        &mut C,
        &mut Budget,
    ) -> Result<HtmlRequest, F>,
    external: Option<&mut Vec<ExternalFile>>,
    b: &mut Budget,
) -> Result<Rendered<'s, 'a>, Error<C::Error, F>> {
    b.poll()?;
    b.charge(nepl3_core::budget::Resource::Work, 1)?;
    if plans.len() != set.pages.len() {
        return Err(Error::PlanCount);
    }
    for (page, (plan, document)) in plans.iter().zip(&set.pages).enumerate() {
        b.charge(nepl3_core::budget::Resource::Work, 1)?;
        if !core::ptr::eq(plan.selection().document(), &document.document) {
            return Err(Error::RootScope { page: page as u64 });
        }
        if !core::ptr::eq(plan.options(), options) {
            return Err(Error::OptionScope { page: page as u64 });
        }
    }
    for (index, input) in inputs.iter().enumerate() {
        b.charge(nepl3_core::budget::Resource::Work, 1)?;
        if usize::try_from(input.page)
            .ok()
            .is_none_or(|page| page >= plans.len())
        {
            return Err(Error::InvalidInputPage(input.page));
        }
        for prior in &inputs[..index] {
            b.charge(nepl3_core::budget::Resource::Work, 1)?;
            if prior.page == input.page {
                return Err(Error::DuplicateInputPage(input.page));
            }
        }
    }
    let mut alt_projections = Vec::new();
    let mut resolutions = Vec::new();
    for (page, plan) in plans.iter().enumerate() {
        let projection = alt::prepare(plan.selection().document(), host, b).map_err(|error| {
            failure::at_page(
                page as u64,
                super::Error::Alt(match boxed(error, b) {
                    Ok(error) => error,
                    Err(reason) => return Error::Stopped(reason),
                }),
                b,
            )
        })?;
        push(&mut alt_projections, projection, b)?;
        let resolved =
            namespace::resolve(plan.selection(), plan.adoption(), host, b).map_err(|error| {
                failure::at_page(
                    page as u64,
                    super::Error::Resolve(match boxed(error, b) {
                        Ok(error) => error,
                        Err(reason) => return Error::Stopped(reason),
                    }),
                    b,
                )
            })?;
        push(&mut resolutions, resolved, b)?;
    }
    let (pages, sentences) = {
        let mut members = Vec::new();
        for (page, resolved) in resolutions.iter().enumerate() {
            let inspected = resolved
                .inspect_members(host, b)
                .map_err(|error| failure::resolve(page as u64, error, b))?;
            push(&mut members, inspected, b)?;
        }
        let mut refs = Vec::new();
        for page in &members {
            let mut items = Vec::new();
            for member in page {
                push(&mut items, member, b)?;
            }
            push(&mut refs, items, b)?;
        }
        let mut namespaces = Vec::new();
        for (page, (resolved, refs)) in resolutions.iter().zip(&refs).enumerate() {
            let checked = resolved
                .recheck_members(refs, host, b)
                .map_err(|error| failure::resolve(page as u64, error, b))?;
            push(&mut namespaces, checked, b)?;
        }
        let mut borrowed = Vec::new();
        for namespace in &namespaces {
            push(&mut borrowed, namespace, b)?;
        }
        let mut bindings = Vec::new();
        for input in inputs {
            let projection = alt_projections
                .get(usize::try_from(input.page).map_err(|_| Error::InternalShape)?)
                .ok_or(Error::InternalShape)?;
            push(
                &mut bindings,
                PageMemberAssets {
                    page: input.page,
                    assets: namespace_assets::MemberAssets {
                        member: MemberId(0),
                        inputs: input.inputs,
                        resolved_alt: &projection.resolved,
                    },
                },
                b,
            )?;
        }
        // Unregistered pages receive no SVG bytes. Every page still retains
        // its actual alt projection, including an empty one.
        let prepared = if let Some(retained) = external {
            let prepared = page_assets::prepare_external(
                set,
                &borrowed,
                options,
                &bindings,
                host.registry,
                host.codec,
                b,
            )
            .map_err(|error| failure::preparation(error, &namespaces, &resolutions, host, b))?;
            let (prepared, assets) = prepared.into_parts();
            for asset in assets {
                let owned = ExternalFile {
                    page: asset.page(),
                    member: asset.member(),
                    path: copy_external(asset.path(), b)?,
                    svg: copy_external(asset.svg(), b)?,
                };
                push(retained, owned, b)?;
            }
            prepared
        } else {
            page_assets::prepare_embedded(
                set,
                &borrowed,
                options,
                &bindings,
                host.registry,
                host.codec,
                b,
            )
            .map_err(|error| failure::preparation(error, &namespaces, &resolutions, host, b))?
        };
        let mut sessions = Vec::new();
        for (page, plan) in plans.iter().enumerate() {
            let session = Session::for_plan::<C::Error, F>(plan, b)
                .map_err(|error| failure::rendering(page as u64, error, b))?;
            push(&mut sessions, session, b)?;
        }
        let rendered = routed::render(
            &prepared,
            &mut |page, prepared, view, embed, b| {
                let index = usize::try_from(page).map_err(|_| super::super::Error::Selection)?;
                let session = sessions
                    .get_mut(index)
                    .ok_or(super::super::Error::Selection)?;
                let resolution = resolutions
                    .get(index)
                    .ok_or(super::super::Error::Selection)?;
                session.render_guest(
                    prepared,
                    resolution.members(),
                    host,
                    &mut |view, embed, codec, b| other(page, view, embed, codec, b),
                    view,
                    embed,
                    b,
                )
            },
            b,
        )
        .map_err(|error| match boxed(error, b) {
            Ok(error) => Error::Render(error),
            Err(reason) => Error::Stopped(reason),
        })?;
        let mut fragments = Vec::new();
        let mut records = Vec::new();
        for (page, ((session, resolution), fragment)) in sessions
            .into_iter()
            .zip(&resolutions)
            .zip(rendered.fragments)
            .enumerate()
        {
            let (fragment, sentences) = session
                .finish::<C::Error, F>(fragment, resolution.members(), b)
                .map_err(|error| failure::rendering(page as u64, error, b))?;
            push(&mut fragments, fragment, b)?;
            push(&mut records, sentences, b)?;
        }
        (routed::RenderedPages { fragments }, records)
    };
    Ok(Rendered {
        pages,
        resolutions,
        sentences,
        alt_projections,
    })
}

/// Only the external renderer constructs this pairing of rendered pages and
/// owned, successfully admitted root assets. No package is implied.
pub struct ExternalRendered<'s, 'a> {
    pub(crate) rendered: Rendered<'s, 'a>,
    pub(crate) assets: Vec<ExternalFile>,
}
impl<'s, 'a> ExternalRendered<'s, 'a> {
    pub fn rendered(&self) -> &Rendered<'s, 'a> {
        &self.rendered
    }
    pub fn assets(&self) -> &[ExternalFile] {
        &self.assets
    }
}

/// Root-only external SVG composition. Adopted guest assets cannot be supplied
/// by root bindings, even when hidden. Admitted bytes are copied under the cumulative budget.
#[allow(clippy::too_many_arguments)]
pub fn render_external<'s, 'a, 'i, C: FoundationValueCodec, F>(
    set: &'a PageSet,
    options: &'s RenderOptions,
    plans: &[Plan<'s, 'a>],
    inputs: &[RootAssets<'i>],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(
        u64,
        &DocEmbedView<'_>,
        EmbedRef,
        &mut C,
        &mut Budget,
    ) -> Result<HtmlRequest, F>,
    b: &mut Budget,
) -> Result<ExternalRendered<'s, 'a>, Error<C::Error, F>> {
    let mut assets = Vec::new();
    let result = render_inner(
        set,
        options,
        plans,
        inputs,
        host,
        other,
        Some(&mut assets),
        b,
    );
    b.poll()?;
    result.map(|rendered| ExternalRendered { rendered, assets })
}

/// Owned copies are constructed only from core admission evidence; this avoids
/// tying retained file bytes to temporary alt-projection borrows.
pub struct ExternalFile {
    page: u64,
    member: MemberId,
    path: String,
    svg: String,
}
impl ExternalFile {
    pub fn page(&self) -> u64 {
        self.page
    }
    pub fn member(&self) -> MemberId {
        self.member
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn svg(&self) -> &str {
        &self.svg
    }
}
fn copy_external(value: &str, b: &mut Budget) -> Result<String, StopReason> {
    use nepl3_core::budget::Resource;
    b.charge(Resource::Work, value.len() as u64)?;
    b.charge(Resource::AllocationUnits, value.len() as u64)?;
    let mut out = String::new();
    out.try_reserve(value.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    out.push_str(value);
    Ok(out)
}
