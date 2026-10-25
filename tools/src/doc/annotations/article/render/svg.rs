//! Native planned SVG composition, with raw assets admitted for the root only.
//! This neither writes external files nor broadens CLI or PageSet admission.
pub mod pages;

use super::{MemberOrigin, Rendered as RenderedArticle, Session, boxed, namespace};
use crate::doc::annotations::{SentenceAnnotationRenderer, alt, document};
use nepl3_core::{
    budget::{Budget, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    labels::namespace::{CheckedNamespace, MemberId},
    model::{DocEmbedView, EmbedRef},
    portable::PortableError,
};
use nepl3_doc_html::{
    LocalPreparationError,
    assets::{AssetError, SvgInput, SvgMode},
    namespace::{assets as namespace_assets, display},
};
use nepl3_markup::html::HtmlRequest;

/// Owned location: no borrowed checked namespace is retained by an error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub member: Option<MemberId>,
    pub origin: Option<MemberOrigin>,
}
#[derive(Debug)]
pub enum AssetFailure<E> {
    Document(document::Error<E>),
    Text(PortableError<E>),
    Duplicate,
    Unused,
    Missing,
    InvalidSvg,
    Digest,
    Alt,
    Unsupported,
}
#[derive(Debug)]
pub enum Error<E, F> {
    Stopped(StopReason),
    Alt(Box<alt::Error<E>>),
    Resolve(Box<namespace::Error<E>>),
    Preparation {
        location: Location,
        error: Box<AssetFailure<E>>,
    },
    InvalidMember(MemberId),
    DuplicateMember(MemberId),
    Selection,
    Render(Box<super::Error<E, F>>),
}
impl<E, F> From<StopReason> for Error<E, F> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct Rendered<'s, 'a> {
    pub output: RenderedArticle<'s, 'a>,
    /// Actual lowered Sentence owners and identity-bound BaseOnly projections.
    pub alt_projection: alt::Projection,
}
fn located<E, F>(location: Location, error: AssetFailure<E>, b: &mut Budget) -> Error<E, F> {
    match boxed(error, b) {
        Ok(error) => Error::Preparation { location, error },
        Err(reason) => Error::Stopped(reason),
    }
}
fn location(member: Option<MemberId>, members: namespace::Members<'_>) -> Location {
    let origin = member.and_then(|id| {
        if id == MemberId(0) {
            Some(MemberOrigin::Article)
        } else {
            members.guest(id).map(|(origin, _)| origin)
        }
    });
    Location { member, origin }
}
fn local_failure<C: FoundationValueCodec, F>(
    failure: LocalPreparationError<'_, C::Error>,
    member: Option<MemberId>,
    checked: &CheckedNamespace<'_, '_>,
    members: namespace::Members<'_>,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Error<C::Error, F> {
    let owned = match member {
        Some(member) => match checked.document(member) {
            Some(document) => {
                document::preparation_error(failure, document, host.registry, host.codec, b)
            }
            None => return Error::Selection,
        },
        None => document::unlocated_preparation_error(failure),
    };
    if let document::Error::Stopped(reason) = owned {
        return Error::Stopped(reason);
    }
    located(location(member, members), AssetFailure::Document(owned), b)
}
fn preparation_failure<C: FoundationValueCodec, F>(
    failure: namespace_assets::PreparationError<'_, C::Error>,
    checked: &CheckedNamespace<'_, '_>,
    members: namespace::Members<'_>,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Error<C::Error, F> {
    let error = match failure {
        namespace_assets::PreparationError::Stopped(reason) => Error::Stopped(reason),
        namespace_assets::PreparationError::InvalidMember(id) => Error::InvalidMember(id),
        namespace_assets::PreparationError::DuplicateMember(id) => Error::DuplicateMember(id),
        namespace_assets::PreparationError::Input(failure) => local_failure(
            LocalPreparationError::Input(failure.error),
            failure.member,
            checked,
            members,
            host,
            b,
        ),
        namespace_assets::PreparationError::Member { member, error } => {
            member_failure(error, member, checked, members, host, b)
        }
    };
    match b.poll() {
        Ok(()) => error,
        Err(reason) => Error::Stopped(reason),
    }
}
fn member_failure<C: FoundationValueCodec, F>(
    error: AssetError<'_, C::Error>,
    member: MemberId,
    checked: &CheckedNamespace<'_, '_>,
    members: namespace::Members<'_>,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Error<C::Error, F> {
    let owned = match error {
        AssetError::Stopped(reason) => return Error::Stopped(reason),
        AssetError::Preparation(error) => {
            return local_failure(error, Some(member), checked, members, host, b);
        }
        AssetError::Text(error) => AssetFailure::Text(error),
        AssetError::Duplicate => AssetFailure::Duplicate,
        AssetError::Unused => AssetFailure::Unused,
        AssetError::Missing => AssetFailure::Missing,
        AssetError::InvalidSvg => AssetFailure::InvalidSvg,
        AssetError::Digest => AssetFailure::Digest,
        AssetError::Alt => AssetFailure::Alt,
        AssetError::Unsupported => AssetFailure::Unsupported,
    };
    located(location(Some(member), members), owned, b)
}
fn rendering_failure<E, F>(error: super::Error<E, F>, b: &mut Budget) -> Error<E, F> {
    if let super::Error::Stopped(reason) = error {
        return Error::Stopped(reason);
    }
    match boxed(error, b) {
        Ok(error) => Error::Render(error),
        Err(reason) => Error::Stopped(reason),
    }
}
/// Render one exact planned Article using explicit root asset bytes. Every
/// adopted member is still inspected; guest assets receive no input and fail.
/// Alt owners remain retained separately from selected HTML namespace members.
/// External mode returns content-addressed references, not written-file proof.
#[allow(clippy::too_many_arguments)]
pub fn render<'s, 'a, C: FoundationValueCodec, F>(
    plan: &super::super::display::Plan<'s, 'a>,
    root_inputs: &[SvgInput<'_>],
    mode: SvgMode,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    other: &mut impl FnMut(&DocEmbedView<'_>, EmbedRef, &mut C, &mut Budget) -> Result<HtmlRequest, F>,
    b: &mut Budget,
) -> Result<Rendered<'s, 'a>, Error<C::Error, F>> {
    b.poll()?;
    let alt_projection =
        alt::prepare(plan.selection().document(), host, b).map_err(|error| match boxed(error, b) {
            Ok(error) => Error::Alt(error),
            Err(reason) => Error::Stopped(reason),
        });
    b.poll()?;
    let alt_projection = alt_projection?;
    let inputs = [namespace_assets::MemberAssets {
        member: MemberId(0),
        inputs: root_inputs,
        resolved_alt: &alt_projection.resolved,
    }];
    let result = namespace::resolve_with(
        plan.selection(),
        plan.adoption(),
        host,
        b,
        |checked, members, host, b| {
            let prepared = namespace_assets::prepare(
                checked,
                plan.options(),
                &inputs,
                mode,
                host.registry,
                host.codec,
                b,
            )
            .map_err(|error| preparation_failure(error, checked, members, host, b))?;
            let mut session =
                Session::for_plan(plan, b).map_err(|error| rendering_failure(error, b))?;
            let mut adapter = |view: &DocEmbedView<'_>, embed: EmbedRef, b: &mut Budget| {
                session.render_guest(&prepared, members, host, other, view, embed, b)
            };
            let rendered = display::render_article(&prepared, MemberId(0), &mut adapter, b)
                .map_err(|error| {
                    let error = match error {
                        display::ArticleError::Member(_)
                        | display::ArticleError::ExpectedArticle(_) => super::Error::Selection,
                        display::ArticleError::Render(
                            nepl3_doc_html::ForeignRenderError::Render(error),
                        ) => super::Error::Render(error),
                        display::ArticleError::Render(
                            nepl3_doc_html::ForeignRenderError::Foreign(error),
                        ) => error,
                    };
                    rendering_failure(error, b)
                })?;
            session
                .finish(rendered, members, b)
                .map_err(|error| rendering_failure(error, b))
        },
    );
    b.poll()?;
    let (resolution, (article, sentences)) = match result {
        Ok(result) => result,
        Err(namespace::OperationError::Resolve(error)) => {
            return Err(Error::Resolve(boxed(error, b)?));
        }
        Err(namespace::OperationError::Operation(error)) => return Err(error),
    };
    Ok(Rendered {
        output: RenderedArticle {
            resolution,
            article,
            sentences,
        },
        alt_projection,
    })
}

#[cfg(test)]
mod tests;
