//! Shared rendering of an immutable, already prepared page set.
use super::*;

pub(super) struct Context<'a> {
    pub compiled: &'a Compiled,
    pub request: &'a PagesHtmlRequest,
    pub page_code: &'a [code::PreparedPageCode],
    pub profiles: &'a [Digest],
    /// Retained preparation evidence uses its own identity contract.
    pub retained_preparation: Option<&'a serde_json::Value>,
    pub origins: &'a [serde_json::Value],
    pub file_origins: &'a [serde_json::Value],
    pub phases: resources::PhaseLimits,
    pub initial_usage: nepl3_core::budget::Usage,
    pub renderer: super::super::MathRenderer,
    pub aliases: &'a [aliases::PageAliases],
}
pub(super) struct RenderedOutput {
    sentence_sources: Vec<super::super::super::annotations::article::SelectionParts>,
    sentences: Vec<Vec<super::super::super::annotations::article::render::SentenceRecord>>,
    files: BTreeMap<String, Vec<u8>>,
    manifest: String,
    math: Vec<Vec<super::super::math::Occurrence>>,
    math_generation: Vec<Vec<super::super::math::generation::Occurrence>>,
    rendered: nepl3_doc_html::pages::RenderedPages,
}
impl RenderedOutput {
    pub(super) fn finish(
        self,
        request: PagesHtmlRequest,
        aliases: &[aliases::PageAliases],
    ) -> Result<GeneratedPages, String> {
        let mut generated = GeneratedPages {
            files: self.files,
            manifest: self.manifest,
            provenance: Some(DisplayProvenance {
                sentence_sources: self.sentence_sources,
                sentences: self.sentences,
                pages: request.set,
                math: self.math,
                math_generation: self.math_generation,
                rendered: self.rendered,
            }),
        };
        aliases::record(&mut generated, aliases)?;
        Ok(generated)
    }
}
pub(super) fn render(
    context: Context<'_>,
    output_budget: &mut Budget,
) -> Result<RenderedOutput, String> {
    let Context {
        compiled,
        request,
        page_code,
        profiles,
        retained_preparation,
        origins,
        file_origins,
        phases,
        initial_usage,
        renderer,
        aliases,
    } = context;
    let r = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    let mut page_math = Vec::new();
    for _ in &request.set.pages {
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                core::mem::size_of::<Vec<super::super::math::Occurrence>>() as u64,
            )
            .map_err(err)?;
        page_math.try_reserve_exact(1).map_err(|_| {
            err(output_budget.stop(nepl3_core::budget::StopReason::AllocationLimit))
        })?;
        page_math.push(Vec::new());
    }
    let mut has_sentence = false;
    for page in &request.set.pages {
        output_budget
            .charge(
                nepl3_core::budget::Resource::Work,
                page.document.value.embeds.kinds.len() as u64,
            )
            .map_err(err)?;
        has_sentence |= page
            .document
            .value
            .embeds
            .kinds
            .contains(&nepl3_doc_core::model::EmbedKind::Sentence);
    }
    let mut sentence_sources = Vec::new();
    let mut sentences = Vec::new();
    let displayed = if has_sentence {
        let output = sentence::render(
            compiled,
            request,
            page_code,
            &mut page_math,
            renderer,
            output_budget,
        )?;
        sentence_sources = output.sources;
        sentences = output.records;
        output.displayed
    } else {
        render_pages_with_display(
            request,
            r,
            &mut c,
            output_budget,
            &mut |page, embed, index, codec, b| {
                let code = usize::try_from(page)
                    .ok()
                    .and_then(|page| page_code.get(page))
                    .ok_or("CodePageMissing")?;
                if embed.kind == nepl3_doc_core::model::EmbedKind::Code {
                    code.render(embed, index, r, b)
                } else {
                    let records = page_math.get_mut(page as usize).ok_or("MathPageMissing")?;
                    super::super::math::DisplayHost {
                        compiled,
                        registry: r,
                        codec,
                        preference: renderer,
                    }
                    .render(embed, index, records, b)
                }
            },
        )
        .map_err(|e| format!("resolve/render: {e:?}; usage={:?}", output_budget.usage()))?
    };
    for ((records, placements), page) in page_math
        .iter_mut()
        .zip(&displayed.foreign)
        .zip(&request.set.pages)
    {
        super::super::math::compose(
            records,
            placements,
            &page.document.value.embeds,
            output_budget,
        )?;
    }
    let mut page_generation = Vec::new();
    for (page, records) in page_math.iter().enumerate() {
        let generated = super::super::math::generation::collect(
            records,
            sentences.get(page).map(Vec::as_slice).unwrap_or(&[]),
            renderer,
            output_budget,
        )?;
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                (2 * core::mem::size_of_val(&generated)) as u64,
            )
            .map_err(err)?;
        page_generation.try_reserve(1).map_err(|_| {
            err(output_budget.stop(nepl3_core::budget::StopReason::AllocationLimit))
        })?;
        page_generation.push(generated);
    }
    let mut rendered = displayed.pages;
    for (page, fragment) in request.set.pages.iter().zip(&mut rendered.fragments) {
        for input in aliases {
            output_budget
                .charge(
                    nepl3_core::budget::Resource::Work,
                    (input.page.len() + page.registration.id.len() + 1) as u64,
                )
                .map_err(err)?;
            if input.page == page.registration.id {
                aliases::apply(fragment, &input.values, output_budget)?;
                break;
            }
        }
    }
    let rendered_usage = output_budget.usage();
    let mut files = BTreeMap::new();
    let mut file_kinds = BTreeMap::new();
    for file in &request.set.files {
        output_budget
            .charge(
                nepl3_core::budget::Resource::OutputBytes,
                file.content.0.len() as u64,
            )
            .map_err(err)?;
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                file.content.0.len() as u64,
            )
            .map_err(err)?;
        insert(
            &mut files,
            file.registration.route.clone(),
            file.content.0.clone(),
        )?;
        file_kinds.insert(file.registration.route.clone(), "application/octet-stream");
    }
    let mut katex_pages = Vec::new();
    let mut native_cache = None;
    for (page_index, (page, fragment)) in request
        .set
        .pages
        .iter()
        .zip(&rendered.fragments)
        .enumerate()
    {
        let route = &page.registration.route;
        if !route.ends_with(".html") {
            return Err("HTML route must end in .html".into());
        }
        let identity = format!(
            "{}:{}:{}",
            digest_hex(rendered.identity),
            page.registration.id,
            renderer.as_str()
        );
        let native = super::super::katex::generate_cached(
            &mut page_generation[page_index],
            &identity,
            native_cache.as_ref(),
            output_budget,
        )?;
        super::super::math::generation::settle(
            &mut page_math[page_index],
            &page_generation[page_index],
            output_budget,
        )?;
        let mut document_css = CSS.to_owned();
        let html = shell_with_assets(
            fragment,
            CssMode::External,
            None,
            &mut document_css,
            native.as_ref(),
            output_budget,
        )
        .map_err(|e| {
            format!(
                "page {route}: {e}; render completed usage={rendered_usage:?}; usage={:?}",
                output_budget.usage()
            )
        })?;
        let css_name = if native.is_some() {
            format!("doc-{}.css", digest_hex(Digest::of(route.as_bytes())))
        } else {
            "doc.css".into()
        };
        let html = if native.is_some() {
            output_budget
                .charge(
                    nepl3_core::budget::Resource::AllocationUnits,
                    html.len() as u64 + css_name.len() as u64,
                )
                .map_err(err)?;
            output_budget
                .charge(nepl3_core::budget::Resource::Work, html.len() as u64)
                .map_err(err)?;
            output_budget
                .charge(
                    nepl3_core::budget::Resource::OutputBytes,
                    (css_name.len() - "doc.css".len()) as u64,
                )
                .map_err(err)?;
            html.replacen(
                "href=\"assets/doc.css\"",
                &format!("href=\"assets/{css_name}\""),
                1,
            )
        } else {
            html
        };
        insert(&mut files, route.clone(), html.into_bytes())?;
        file_kinds.insert(route.clone(), "text/html; charset=utf-8");
        let css = match route.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/assets/{css_name}"),
            None => format!("assets/{css_name}"),
        };
        katex_pages.push(native.as_ref().map(|k| serde_json::json!({"version":"0.18.7","identity":k.identity(),"fonts":"all fixed fonts embedded in CSS","license":"complete original license embedded in CSS","visuals":k.visuals.iter().map(|v|serde_json::json!({"math_arena_root":v.root,"scope":v.scope})).collect::<Vec<_>>(),"host_internal_work":"unobserved","host_internal_allocation":"unobserved"})));
        if request
            .set
            .files
            .iter()
            .any(|file| conflict(&file.registration.route, &css))
        {
            return Err("registered file conflicts with generated stylesheet".into());
        }
        insert(&mut files, css.clone(), document_css.into_bytes())?;
        file_kinds.insert(css, "text/css; charset=utf-8");
        if native.is_some() {
            native_cache = native;
        }
    }
    // Reserve the completion marker before writing anything to disk.
    if files.keys().any(|path| conflict(path, "manifest.json")) {
        return Err("reserved manifest path".into());
    }
    let records = files
        .iter()
        .map(|(path, bytes)| {
            serde_json::json!({"path":path,"sha256":digest_hex(Digest::of(bytes)),
        "mime":file_kinds[path],"license":if path.ends_with(".css") {Some("MIT")} else {None}})
        })
        .collect::<Vec<_>>();
    let output_identity = resources::execution_identity(
        rendered.identity,
        output_budget.limits(),
        initial_usage,
        renderer,
    );
    let output_identity = if matches!(request.options.parallel, ParallelMode::Rows) {
        output_identity
    } else {
        super::display::bind_identity(
            b"nepl3.local-doc-pages.display-execution/1\0",
            output_identity,
            &request.options.parallel,
            output_budget,
        )?
    };
    let phase_execution = if let Some(evidence) = retained_preparation {
        let mut encoded = Vec::from(b"nepl3.local-doc-pages.retained-preparation/1\0".as_slice());
        encoded.extend_from_slice(&output_identity.0);
        encoded.extend_from_slice(&serde_json::to_vec(evidence).map_err(err)?);
        serde_json::json!({"contract":"nepl3.local-doc-pages.retained-preparation/1",
            "identity":digest_hex(Digest::of(&encoded)), "evidence":evidence})
    } else {
        serde_json::json!({"contract":"nepl3.local-doc-pages.phases/1",
            "identity":digest_hex(resources::phase_identity(output_identity,profiles,phases))})
    };
    let mut display_options = serde_json::json!({"parallel":match &request.options.parallel {ParallelMode::Rows=>"Rows",ParallelMode::Columns=>"Columns",ParallelMode::Single{..}=>"Single"},"math_renderer":renderer.as_str()});
    if let ParallelMode::Single {
        language,
        fallbacks,
    } = &request.options.parallel
    {
        display_options["language"] = serde_json::json!(language);
        display_options["fallbacks"] = serde_json::json!(fallbacks);
    }
    let manifest = serde_json::to_string_pretty(&serde_json::json!({
        "format":"nepl3.local-doc-pages/1","identity":digest_hex(rendered.identity),"pages":origins,"files":records,
        "execution_identity":digest_hex(output_identity),
        "phase_execution":phase_execution,
        "output_budget":{"contract":if matches!(request.options.parallel,ParallelMode::Rows){"nepl3.local-doc-pages.execution/2"}else{"nepl3.local-doc-pages.display-execution/1"},"limits":resources::limits(output_budget.limits()),
            "initial_usage":resources::usage(initial_usage),"usage":resources::usage(output_budget.usage())},
        "registered_files":file_origins,
        "renderer":if has_sentence {"nepl3-doc-html sentence-pages/1"} else {"nepl3-doc-html pages/4"},"options":display_options,
        "katex":katex_pages,
        "math_diagnostics":page_generation.iter().map(|records| super::super::math::diagnostics(records)).collect::<Vec<_>>(),"viewer_scripts":false,
        "packages":"compiled checked bootstrap fixtures","scope":if has_sentence {"Native all-occurrence typed Sentence namespaces with per-page retained owners, cross-page emitted-anchor checks and nested Math records. Rows/Columns or explicit Single display selection; no image-alt/asset selection or portable namespace proof. Code remains syntax-only; output does not prove network availability or deployment."} else {"Internal Doc page links and checked external http/https/mailto hrefs; no network or destination availability check. Retained Code uses shared syntax-only highlighting without guest evaluation; Math uses independent MathML with explicit renderer fallback diagnostics; assets and other foreign kinds remain unsupported. Not Pages deployment evidence."},
        "identity_contract":if has_sentence {if matches!(request.options.parallel,ParallelMode::Single{..}) {"nepl3.local-doc-pages.sentence-single/1: source digest, Single tag 2, UTF-8 language and ordered fallbacks with little-endian u64 lengths"} else {"nepl3.local-doc-pages.sentence-input/1 plus parallel mode"}} else {"legacy Doc PageSet"},
        "budget_scope":"Each parse/lower separately bounded; one shared resolve/render/serialize output budget",
        "output_usage":{"work":output_budget.usage().work,"allocation_units":output_budget.usage().allocation_units,"output_bytes":output_budget.usage().output_bytes}
    })).map_err(err)? + "\n";
    Ok(RenderedOutput {
        sentence_sources,
        sentences,
        files,
        manifest,
        math: page_math,
        math_generation: page_generation,
        rendered,
    })
}
