//! Typed Sentence ownership for one bounded native PageSet export operation.
use super::*;
use crate::doc::annotations::article::{self, namespace, render::Session};
use nepl3_core::{
    budget::{Resource, StopReason},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::labels::namespace as labels;
use nepl3_doc_html::namespace::pages as routed;

pub(super) struct Output {
    pub displayed: nepl3_doc_html::pages::RenderedCodePages,
    pub sources: Vec<article::SelectionParts>,
    pub records: Vec<Vec<article::render::SentenceRecord>>,
}
fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), String> {
    b.charge(Resource::Work, 1).map_err(err)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )
    .map_err(err)?;
    v.try_reserve(1)
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    v.push(item);
    Ok(())
}
pub(super) fn render(
    compiled: &Compiled,
    request: &PagesHtmlRequest,
    code: &[code::PreparedPageCode],
    math: &mut [Vec<super::super::math::Occurrence>],
    renderer: super::super::MathRenderer,
    b: &mut Budget,
) -> Result<Output, String> {
    let r = &compiled.doc.registry;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &sources, &mut admission).map_err(err)?;
    let mut selections = Vec::new();
    for (page_index, page) in request.set.pages.iter().enumerate() {
        let selection = article::collect_structural(
            &page.document,
            &mut super::super::sentence_host(compiled, r, &mut codec)?,
            b,
        )
        .map_err(|error| {
            format!(
                "page {page_index} ({}): selection: {error:?}",
                page.registration.id
            )
        })?;
        push(&mut selections, selection, b)?;
    }
    let (displayed, records) = {
        let mut resolutions = Vec::new();
        for (page, selection) in selections.iter().enumerate() {
            let mut adoption = Vec::new();
            for index in 0..selection.sentences().len() {
                push(&mut adoption, namespace::SentenceOccurrenceId(index), b)?;
            }
            let resolved = namespace::resolve(
                selection,
                &adoption,
                &mut super::super::sentence_host(compiled, r, &mut codec)?,
                b,
            )
            .map_err(|error| {
                format!(
                    "page {page} ({}): namespace: {error:?}",
                    request.set.pages[page].registration.id
                )
            })?;
            push(&mut resolutions, resolved, b)?;
        }
        let mut members = Vec::new();
        for (page, resolved) in resolutions.iter().enumerate() {
            let inspected = resolved
                .inspect_members(
                    &mut super::super::sentence_host(compiled, r, &mut codec)?,
                    b,
                )
                .map_err(|error| {
                    format!(
                        "page {page} ({}): members: {error:?}",
                        request.set.pages[page].registration.id
                    )
                })?;
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
        for (page, resolved) in refs.iter().zip(&resolutions) {
            let (definitions, references) = resolved.orders();
            push(
                &mut namespaces,
                labels::resolve_interleaved(page, definitions, references, b).map_err(err)?,
                b,
            )?;
        }
        let mut borrowed = Vec::new();
        for namespace in &namespaces {
            push(&mut borrowed, namespace, b)?;
        }
        let prepared = routed::prepare(&request.set, &borrowed, &request.options, r, &mut codec, b)
            .map_err(err)?;
        let mut sessions = Vec::new();
        for (page, selection) in selections.iter().enumerate() {
            let session = Session::for_display::<std::convert::Infallible, String>(
                selection,
                &request.options,
                b,
            )
            .map_err(|error| {
                format!(
                    "page {page} ({}): display selection: {error:?}",
                    request.set.pages[page].registration.id
                )
            })?;
            push(&mut sessions, session, b)?;
        }
        let rendered = routed::render(
            &prepared,
            &mut |page, prepared, view, index, b| {
                let page = usize::try_from(page).map_err(err)?;
                let session = sessions.get_mut(page).ok_or("SentencePageMissing")?;
                let resolution = resolutions.get(page).ok_or("SentencePageMissing")?;
                session
                    .render_guest(
                        prepared,
                        resolution.members(),
                        &mut super::super::sentence_host(compiled, r, &mut codec)?,
                        &mut |view, index, codec, b| {
                            if view.kind == nepl3_doc_core::model::EmbedKind::Code {
                                code.get(page)
                                    .ok_or("CodePageMissing")?
                                    .render_with_codec(view, index, codec, b)
                            } else {
                                super::super::math::DisplayHost {
                                    compiled,
                                    registry: r,
                                    codec,
                                    preference: renderer,
                                }
                                .render(
                                    view,
                                    index,
                                    math.get_mut(page).ok_or("MathPageMissing")?,
                                    b,
                                )
                            }
                        },
                        view,
                        index,
                        b,
                    )
                    .map_err(err)
            },
            b,
        )
        .map_err(err)?;
        let mut fragments = Vec::new();
        let mut foreign = Vec::new();
        let mut records = Vec::new();
        for ((session, resolution), fragment) in sessions
            .into_iter()
            .zip(&resolutions)
            .zip(rendered.fragments)
        {
            let (fragment, sentence) = session
                .finish::<std::convert::Infallible, String>(fragment, resolution.members(), b)
                .map_err(err)?;
            push(&mut fragments, fragment.fragment, b)?;
            push(&mut foreign, fragment.foreign, b)?;
            push(&mut records, sentence, b)?;
        }
        // Separate host identity: fixed all-occurrence adoption, not a portable
        // CheckedPages receipt and never the legacy page identity contract.
        let value = nepl3_doc_core::portable::pages::set_to_value(&request.set, r, &mut codec, b)
            .map_err(err)?;
        let source = codec
            .canonical_value_digest(b"nepl3.local-doc-pages.sentence-input/1\0", &value, b)
            .map_err(err)?;
        let identity = display_identity(source, &request.options.parallel, b)?;
        (
            nepl3_doc_html::pages::RenderedCodePages {
                pages: nepl3_doc_html::pages::RenderedPages {
                    identity,
                    fragments,
                },
                foreign,
            },
            records,
        )
    };
    let mut parts = Vec::new();
    for selection in selections {
        push(&mut parts, selection.into_parts(), b)?;
    }
    Ok(Output {
        displayed,
        sources: parts,
        records,
    })
}

