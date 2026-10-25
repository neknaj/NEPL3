//! Script-free local Doc export, using the same production pipeline as tests.
pub mod assets;
mod code;
mod katex;
pub mod math;
pub mod pages;
pub use pages::{DisplayOptions, DisplaySelection};
mod stylesheet;
pub use super::math::display::Preference as MathRenderer;
use super::source::{Compiled, budget, compiled, err, with_named_input_limits};
use nepl3_core::source::{Digest, SourceAdmission, SourceStore};
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::check::Category;
use nepl3_doc_html::ParallelMode;
use nepl3_wire::foundation::FoundationCodec;
use pages::resources::ExecutionLimits;
use std::time::{Duration, Instant};
use std::{fs, io::Read, path::Path};
pub use stylesheet::CssMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// Elapsed includes native profile/provider setup and resolution. Usage is
    /// the parsing/validation Budget only; setup uses separate budgets.
    ParseAndValidate,
    /// Includes creation of the codec and its source admission.
    Lower,
    Prepare,
    /// Ends after HTML shell serialization; includes the inline CSP hash; excludes manifest/file-hash generation
    /// and destruction of intermediate values.
    RenderAndSerialize,
}

#[derive(Clone, Copy, Debug)]
pub struct StageMeasurement {
    pub stage: Stage,
    pub elapsed: Duration,
    /// Cumulative logical usage in this operation's Budget. Prepare and render
    /// share one Budget; compare their snapshots rather than summing them.
    pub usage: nepl3_core::budget::Usage,
}

pub const CSS: &str = nepl3_doc_html::STYLESHEET;
/// Host input cap, also checked for callers supplying an in-memory source.
pub const MAX_SOURCE_BYTES: u64 = 10_000_000;

pub struct LocalDocument {
    pub html: String,
    pub manifest: String,
    pub stylesheet: String,
    pub document: nepl3_doc_core::model::DocumentSyntax,
    pub math: Vec<math::Occurrence>,
    pub math_generation: Vec<math::generation::Occurrence>,
    pub sentence_sources: Option<super::annotations::article::SelectionParts>,
    pub sentence_alt_sources: Option<super::annotations::alt::Projection>,
    pub sentences: Vec<super::annotations::article::render::SentenceRecord>,
    pub rendered: nepl3_doc_html::RenderedFragment,
}

pub fn generate(compiled: &Compiled, input: &str) -> Result<LocalDocument, String> {
    generate_with_css(compiled, input, CssMode::External)
}

/// Generate a local document with an explicit stylesheet packaging mode.
pub fn generate_with_css(
    compiled: &Compiled,
    input: &str,
    css: CssMode,
) -> Result<LocalDocument, String> {
    generate_observed_with_css(compiled, input, css, &mut |_| {})
}

/// Observe successful stage boundaries in the production export pipeline.
/// Timing stays in the host callback and never enters generated artifacts.
/// An error returns normally; no measurement claims completion of that stage.
/// These timings are stage intervals, not complete export latency. Observer
/// execution is excluded from the following interval.
pub fn generate_observed(
    compiled: &Compiled,
    input: &str,
    observe: &mut impl FnMut(StageMeasurement),
) -> Result<LocalDocument, String> {
    generate_observed_with_css(compiled, input, CssMode::External, observe)
}

/// Observe the same pipeline while selecting local stylesheet packaging.
pub fn generate_observed_with_css(
    compiled: &Compiled,
    input: &str,
    css: CssMode,
    observe: &mut impl FnMut(StageMeasurement),
) -> Result<LocalDocument, String> {
    generate_impl(
        compiled,
        input,
        css,
        None,
        DisplayOptions::default(),
        ExecutionLimits::default(),
        observe,
    )
}
pub fn generate_with_renderer(
    compiled: &Compiled,
    input: &str,
    css: CssMode,
    renderer: MathRenderer,
) -> Result<LocalDocument, String> {
    generate_with_limits(compiled, input, css, renderer, ExecutionLimits::default())
}
pub fn generate_with_limits(
    compiled: &Compiled,
    input: &str,
    css: CssMode,
    renderer: MathRenderer,
    limits: ExecutionLimits,
) -> Result<LocalDocument, String> {
    generate_with_display(
        compiled,
        input,
        css,
        DisplayOptions {
            math_renderer: renderer,
            ..DisplayOptions::default()
        },
        limits,
    )
}

