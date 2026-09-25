//! One operation owns structural checks, encoding and namespace resolution.
//! The callback borrows the completed proof while its member tables are alive.
use super::*;
use crate::labels::namespace as labels;

/// One explicitly selected display occurrence. Member zero is the page root
/// at relative depth zero. Guest depths are supplied by the composition host.
#[derive(Clone, Copy)]
pub struct NamespaceDocument<'a> {
    pub document: &'a DocumentSyntax,
    pub relative_depth: u64,
}

#[derive(Debug)]
pub enum ScopedError<'a, C, E> {
    Stopped(StopReason),
    Preparation(Error<'a, C>),
    Output(E),
}

/// Validate, encode and resolve the complete explicit selection with one
/// Budget and codec admission context, then lend the proof to the callback.
/// Each immutable document's structure proof is used by label collection and
/// its encoder within this operation. No proof cache crosses an invocation.
/// Repeated occurrences remain distinct members and retain duplicate-ID checks.
/// The output callback starts only after all pages and members have succeeded.
/// It cannot return references to these operation-owned namespace tables.
pub fn with_resolved<'a, C: FoundationValueCodec, T, E>(
    set: &'a PageSet,
    selected: &[&[NamespaceDocument<'a>]],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
    output: impl FnOnce(&CheckedPageNamespaces<'_, '_, 'a>, &mut C, &mut Budget) -> Result<T, E>,
) -> Result<T, ScopedError<'a, C::Error, E>> {
    resolve_with_inputs(set, selected, registry, codec, b, None, output)
}

/// Reuse exact page-root validation from discovery. Inputs must correspond in
/// page order to the identical immutable root documents. Each receiving codec
/// still admits sources and applies depth; registry changes fully revalidate.
pub fn with_validated_roots<'a, C: FoundationValueCodec, T, E>(
    set: &'a PageSet,
    selected: &[&[NamespaceDocument<'a>]],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
    roots: Vec<crate::check::RegistryValidatedDocumentSyntax<'a, '_>>,
    output: impl FnOnce(&CheckedPageNamespaces<'_, '_, 'a>, &mut C, &mut Budget) -> Result<T, E>,
) -> Result<T, ScopedError<'a, C::Error, E>> {
    resolve_with_inputs(set, selected, registry, codec, b, Some(roots), output)
}

fn resolve_with_inputs<'a, C: FoundationValueCodec, T, E>(
    set: &'a PageSet,
    selected: &[&[NamespaceDocument<'a>]],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
    roots: Option<Vec<crate::check::RegistryValidatedDocumentSyntax<'a, '_>>>,
    output: impl FnOnce(&CheckedPageNamespaces<'_, '_, 'a>, &mut C, &mut Budget) -> Result<T, E>,
) -> Result<T, ScopedError<'a, C::Error, E>> {
    b.poll().map_err(ScopedError::Stopped)?;
    let result = (|| {
        registrations(set, b).map_err(|e| ScopedError::Preparation(Error::Page(e)))?;
        if selected.len() != set.pages.len() {
            return Err(ScopedError::Preparation(Error::Selection));
        }
        for (page, members) in set.pages.iter().zip(selected) {
            b.charge(Resource::Work, 1).map_err(ScopedError::Stopped)?;
            if !matches!(page.document.value.root, DocRoot::Article(_))
                || !members.first().is_some_and(|first| {
                    first.relative_depth == 0 && core::ptr::eq(first.document, &page.document)
                })
            {
                return Err(ScopedError::Preparation(Error::Selection));
            }
        }
        let (value, structures) =
            portable::pages::set_to_value_with_inputs(set, registry, codec, b, roots)
                .map_err(|e| ScopedError::Preparation(Error::Page(PageError::from(e))))?;
        let base = b.current_depth();
        let mut inspected = Vec::new();
        let mut plans = Vec::new();
        for (page, members) in selected.iter().enumerate() {
            let mut page_members = Vec::new();
            let mut page_plans = Vec::new();
            for (member, selected) in members.iter().enumerate() {
                let depth = base
                    .checked_add(selected.relative_depth)
                    .ok_or_else(|| ScopedError::Stopped(b.stop(StopReason::DepthLimit)))?;
                let (labels, plan) = b
                    .with_depth_at_least::<_, Error<'a, C::Error>>(depth, |b| {
                        b.charge(Resource::Work, 1)?;
                        let input_error = |error| Error::Input {
                            page: page as u64,
                            error,
                        };
                        let namespace_error = |error| Error::Namespace {
                            page: page as u64,
                            error,
                        };
                        if member == 0 {
                            let labels = labels::inspect_structure(&structures[page], b)
                                .map_err(namespace_error)?;
                            let encoded = portable::pages::document_value(&value, page, b)
                                .map_err(|e| Error::Page(PageError::from(e)))?;
                            let plan = prepare::encoded_plan(
                                selected.document,
                                encoded,
                                registry,
                                codec,
                                b,
                            )
                            .map_err(input_error)?;
                            Ok((labels, plan))
                        } else {
                            let input = portable::EncodingInput::new(
                                selected.document,
                                registry,
                                b,
                                codec.source_admission(),
                            )
                            .map_err(|e| input_error(crate::labels::LabelError::from(e).into()))?;
                            let labels = labels::inspect_structure(input.structure(), b)
                                .map_err(namespace_error)?;
                            let encoded = input
                                .into_value(codec, b)
                                .map_err(|e| input_error(e.into()))?;
                            let plan = prepare::encoded_plan(
                                selected.document,
                                &encoded,
                                registry,
                                codec,
                                b,
                            )
                            .map_err(input_error)?;
                            Ok((labels, plan))
                        }
                    })
                    .map_err(ScopedError::Preparation)?;
                push(&mut page_members, labels, b).map_err(ScopedError::Stopped)?;
                push(&mut page_plans, plan, b).map_err(ScopedError::Stopped)?;
            }
            push(&mut inspected, page_members, b).map_err(ScopedError::Stopped)?;
            push(&mut plans, page_plans, b).map_err(ScopedError::Stopped)?;
        }
        let mut member_refs = Vec::new();
        for page in &inspected {
            let mut refs = Vec::new();
            for member in page {
                push(&mut refs, member, b).map_err(ScopedError::Stopped)?;
            }
            push(&mut member_refs, refs, b).map_err(ScopedError::Stopped)?;
        }
        let mut namespaces = Vec::new();
        for (page, members) in member_refs.iter().enumerate() {
            let checked = labels::resolve(members, b).map_err(|error| {
                ScopedError::Preparation(Error::Namespace {
                    page: page as u64,
                    error,
                })
            })?;
            push(&mut namespaces, checked, b).map_err(ScopedError::Stopped)?;
        }
        let mut refs = Vec::new();
        for namespace in &namespaces {
            push(&mut refs, namespace, b).map_err(ScopedError::Stopped)?;
        }
        let checked =
            resolve_plans(set, &refs, value, plans, codec, b).map_err(ScopedError::Preparation)?;
        output(&checked, codec, b).map_err(ScopedError::Output)
    })();
    b.poll().map_err(ScopedError::Stopped)?;
    result
}
