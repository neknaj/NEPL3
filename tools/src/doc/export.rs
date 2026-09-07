//! Script-free local Doc export, using the same production pipeline as tests.
pub mod pages;
use super::source::{Compiled, budget, compiled, err, with_input_route};
use nepl3_core::source::{Digest, SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower};
use nepl3_doc_html::{ParallelMode, RenderOptions, prepare_local, render};
use nepl3_wire::foundation::FoundationCodec;
use std::{fs, io::Read, path::Path};

pub const CSS: &str = nepl3_doc_html::STYLESHEET;
/// Host input cap, also checked for callers supplying an in-memory source.
pub const MAX_SOURCE_BYTES: u64 = 10_000_000;

pub struct LocalDocument {
    pub html: String,
    pub manifest: String,
}

pub fn generate(compiled: &Compiled, input: &str) -> Result<LocalDocument, String> {
    if input.len() as u64 > MAX_SOURCE_BYTES {
        return Err("SourceLimit".into());
    }
    with_input_route(true, compiled, input, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let parse_usage = b.usage();
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let mut lower_budget = budget();
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut lower_budget,
            &mut codec,
        )
        .map_err(err)?;
        let mut output_budget = budget();
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let prepared = prepare_local(
            &doc,
            &options,
            profile.registry(),
            &mut codec,
            &mut output_budget,
        )
        .map_err(err)?;
        let rendered = render(&prepared, &mut output_budget).map_err(err)?;
        let html = shell(&rendered, &mut output_budget)?;
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
        let manifest = serde_json::to_string_pretty(&serde_json::json!({
            "format":"nepl3.local-doc-export/1",
            "scope":"local Doc only; external pages, assets and foreign rendering require resolution",
            "source_sha256":digest(input.as_bytes()),
            "profile_sha256":digest_hex(profile.digest()),
            "doc_schema_sha256":digest_hex(compiled.doc.package.schema.digest),
            "renderer":"nepl3-doc-html local/1",
            "options":{"parallel":"Rows"},
            "files":[{"path":"document.html","mime":"text/html; charset=utf-8","sha256":digest(html.as_bytes())},
                     {"path":"assets/doc.css","mime":"text/css; charset=utf-8","license":"MIT","sha256":digest(CSS.as_bytes())}],
            "operations":{"parse_and_validate":usage(parse_usage),"lower":usage(lower_budget.usage()),
                "prepare_render_serialize":usage(output_budget.usage())},
            "budget_scope":"Separate bounded operations; not a single end-to-end budget",
            "packages":"compiled checked bootstrap fixtures",
            "viewer_scripts":false
        })).map_err(err)? + "\n";
        Ok(LocalDocument { html, manifest })
    })
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
        if let HtmlNode::Element { children, .. } = &fragment.nodes[node as usize] {
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
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let source = read_source(input)?;
    let generated = generate(&compiled()?, &source)?;
    fs::create_dir(output)?;
    fs::create_dir(output.join("assets"))?;
    fs::write(output.join("assets/doc.css"), CSS.as_bytes())?;
    fs::write(output.join("document.html"), generated.html.as_bytes())?;
    // This is written last. Missing manifest means the output is incomplete.
    fs::write(output.join("manifest.json"), generated.manifest.as_bytes())?;
    Ok(())
}

fn shell(
    rendered: &nepl3_doc_html::RenderedFragment,
    output_budget: &mut nepl3_core::budget::Budget,
) -> Result<String, String> {
    let m = &rendered.markup;
    let checked =
        nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, output_budget).map_err(err)?;
    check_shell_depth(&m.fragment, output_budget)?;
    let fragment = nepl3_markup::html::serialize(&checked, output_budget).map_err(err)?;
    let head = "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'self'; base-uri 'none'; form-action 'none'\"><title>NEPL3 Doc</title><link rel=\"stylesheet\" href=\"assets/doc.css\"></head><body>\n";
    let tail = "\n</body></html>\n";
    output_budget
        .charge(
            nepl3_core::budget::Resource::OutputBytes,
            (head.len() + tail.len() + CSS.len()) as u64,
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
