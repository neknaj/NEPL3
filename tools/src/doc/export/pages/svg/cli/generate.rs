mod native;
use super::input::{Captured, SVG_BYTES, SVG_COUNT};
use crate::doc::{
    annotations::article,
    export::{self, MathRenderer, pages},
    source::{Compiled, err},
};
use article::render::svg::pages as planned;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{Digest, SourceAdmission, SourceStore},
};
use nepl3_doc_core::model::EmbedKind;
use nepl3_doc_html::assets::SvgInput;
use nepl3_wire::foundation::FoundationCodec;
use pages::svg::{RootAssets, identity, output};
use std::collections::BTreeMap;

pub(super) struct Generated {
    pub files: BTreeMap<String, Vec<u8>>,
    pub manifest: String,
    pub math_generation: Vec<Vec<export::math::generation::Occurrence>>,
    pub native_generation: Option<native::Prepared>,
}
fn push<T>(v: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), String> {
    b.charge(Resource::Work, 1).map_err(err)?;
    b.charge(
        Resource::AllocationUnits,
        (2 * core::mem::size_of::<T>()) as u64,
    )
    .map_err(err)?;
    v.try_reserve(1)
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    v.push(value);
    Ok(())
}
fn metadata_charge(bytes: usize, allocation: usize, b: &mut Budget) -> Result<(), String> {
    b.charge(Resource::Work, bytes as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, allocation as u64)
        .map_err(err)
}
#[cfg(test)]
pub(super) fn run(c: &Compiled, captured: &Captured, b: &mut Budget) -> Result<Generated, String> {
    run_selected(c, captured, MathRenderer::MathMLOnly, b)
}
pub(super) fn run_selected(
    c: &Compiled,
    captured: &Captured,
    preference: MathRenderer,
    b: &mut Budget,
) -> Result<Generated, String> {
    let result = run_inner(c, captured, preference, b);
    b.poll().map_err(err)?;
    result
}
fn run_inner(
    c: &Compiled,
    captured: &Captured,
    preference: MathRenderer,
    b: &mut Budget,
) -> Result<Generated, String> {
    b.poll().map_err(err)?;
    let initial = b.usage();
    // New source-host provenance is explicitly charged before shared preparation.
    // The legacy path retains its existing preparation/receipt charges unchanged.
    for (entry, source) in &captured.inputs {
        let names = entry.id.len()
            + entry.source.len()
            + entry.route.len()
            + entry.input.as_ref().map_or(0, String::len);
        metadata_charge(source.len() + names + 16_384, 2 * names + 16_384, b)?;
    }
    for (entry, _) in &captured.files {
        let names = entry.id.len()
            + entry.source.len()
            + entry.route.len()
            + entry.input.as_ref().map_or(0, String::len);
        metadata_charge(names + 2048, 2 * names + 4096, b)?;
    }
    let option_size = match &captured.options.parallel {
        nepl3_doc_html::ParallelMode::Single {
            language,
            fallbacks,
        } => {
            language.len()
                + fallbacks
                    .iter()
                    .map(|s| s.len() + core::mem::size_of::<String>())
                    .sum::<usize>()
        }
        _ => 0,
    };
    metadata_charge(option_size, 2 * option_size, b)?;
    let prepared = pages::prepare_inputs(
        c,
        &captured.inputs,
        &captured.files,
        captured.phases,
        b,
        &[],
        captured.options.clone(),
    )?;
    let request = &prepared.request;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut host = export::sentence_host(c, &c.doc.registry, &mut codec)?;
    let mut selections = Vec::new();
    let mut math = Vec::new();
    for page in &request.set.pages {
        let selected = article::collect_structural(&page.document, &mut host, b).map_err(err)?;
        push(&mut selections, selected, b)?;
        push(&mut math, Vec::new(), b)?;
    }
    let mut plans = Vec::new();
    for selection in &selections {
        let plan = article::display::plan(selection, &request.options, b).map_err(err)?;
        push(&mut plans, plan, b)?;
    }
    let mut borrowed = Vec::new();
    for binding in &captured.assets {
        let mut assets = Vec::new();
        for asset in &binding.assets {
            push(
                &mut assets,
                SvgInput {
                    id: &asset.id,
                    svg: &asset.svg,
                },
                b,
            )?;
        }
        push(&mut borrowed, assets, b)?;
    }
    let mut bindings = Vec::new();
    for (binding, inputs) in captured.assets.iter().zip(&borrowed) {
        push(
            &mut bindings,
            RootAssets {
                page: binding.page as u64,
                inputs,
            },
            b,
        )?;
    }
    let rendered = planned::render(
        &request.set,
        &request.options,
        &plans,
        &bindings,
        &mut host,
        &mut |page, view, index, codec, b| {
            let page = usize::try_from(page).map_err(err)?;
            if view.kind == EmbedKind::Code {
                prepared
                    .page_code
                    .get(page)
                    .ok_or("CodePageMissing")?
                    .render_with_codec(view, index, codec, b)
            } else {
                export::math::DisplayHost {
                    compiled: c,
                    registry: &c.doc.registry,
                    codec,
                    preference,
                }
                .render(
                    view,
                    index,
                    math.get_mut(page).ok_or("MathPageMissing")?,
                    b,
                )
            }
        },
        b,
    )
    .map_err(err)?;
    let input_identity = identity::input(
        &request.set,
        &request.options,
        &bindings,
        &c.doc.registry,
        host.codec,
        b,
    )?;
    let mut generated = Vec::new();
    for (page, ((records, fragment), document)) in math
        .iter_mut()
        .zip(&rendered.pages.fragments)
        .zip(&request.set.pages)
        .enumerate()
    {
        export::math::compose(
            records,
            &fragment.foreign,
            &document.document.value.embeds,
            b,
        )?;
        let generation = export::math::generation::collect(
            records,
            rendered.sentences.get(page).ok_or("SentencePageMissing")?,
            preference,
            b,
        )?;
        push(&mut generated, generation, b)?;
    }
    let native_generation = if preference == MathRenderer::KaTeXPreferred {
        Some(native::prepare(
            &mut generated,
            &mut math,
            &request.set,
            input_identity,
            b,
        )?)
    } else {
        None
    };
    let mut formula_pages = Vec::new();
    for records in &generated {
        let mut formulas = Vec::new();
        for record in records {
            if preference == MathRenderer::MathMLOnly {
                if !matches!(
                    record.tex.as_ref(),
                    crate::doc::math::display::TexPreparation::MathMLOnly
                ) || record.fallback.is_some()
                    || record.fallback_detail.is_some()
                {
                    return Err("ExplicitMathMLSelectionMismatch".into());
                }
            } else if matches!(
                record.tex.as_ref(),
                crate::doc::math::display::TexPreparation::MathMLOnly
            ) {
                return Err("PreferredMathSelectionMismatch".into());
            }
            push(
                &mut formulas,
                output::Formula {
                    root: record.root,
                    display: match record.display {
                        nepl3_markup::mathml::Display::Inline => "inline",
                        nepl3_markup::mathml::Display::Block => "block",
                    },
                    owner_path: &record.owner,
                    renderer: if preference == MathRenderer::KaTeXPreferred
                        && record.fallback.is_none()
                    {
                        "KaTeX+MathML"
                    } else {
                        "MathML"
                    },
                    selection_reason: if preference == MathRenderer::MathMLOnly {
                        "ExplicitMathMLOnly"
                    } else {
                        "KaTeXPreferred"
                    },
                    fallback_reason: record.fallback.map(|reason| match reason {
                        export::math::Fallback::HostCapabilityUnavailable => {
                            "KaTeXHostCapabilityUnavailable"
                        }
                        export::math::Fallback::ResourceUnavailable => "KaTeXResourceUnavailable",
                        export::math::Fallback::RendererParseError => "KaTeXRendererParseError",
                        export::math::Fallback::UnsupportedTex { .. } => "FaithfulTexUnsupported",
                    }),
                },
                b,
            )?;
        }
        push(&mut formula_pages, formulas, b)?;
    }
    let mut svg_capture = Vec::new();
    for binding in &captured.assets {
        let page = &captured
            .inputs
            .get(binding.page)
            .ok_or("SvgPageMissing")?
            .0
            .id;
        for asset in &binding.assets {
            metadata_charge(asset.svg.len() + 64, 64, b)?;
            push(
                &mut svg_capture,
                output::CapturedSvg {
                    page,
                    id: &asset.id,
                    source: &asset.source,
                    bytes: asset.svg.len(),
                    sha256: export::digest_hex(Digest::of(asset.svg.as_bytes())),
                },
                b,
            )?;
        }
    }
    let evidence = output::SourceEvidence {
        scope: "Inputs captured once under finite count/byte limits before parsing; capture I/O is outside parse/output Budgets. Output initial_usage precedes source preparation; each parse/lower has separate limits. Resolved paths are containment-checked, not race-free filesystem capabilities. No persistent retry/capture restoration.",
        math_renderer: if preference == MathRenderer::MathMLOnly {
            "mathml-only-explicit"
        } else {
            "katex-preferred"
        },
        sources: &prepared.origins,
        registered_files: &prepared.file_origins,
        svg_capture,
        svg_total_byte_limit: SVG_BYTES,
        svg_total_count_limit: SVG_COUNT,
        math: formula_pages,
        native_generation: native_generation
            .as_ref()
            .map(|n| n.evidence(&generated, &request.set, b))
            .transpose()?,
    };
    let (files, manifest) = output::package(
        &request.set,
        &request.options,
        &bindings,
        &rendered,
        input_identity,
        initial,
        Some(&evidence),
        native_generation.as_ref().map(|n| n.pages.as_slice()),
        b,
    )?;
    Ok(Generated {
        files,
        manifest,
        math_generation: generated,
        native_generation,
    })
}