/// Asset-free Article export with explicit native display selection.
/// Existing wrappers retain Rows; selection is never inferred from locale.
pub fn generate_with_display(
    compiled: &Compiled,
    input: &str,
    css: CssMode,
    display: DisplayOptions,
    limits: ExecutionLimits,
) -> Result<LocalDocument, String> {
    generate_impl(compiled, input, css, None, display, limits, &mut |_| {})
}
fn generate_impl(
    compiled: &Compiled,
    input: &str,
    css: CssMode,
    assets: Option<(
        &[nepl3_doc_html::assets::SvgInput<'_>],
        nepl3_doc_html::assets::SvgMode,
    )>,
    display: DisplayOptions,
    limits: ExecutionLimits,
    observe: &mut impl FnMut(StageMeasurement),
) -> Result<LocalDocument, String> {
    if input.len() as u64 > MAX_SOURCE_BYTES {
        return Err("SourceLimit".into());
    }
    let DisplayOptions {
        document: options,
        math_renderer: renderer,
    } = display;
    let parse_start = Instant::now();
    with_named_input_limits(
        true,
        compiled,
        input,
        "doc-input",
        "Article",
        limits.phases().parse,
        |tree, profile, b, _a| {
            let checked = tree.syntax();
            let parse_usage = b.usage();
            observe(StageMeasurement {
                stage: Stage::ParseAndValidate,
                elapsed: parse_start.elapsed(),
                usage: parse_usage,
            });
            let lower_start = Instant::now();
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let mut lower_budget = limits.lower_limits.budget();
            let doc = nepl3_suite::adapters::sentence::literal::lower(
                checked,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut lower_budget,
                &mut codec,
            )
            .map_err(err)?;
            observe(StageMeasurement {
                stage: Stage::Lower,
                elapsed: lower_start.elapsed(),
                usage: lower_budget.usage(),
            });
            let mut output_budget = limits.output_limits.budget();
            if let Some((inputs, nepl3_doc_html::assets::SvgMode::External)) = assets {
                for (index, asset) in inputs.iter().enumerate() {
                    let mut duplicate = false;
                    for prior in &inputs[..index] {
                        output_budget
                            .charge(
                                nepl3_core::budget::Resource::Work,
                                asset.svg.len().min(prior.svg.len()) as u64 + 1,
                            )
                            .map_err(err)?;
                        duplicate |= asset.svg == prior.svg;
                    }
                    if !duplicate {
                        output_budget
                            .charge(
                                nepl3_core::budget::Resource::OutputBytes,
                                asset.svg.len() as u64,
                            )
                            .map_err(err)?;
                        output_budget
                            .charge(
                                nepl3_core::budget::Resource::AllocationUnits,
                                asset.svg.len() as u64,
                            )
                            .map_err(err)?;
                    }
                }
            }
            let prepare_start = Instant::now();
            enum Prepared<'a> {
                Local(nepl3_doc_html::display::PreparedDisplayArticle<'a>),
                Svg(nepl3_doc_html::assets::PreparedSvgDisplayArticle<'a>),
                Sentence(super::annotations::article::Selection<'a>),
                SvgSentence {
                    prepared: nepl3_doc_html::assets::PreparedSvgDisplayArticle<'a>,
                    projection: super::annotations::alt::Projection,
                },
            }
            output_budget
                .charge(
                    nepl3_core::budget::Resource::Work,
                    doc.value.embeds.kinds.len() as u64,
                )
                .map_err(err)?;
            let has_sentence = doc
                .value
                .embeds
                .kinds
                .contains(&nepl3_doc_core::model::EmbedKind::Sentence);
            let prepared = if has_sentence {
                if let Some((inputs, mode)) = assets {
                    assets::sentence::check_alt_only(&doc, &mut output_budget)?;
                    let mut host = sentence_host(compiled, profile.registry(), &mut codec)?;
                    let projection =
                        super::annotations::alt::prepare(&doc, &mut host, &mut output_budget)
                            .map_err(err)?;
                    let prepared = nepl3_doc_html::assets::prepare_svg_display_with_text(
                        &doc,
                        &options,
                        inputs,
                        mode,
                        &projection.resolved,
                        profile.registry(),
                        &mut codec,
                        &mut output_budget,
                    )
                    .map_err(err)?;
                    Prepared::SvgSentence {
                        prepared,
                        projection,
                    }
                } else {
                    let mut host = sentence_host(compiled, profile.registry(), &mut codec)?;
                    Prepared::Sentence(
                        super::annotations::article::collect_structural(
                            &doc,
                            &mut host,
                            &mut output_budget,
                        )
                        .map_err(err)?,
                    )
                }
            } else if let Some((inputs, mode)) = assets {
                Prepared::Svg(
                    nepl3_doc_html::assets::prepare_svg_display(
                        &doc,
                        &options,
                        inputs,
                        mode,
                        profile.registry(),
                        &mut codec,
                        &mut output_budget,
                    )
                    .map_err(err)?,
                )
            } else {
                Prepared::Local(
                    nepl3_doc_html::display::prepare_display(
                        &doc,
                        &options,
                        profile.registry(),
                        &mut codec,
                        &mut output_budget,
                    )
                    .map_err(err)?,
                )
            };
            observe(StageMeasurement {
                stage: Stage::Prepare,
                elapsed: prepare_start.elapsed(),
                usage: output_budget.usage(),
            });
            let render_start = Instant::now();
            let mut math = Vec::new();
            let mut sentences = Vec::new();
            let mut adapter = |embed: &nepl3_doc_core::model::DocEmbedView<'_>,
                               index,
                               codec: &mut nepl3_wire::foundation::FoundationCodec<'_>,
                               b: &mut nepl3_core::budget::Budget| {
                if embed.kind == nepl3_doc_core::model::EmbedKind::Code {
                    code::render(embed, tree, profile, codec.source_admission(), b)
                } else {
                    math::DisplayHost {
                        compiled,
                        registry: profile.registry(),
                        codec,
                        preference: renderer,
                    }
                    .render(embed, index, &mut math, b)
                }
            };
            let rendered = match &prepared {
                Prepared::Local(p) => nepl3_doc_html::display::render_display(
                    p,
                    &mut |guest, index, b| adapter(guest, index, &mut codec, b),
                    &mut output_budget,
                )
                .map_err(err)?,
                Prepared::Svg(p) | Prepared::SvgSentence { prepared: p, .. } => {
                    nepl3_doc_html::assets::render_svg_display(
                        p,
                        &mut |guest, index, b| adapter(guest, index, &mut codec, b),
                        &mut output_budget,
                    )
                    .map_err(err)?
                }
                Prepared::Sentence(input) => {
                    let mut host = sentence_host(compiled, profile.registry(), &mut codec)?;
                    let output = super::annotations::article::render::render(
                        input,
                        &options,
                        &mut host,
                        &mut adapter,
                        &mut output_budget,
                    )
                    .map_err(err)?;
                    sentences = output.sentences;
                    output.article
                }
            };
            math::compose(
                &mut math,
                &rendered.foreign,
                &doc.value.embeds,
                &mut output_budget,
            )?;
            let rendered = rendered.fragment;
            let mut identity = format!(
                "{}:{}:{}",
                digest_hex(Digest::of(input.as_bytes())),
                digest_hex(profile.digest()),
                renderer.as_str()
            );
            let display_scope = if !matches!(options.parallel, ParallelMode::Rows) {
                // This additional seed hash and scope suffix are generation
                // work, unlike the later manifest serialization.
                output_budget
                    .charge(nepl3_core::budget::Resource::Work, identity.len() as u64)
                    .map_err(err)?;
                let selected = pages::display::bind_identity(
                    b"nepl3.local-doc-export.display-scope/1\0",
                    Digest::of(identity.as_bytes()),
                    &options.parallel,
                    &mut output_budget,
                )?;
                let extended = (identity.len() as u64).checked_add(65).ok_or_else(|| {
                    err(output_budget.stop(nepl3_core::budget::StopReason::AllocationLimit))
                })?;
                let allocation = extended.checked_mul(2).ok_or_else(|| {
                    err(output_budget.stop(nepl3_core::budget::StopReason::AllocationLimit))
                })?;
                output_budget
                    .charge(nepl3_core::budget::Resource::Work, extended)
                    .map_err(err)?;
                output_budget
                    .charge(nepl3_core::budget::Resource::AllocationUnits, allocation)
                    .map_err(err)?;
                identity.try_reserve_exact(65).map_err(|_| {
                    err(output_budget.stop(nepl3_core::budget::StopReason::AllocationLimit))
                })?;
                identity.push(':');
                let digits = b"0123456789abcdef";
                for byte in selected.0 {
                    identity.push(digits[(byte >> 4) as usize] as char);
                    identity.push(digits[(byte & 15) as usize] as char);
                }
                Some(selected)
            } else {
                None
            };
            let mut generation =
                math::generation::collect(&math, &sentences, renderer, &mut output_budget)?;
            let katex = katex::generate(&mut generation, &identity, &mut output_budget)?;
            math::generation::settle(&mut math, &generation, &mut output_budget)?;
            let mut document_css =
                assets::document_css(&doc, assets.map(|(inputs, _)| inputs), &mut output_budget)?;
            let html = shell_with_assets(
                &rendered,
                css,
                assets.map(|(_, mode)| mode),
                &mut document_css,
                katex.as_ref(),
                &mut output_budget,
            )?;
            observe(StageMeasurement {
                stage: Stage::RenderAndSerialize,
                elapsed: render_start.elapsed(),
                usage: output_budget.usage(),
            });
            let digest = |bytes: &[u8]| {
                Digest::of(bytes)
                    .0
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            };
            let usage = |u: nepl3_core::budget::Usage| {
                serde_json::json!({
                    "work":u.work,"source_bytes":u.source_bytes,"allocation_units":u.allocation_units,
                    "nodes":u.nodes,"depth":u.depth,"output_bytes":u.output_bytes,
                    "diagnostics":u.diagnostics,"events":u.events
                })
            };
            let mut files = vec![
                serde_json::json!({"path":"document.html","mime":"text/html; charset=utf-8","sha256":digest(html.as_bytes())}),
            ];
            if css == CssMode::External {
                files.push(serde_json::json!({"path":"assets/doc.css","mime":"text/css; charset=utf-8","license":"MIT","sha256":digest(document_css.as_bytes())}));
            }
            let mut display_options = serde_json::json!({
                "parallel": match &options.parallel { ParallelMode::Rows => "Rows", ParallelMode::Columns => "Columns", ParallelMode::Single { .. } => "Single" },
                "css": css.as_str(), "math_renderer": renderer.as_str()
            });
            if let ParallelMode::Single {
                language,
                fallbacks,
            } = &options.parallel
            {
                display_options["language"] = serde_json::json!(language);
                display_options["fallbacks"] = serde_json::json!(fallbacks);
            }
            let mut manifest_value = serde_json::json!({
                "format":"nepl3.local-doc-export/1",
                "scope":if matches!(options.parallel, ParallelMode::Rows) { "Doc with retained Code, direct/nested Math, and asset-free typed Sentence display in Rows mode; validated external URLs; page/relative links, typed Sentence PageSet routing, and Sentence assets remain unsupported" } else { "Doc with retained Code, direct/nested Math, and asset-free typed Sentence display using explicitly selected native options; validated external URLs; page/relative links, typed Sentence PageSet routing, and Sentence assets remain unsupported" },
                "source_sha256":digest(input.as_bytes()),
                "profile_sha256":digest_hex(profile.digest()),
                "doc_schema_sha256":digest_hex(compiled.doc.package.schema.digest),
                "renderer":"nepl3-doc-html local/1",
                "options":display_options,
                "math_diagnostics":math::diagnostics(&generation),
                "katex":katex.as_ref().map(|k| serde_json::json!({"version":"0.18.7","generation":"native pinned worker","identity":k.identity(),"viewer_scripts":false,"fonts":"all fixed fonts embedded in CSS","license":"complete original license embedded in CSS","host_internal_work":"unobserved","host_internal_allocation":"unobserved","visuals":k.visuals.iter().map(|v| serde_json::json!({"math_arena_root":v.root,"scope":v.scope})).collect::<Vec<_>>(),"resources":k.files.iter().map(|f|serde_json::json!({"path":f.path,"mime":f.mime,"sha256":digest(&f.bytes),"bytes":f.bytes.len(),"placement":"embedded stylesheet"})).collect::<Vec<_>>()})),
                "stylesheet":{"sha256":digest(document_css.as_bytes()),"license":"MIT"},
                "font":{"family":"Klee One","weights":[400,600],"stylesheet":stylesheet::FONT_STYLESHEET,"bundled":false,"offline":"system fallback"},
                "files":files,
                "operations":{"parse_and_validate":usage(parse_usage),"lower":usage(lower_budget.usage()),
                    "prepare_render_serialize":usage(output_budget.usage())},
                "limits":limits,
                "budget_scope":"Separate bounded operations; not a single end-to-end budget",
                "packages":"compiled checked bootstrap fixtures",
                "viewer_scripts":false
            });
            if let Some(scope) = display_scope {
                manifest_value["display_scope"] = serde_json::json!({
                    "contract":"nepl3.local-doc-export.display-scope/1",
                    "identity":digest_hex(scope)
                });
            }
            if let Prepared::SvgSentence { projection, .. } = &prepared {
                manifest_value["scope"] = serde_json::json!(
                    "Doc SVG with independent Sentence restricted to image alt; BaseOnly native text projection, retained Code and Math; non-alt Sentence assets, PageSet assets and namespace composition remain unsupported"
                );
                manifest_value["sentence_alt"] = serde_json::json!({
                    "policy": "BaseOnly", "owners": projection.sentences.len(),
                    "scope": "native text projection; no HTML namespace adoption or portable proof"
                });
            }
            let manifest = serde_json::to_string_pretty(&manifest_value).map_err(err)? + "\n";
            let (sentence_sources, sentence_alt_sources) = match prepared {
                Prepared::Sentence(input) => (Some(input.into_parts()), None),
                Prepared::SvgSentence { projection, .. } => (None, Some(projection)),
                _ => (None, None),
            };
            Ok(LocalDocument {
                math_generation: generation,
                sentence_sources,
                sentence_alt_sources,
                sentences,
                html,
                manifest,
                stylesheet: document_css,
                document: doc,
                math,
                rendered,
            })
        },
    )
}

/// Called only after fragment validation; account for html/body ancestors.
fn check_shell_depth(
    fragment: &nepl3_markup::html::HtmlFragment,
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::Resource;
    use nepl3_markup::html::HtmlNode;
    let mut pending = Vec::new();
    push_pending(&mut pending, (fragment.root, 3), budget)?;
    while let Some((node, depth)) = pending.pop() {
        budget.charge(Resource::Work, 1).map_err(err)?;
        if depth > 256 {
            return Err(format!(
                "OutputDepth {{ element: {node}, shell_depth: {depth} }}"
            ));
        }
        budget.observe_depth(depth).map_err(err)?;
        if let HtmlNode::Element { children, .. } | HtmlNode::MathElement { children, .. } =
            &fragment.nodes[node as usize]
        {
            for child in children.iter().rev() {
                push_pending(&mut pending, (*child, depth + 1), budget)?;
            }
        }
    }
    Ok(())
}

fn push_pending(
    pending: &mut Vec<(u64, u64)>,
    value: (u64, u64),
    budget: &mut nepl3_core::budget::Budget,
) -> Result<(), String> {
    use nepl3_core::budget::{Resource, StopReason};
    budget.charge(Resource::Work, 1).map_err(err)?;
    if pending.len() == pending.capacity() {
        let size = std::mem::size_of::<(u64, u64)>();
        let Some((capacity, bytes)) = pending
            .capacity()
            .checked_mul(2)
            .map(|v| v.max(1))
            .and_then(|c| c.checked_mul(size).map(|bytes| (c, bytes)))
        else {
            return Err(err(budget.stop(StopReason::AllocationLimit)));
        };
        budget
            .charge(Resource::AllocationUnits, bytes as u64)
            .map_err(err)?;
        pending.reserve_exact(capacity - pending.len());
    }
    pending.push(value);
    Ok(())
}

fn digest_hex(value: Digest) -> String {
    value.0.iter().map(|b| format!("{b:02x}")).collect()
}

fn read_source(input: &Path) -> crate::Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(input)?
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err("SourceLimit".into());
    }
    Ok(String::from_utf8(bytes)?)
}

/// Write a new output directory. Existing output is never silently overwritten.
pub fn write(input: &Path, output: &Path) -> crate::Result<()> {
    write_with_css(input, output, CssMode::External)
}

/// Write the selected local export, preserving the no-overwrite contract.
pub fn write_with_css(input: &Path, output: &Path, css: CssMode) -> crate::Result<()> {
    write_with_renderer(input, output, css, MathRenderer::KaTeXPreferred)
}
pub fn write_with_renderer(
    input: &Path,
    output: &Path,
    css: CssMode,
    renderer: MathRenderer,
) -> crate::Result<()> {
    write_with_limits(input, output, css, renderer, ExecutionLimits::default())
}
pub fn write_with_limits(
    input: &Path,
    output: &Path,
    css: CssMode,
    renderer: MathRenderer,
    limits: ExecutionLimits,
) -> crate::Result<()> {
    write_with_display(
        input,
        output,
        css,
        DisplayOptions {
            math_renderer: renderer,
            ..DisplayOptions::default()
        },
        limits,
    )
}

/// Explicit asset-free display export; preserve the completion-marker contract.
pub fn write_with_display(
    input: &Path,
    output: &Path,
    css: CssMode,
    display: DisplayOptions,
    limits: ExecutionLimits,
) -> crate::Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let source = read_source(input)?;
    let generated = generate_with_display(&compiled()?, &source, css, display, limits)?;
    fs::create_dir(output)?;
    if css == CssMode::External {
        fs::create_dir(output.join("assets"))?;
        fs::write(
            output.join("assets/doc.css"),
            generated.stylesheet.as_bytes(),
        )?;
    }
    fs::write(output.join("document.html"), generated.html.as_bytes())?;
    // This is written last. Missing manifest means the output is incomplete.
    fs::write(output.join("manifest.json"), generated.manifest.as_bytes())?;
    Ok(())
}