// Retain Rows/Columns identities; Single has a separately domain-tagged,
// length-delimited native host contract, not a portable receipt.
fn display_identity(
    source: Digest,
    mode: &nepl3_doc_html::ParallelMode,
    b: &mut Budget,
) -> Result<Digest, String> {
    use nepl3_doc_html::ParallelMode;
    match mode {
        ParallelMode::Rows | ParallelMode::Columns => {
            let mut bytes = [0u8; 33];
            bytes[..32].copy_from_slice(&source.0);
            bytes[32] = if matches!(mode, ParallelMode::Rows) {
                0
            } else {
                1
            };
            b.charge(Resource::Work, 33).map_err(err)?;
            Ok(Digest::of(&bytes))
        }
        ParallelMode::Single { .. } => super::display::bind_identity(
            b"nepl3.local-doc-pages.sentence-single/1\0",
            source,
            mode,
            b,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_display_identity_preserves_legacy_modes_and_bounds_single_options()
    -> Result<(), String> {
        use nepl3_doc_html::ParallelMode;
        let source = Digest([3; 32]);
        for (mode, tag) in [(ParallelMode::Rows, 0), (ParallelMode::Columns, 1)] {
            let mut old = [3u8; 33];
            old[32] = tag;
            assert_eq!(
                display_identity(source, &mode, &mut crate::doc::source::budget())?,
                Digest::of(&old)
            );
        }
        let mode = ParallelMode::Single {
            language: "fr".into(),
            fallbacks: vec!["en".into(), "ja".into()],
        };
        let mut measured = crate::doc::source::budget();
        measured.charge(Resource::Work, 7).map_err(err)?;
        let actual = display_identity(source, &mode, &mut measured)?;
        let mut expected = b"nepl3.local-doc-pages.sentence-single/1\0".to_vec();
        expected.extend_from_slice(&[3; 32]);
        expected.push(2);
        expected.extend_from_slice(&2u64.to_le_bytes());
        expected.extend_from_slice(b"fr");
        expected.extend_from_slice(&2u64.to_le_bytes());
        for value in [b"en", b"ja"] {
            expected.extend_from_slice(&2u64.to_le_bytes());
            expected.extend_from_slice(value);
        }
        assert_eq!(actual, Digest::of(&expected));
        let mut limits = measured.limits();
        limits.work = measured.usage().work;
        limits.allocation_units = measured.usage().allocation_units;
        let mut exact = Budget::new(limits);
        exact.charge(Resource::Work, 7).map_err(err)?;
        assert_eq!(display_identity(source, &mode, &mut exact)?, actual);
        limits.allocation_units -= 1;
        let mut short = Budget::new(limits);
        short.charge(Resource::Work, 7).map_err(err)?;
        assert!(display_identity(source, &mode, &mut short).is_err());
        assert_eq!(short.poll(), Err(StopReason::AllocationLimit));
        Ok(())
    }
}
