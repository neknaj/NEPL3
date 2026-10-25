//! Convert temporary namespace failures into owned, page-contextual errors.
use super::*;

pub(super) fn at_page<E, F>(
    page: u64,
    error: super::super::Error<E, F>,
    b: &mut Budget,
) -> Error<E, F> {
    if let super::super::Error::Stopped(reason) = error {
        return Error::Stopped(reason);
    }
    match boxed(error, b) {
        Ok(error) => Error::Page { page, error },
        Err(reason) => Error::Stopped(reason),
    }
}
pub(super) fn resolve<E, F>(page: u64, error: namespace::Error<E>, b: &mut Budget) -> Error<E, F> {
    match boxed(error, b) {
        Ok(error) => at_page(page, super::super::Error::Resolve(error), b),
        Err(reason) => Error::Stopped(reason),
    }
}
pub(super) fn rendering<E, F>(
    page: u64,
    error: super::super::super::Error<E, F>,
    b: &mut Budget,
) -> Error<E, F> {
    at_page(page, super::super::rendering_failure(error, b), b)
}
fn routing_failure<C: FoundationValueCodec, F>(
    error: routing::Error<'_, C::Error>,
    namespaces: &[CheckedNamespace<'_, '_>],
    resolutions: &[namespace::Resolution<'_, '_>],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Error<C::Error, F> {
    let failure = match error {
        routing::Error::Stopped(reason) => return Error::Stopped(reason),
        routing::Error::Registration(error) => match boxed(error, b) {
            Ok(error) => RoutingFailure::Registration(error),
            Err(reason) => return Error::Stopped(reason),
        },
        routing::Error::PageCount => RoutingFailure::Count,
        routing::Error::ScopeMismatch => RoutingFailure::Scope,
        routing::Error::RootMismatch { page } => RoutingFailure::Root { page },
        routing::Error::Preparation {
            page,
            member,
            error,
        } => {
            let Ok(index) = usize::try_from(page) else {
                return Error::InternalShape;
            };
            let (Some(checked), Some(resolved)) = (namespaces.get(index), resolutions.get(index))
            else {
                return Error::InternalShape;
            };
            let owned = super::super::local_failure(
                LocalPreparationError::Input(error),
                member,
                checked,
                resolved.members(),
                host,
                b,
            );
            return at_page(page, owned, b);
        }
        routing::Error::Destination { member, error } => {
            let page = match &error {
                PageError::Input { page, .. }
                | PageError::InvalidRegistration { page, .. }
                | PageError::Collision { page, .. }
                | PageError::InvalidRelative { page, .. }
                | PageError::MissingPage { page, .. }
                | PageError::MissingFragment { page, .. }
                | PageError::FileFragment { page, .. } => Some(*page),
                _ => None,
            };
            let origin = page
                .and_then(|page| usize::try_from(page).ok())
                .and_then(|page| resolutions.get(page))
                .and_then(|resolved| resolved.origin(member));
            match boxed(error, b) {
                Ok(error) => RoutingFailure::Destination {
                    member,
                    origin,
                    error,
                },
                Err(reason) => return Error::Stopped(reason),
            }
        }
        routing::Error::MissingFragment { site, target } => {
            let Some(origin) = usize::try_from(site.page)
                .ok()
                .and_then(|page| resolutions.get(page))
                .and_then(|resolved| resolved.origin(site.member))
            else {
                return Error::InternalShape;
            };
            RoutingFailure::MissingFragment {
                site,
                target,
                origin,
            }
        }
    };
    match boxed(failure, b) {
        Ok(error) => Error::Routing(error),
        Err(reason) => Error::Stopped(reason),
    }
}
pub(super) fn preparation<C: FoundationValueCodec, F>(
    error: page_assets::PreparationError<'_, C::Error>,
    namespaces: &[CheckedNamespace<'_, '_>],
    resolutions: &[namespace::Resolution<'_, '_>],
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Error<C::Error, F> {
    match error {
        page_assets::PreparationError::Stopped(reason) => Error::Stopped(reason),
        page_assets::PreparationError::Routing(error) => {
            routing_failure(error, namespaces, resolutions, host, b)
        }
        page_assets::PreparationError::InvalidPage(page) => Error::InvalidInputPage(page),
        page_assets::PreparationError::InvalidMember { page, member } => {
            at_page(page, super::super::Error::InvalidMember(member), b)
        }
        page_assets::PreparationError::DuplicateMember { page, member } => {
            at_page(page, super::super::Error::DuplicateMember(member), b)
        }
        page_assets::PreparationError::InternalShape => Error::InternalShape,
        page_assets::PreparationError::Member {
            page,
            member,
            error,
        } => {
            let Ok(index) = usize::try_from(page) else {
                return Error::InternalShape;
            };
            let (Some(checked), Some(resolved)) = (namespaces.get(index), resolutions.get(index))
            else {
                return Error::InternalShape;
            };
            let owned =
                super::super::member_failure(error, member, checked, resolved.members(), host, b);
            at_page(page, owned, b)
        }
    }
}