fn shell_with_assets(
    rendered: &nepl3_doc_html::RenderedFragment,
    css: CssMode,
    assets: Option<nepl3_doc_html::assets::SvgMode>,
    document_css: &mut String,
    katex: Option<&katex::Generated>,
    output_budget: &mut nepl3_core::budget::Budget,
) -> Result<String, String> {
    let m = &rendered.markup;
    let validation_start = output_budget.usage();
    let checked = nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, output_budget)
        .map_err(|e| {
            format!(
                "HTML validation: {e:?}; initial usage={validation_start:?}; arena nodes={}",
                m.fragment.nodes.len()
            )
        })?;
    check_shell_depth(&m.fragment, output_budget)
        .map_err(|e| format!("{e}; phase=HTML shell depth"))?;
    let (fragment, precharged_css) = if let Some(katex) = katex {
        let result = katex.compose(&checked, output_budget)?;
        let fonts = katex.stylesheet(output_budget)?;
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                (result.stylesheet.len() + fonts.len()) as u64,
            )
            .map_err(err)?;
        let precharged_css = result.stylesheet.len();
        document_css.push_str(&fonts);
        document_css.push_str(&result.stylesheet);
        (result.html, precharged_css)
    } else {
        (
            nepl3_markup::html::serialize(&checked, output_budget)
                .map_err(|e| format!("HTML serialization: {e:?}"))?,
            0,
        )
    };
    let head = stylesheet::head_with_css(css, document_css, output_budget)?;
    let head = if katex.is_some() {
        let rule = "font-src data: https://fonts.gstatic.com; style-src-attr 'none'";
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                (head.len() + rule.len()) as u64,
            )
            .map_err(err)?;
        std::borrow::Cow::Owned(head.replace("font-src https://fonts.gstatic.com", rule))
    } else {
        head
    };
    let head = if let Some(mode) = assets {
        use nepl3_doc_html::assets::SvgMode;
        let rule = match mode {
            SvgMode::External => "img-src 'self'; ",
            SvgMode::Embedded => "img-src data:; ",
        };
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                (head.len() + rule.len()) as u64,
            )
            .map_err(err)?;
        std::borrow::Cow::Owned(head.replacen(
            "default-src 'none'; ",
            &format!("default-src 'none'; {rule}"),
            1,
        ))
    } else {
        head
    };
    let tail = "\n</body></html>\n";
    output_budget
        .charge(
            nepl3_core::budget::Resource::OutputBytes,
            (head.len()
                + tail.len()
                + if css == CssMode::External {
                    document_css.len()
                } else {
                    0
                }
                - precharged_css) as u64,
        )
        .map_err(err)?;
    output_budget
        .charge(
            nepl3_core::budget::Resource::AllocationUnits,
            (head.len() + fragment.len() + tail.len()) as u64,
        )
        .map_err(err)?;
    let html = format!("{head}{fragment}{tail}");
    Ok(html)
}

fn sentence_host<'a, C: nepl3_core::value_codec::FoundationValueCodec>(
    compiled: &'a Compiled,
    registry: &'a nepl3_core::schema::SchemaRegistry,
    codec: &'a mut C,
) -> Result<super::annotations::SentenceAnnotationRenderer<'a, C>, String> {
    Ok(super::annotations::SentenceAnnotationRenderer {
        registry,
        surface: &compiled.others.get(3).ok_or("SentenceSelection")?.schema,
        math_surface: Some(&compiled.others.first().ok_or("MathSelection")?.schema),
        doc_surface: Some(&compiled.doc.package.schema),
        codec,
    })
}
