//! Bundle emission checks; independent browser qualification remains required.
use super::*;
use nepl3_tools::doc::math::document::{LocalDocument, bundle};

pub(super) fn check_bundle(source: &LocalDocument<'_, '_, '_, '_>) -> Result<(), String> {
    let mut b = budget();
    let bundled = source.materialize_bundle(&mut b).map_err(err)?;
    let usage = b.usage();
    assert!(core::ptr::eq(bundled.source(), source));
    assert!(bundled.html().starts_with("<!DOCTYPE html><html><head>"));
    assert!(bundled.html().ends_with("</body></html>"));
    assert!(
        bundled.html().contains(
            "default-src 'none'; style-src 'self'; style-src-attr 'none'; font-src 'self'"
        )
    );
    assert!(!bundled.html().contains("fonts.googleapis.com"));
    assert!(!bundled.html().contains("<script"));
    assert_eq!(bundled.doc_stylesheet(), nepl3_doc_html::STYLESHEET);
    assert_eq!(
        bundled.math_stylesheet(),
        source
            .math()
            .iter()
            .map(|m| m.stylesheet())
            .collect::<String>()
    );
    if source
        .math()
        .iter()
        .all(|m| m.termination_failure() != Some(true))
    {
        let mut gate_budget = budget();
        source
            .check_math_termination(&mut gate_budget)
            .map_err(err)?;
        assert_eq!(gate_budget.usage().work, source.math().len() as u64);
        assert_eq!(gate_budget.usage().allocation_units, 0);
        assert_eq!(gate_budget.usage().output_bytes, 0);
    }
    for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
        let mut limited = budget();
        if reason == StopReason::Cancelled {
            limited.cancel();
        } else {
            let mut limits = limited.limits();
            limits.work = 0;
            limited = Budget::new(limits);
        }
        if reason == StopReason::WorkLimit && source.math().is_empty() {
            continue;
        }
        assert!(matches!(source.check_math_termination(&mut limited),
            Err(nepl3_tools::doc::math::document::TerminationError::Stopped(s)) if s == reason));
        assert_eq!(limited.usage().allocation_units, 0);
        assert_eq!(limited.usage().output_bytes, 0);
    }
    let assets = source.math().iter().find_map(|m| m.assets());
    if let Some(assets) = assets {
        assert_eq!(bundled.katex_assets().len(), assets.files().len());
        assert!(
            bundled
                .html()
                .contains("href=\"assets/katex/katex.min.css\"")
        );
        assert!(bundled.html().contains("href=\"assets/math.css\""));
        for (got, expected) in bundled.katex_assets().iter().zip(assets.files()) {
            assert_eq!(got.path(), expected.path());
            assert_eq!(got.mime(), expected.mime());
            assert_eq!(got.digest(), expected.digest());
            assert_eq!(got.bytes(), expected.bytes());
        }
    } else {
        assert!(bundled.katex_assets().is_empty());
        assert!(bundled.math_stylesheet().is_empty());
        assert!(!bundled.html().contains("assets/katex/"));
        assert!(!bundled.html().contains("assets/math.css"));
    }
    let emitted = bundled.html().len()
        + bundled.doc_stylesheet().len()
        + bundled.math_stylesheet().len()
        + bundled
            .katex_assets()
            .iter()
            .map(|f| f.bytes().len())
            .sum::<usize>();
    assert_eq!(usage.output_bytes, emitted as u64);
    let second = source.materialize_bundle(&mut b).map_err(err)?;
    assert_eq!(second.html(), bundled.html());
    assert_eq!(second.math_stylesheet(), bundled.math_stylesheet());
    assert_eq!(b.usage().work, 2 * usage.work);
    assert_eq!(b.usage().allocation_units, 2 * usage.allocation_units);
    assert_eq!(b.usage().nodes, 2 * usage.nodes);
    assert_eq!(b.usage().output_bytes, 2 * usage.output_bytes);
    for mode in 0..8 {
        let mut limits = budget().limits();
        limits.work = usage.work - u64::from(mode == 1);
        limits.allocation_units = usage.allocation_units - u64::from(mode == 2);
        limits.nodes = usage.nodes - u64::from(mode == 3);
        limits.output_bytes = usage.output_bytes - u64::from(mode == 4);
        limits.depth = usage.depth + if mode >= 6 { 5 } else { 0 } - u64::from(mode == 7);
        let mut exact = Budget::new(limits);
        if mode == 5 {
            exact.cancel();
        }
        let result = if mode >= 6 {
            exact.with_depth_at_least(5, |b| source.materialize_bundle(b))
        } else {
            source.materialize_bundle(&mut exact)
        };
        if mode == 0 || mode == 6 {
            let result = result.map_err(err)?;
            assert_eq!(result.html(), bundled.html());
            assert_eq!(
                exact.usage().depth,
                usage.depth + if mode == 6 { 5 } else { 0 }
            );
        } else {
            let expected = match mode {
                1 => StopReason::WorkLimit,
                2 => StopReason::AllocationLimit,
                3 => StopReason::NodeLimit,
                4 => StopReason::OutputLimit,
                5 => StopReason::Cancelled,
                _ => StopReason::DepthLimit,
            };
            assert!(matches!(result, Err(bundle::Error::Stopped(s)) if s == expected));
            assert_eq!(exact.poll(), Err(expected));
        }
    }
    Ok(())
}

#[test]
fn bundle_rejects_unowned_code_resources_and_resolves_local_links() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower};
    use nepl3_doc_html::{ParallelMode, RenderOptions, guests};
    use nepl3_markup::html::{
        HtmlAttribute, HtmlFragment, HtmlHref, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot, HtmlTag,
    };
    use nepl3_tools::doc::math::{document, occurrence};
    let compiled = compiled()?;
    with_input(
        &compiled,
        "article en \"Bundle\" body cons code Math x nil",
        "Article",
        |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared =
                guests::prepare(&doc, &options, profile.registry(), &mut codec, b).map_err(err)?;
            for mode in 0..6 {
                let request = if mode == 0 {
                    HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![HtmlNode::Element {
                                tag: HtmlTag::Img,
                                attributes: vec![
                                    HtmlAttribute::Src {
                                        path: "missing.png".into(),
                                    },
                                    HtmlAttribute::Alt {
                                        value: "image".into(),
                                    },
                                ],
                                children: vec![],
                            }],
                        },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    }
                } else {
                    let href = match mode {
                        1 => HtmlHref::Artifact {
                            path: "other.html".into(),
                            fragment: None,
                        },
                        2 => HtmlHref::BetweenArtifacts {
                            source: "index.html".into(),
                            target: "other.html".into(),
                            fragment: None,
                        },
                        3 | 4 => HtmlHref::Fragment {
                            id: "target".into(),
                        },
                        _ => HtmlHref::External {
                            uri: "https://example.com/".into(),
                        },
                    };
                    HtmlRequest {
                        fragment: HtmlFragment {
                            root: 0,
                            nodes: vec![
                                HtmlNode::Element {
                                    tag: HtmlTag::A,
                                    attributes: vec![HtmlAttribute::Href { value: href }],
                                    children: vec![1],
                                },
                                HtmlNode::Element {
                                    tag: HtmlTag::Span,
                                    attributes: if mode == 4 {
                                        vec![HtmlAttribute::Id {
                                            value: "target".into(),
                                        }]
                                    } else {
                                        vec![]
                                    },
                                    children: vec![2],
                                },
                                HtmlNode::Text {
                                    text: "link".into(),
                                },
                            ],
                        },
                        slot: HtmlSlot::Phrasing,
                        policy: HtmlPolicy { classes: vec![] },
                    }
                };
                let mut request = Some(request);
                let doc = document::render(
                    &prepared,
                    0,
                    &mut |_, _| {
                        Err::<occurrence::Composed<'_, '_, '_>, String>("unexpected Math".into())
                    },
                    &mut |_, _| request.take().ok_or_else(|| "Code repeated".to_owned()),
                    &mut budget(),
                );
                if mode == 3 {
                    assert!(matches!(
                        doc,
                        Err(document::Error::Render(
                            nepl3_doc_html::RenderError::Markup(
                                nepl3_markup::html::HtmlError::MissingFragment(_)
                            )
                        ))
                    ));
                    continue;
                }
                let doc = doc.map_err(err)?;
                let mut operation = budget();
                let result = doc.materialize_bundle(&mut operation);
                if mode == 0 {
                    assert!(matches!(result, Err(bundle::Error::UnresolvedImage { .. })));
                } else if mode < 4 {
                    assert!(matches!(result, Err(bundle::Error::UnresolvedLink { .. })));
                } else {
                    let result = result.map_err(err)?;
                    assert!(result.katex_assets().is_empty());
                    check_bundle(&doc)?;
                }
                if mode < 4 {
                    assert_eq!(operation.usage().output_bytes, 0);
                }
                let mut cancelled = budget();
                cancelled.cancel();
                assert!(matches!(
                    doc.materialize_bundle(&mut cancelled),
                    Err(bundle::Error::Stopped(StopReason::Cancelled))
                ));
            }
            Ok(())
        },
    )
}

pub(super) fn check_html_parser(
    source: &LocalDocument<'_, '_, '_, '_>,
    node: &str,
) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let bundle = source.materialize_bundle(&mut budget()).map_err(err)?;
    let parser = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("audit/math/node_modules/parse5/dist/index.js");
    let script = r#"
import {pathToFileURL} from 'node:url';
import assert from 'node:assert/strict';
const {parse} = await import(pathToFileURL(process.argv[1]).href);
process.stdin.setEncoding('utf8');
let input = ''; for await (const chunk of process.stdin) input += chunk;
const doc = parse(input), links = []; let maths = 0, svgs = 0;
function walk(n, hidden = false) {
  const attrs = Object.fromEntries((n.attrs ?? []).map(a => [a.name, a.value]));
  hidden ||= attrs['aria-hidden'] === 'true';
  assert.notEqual(n.tagName, 'script'); assert.equal(attrs.style, undefined);
  if (n.tagName === 'link') { assert.equal(attrs.rel, 'stylesheet'); links.push(attrs.href); }
  if (n.tagName === 'math') { assert.equal(hidden, false); assert.equal(n.namespaceURI, 'http://www.w3.org/1998/Math/MathML'); maths++; }
  if (n.tagName === 'svg') { assert.equal(n.namespaceURI, 'http://www.w3.org/2000/svg'); svgs++; }
  for (const child of n.childNodes ?? []) walk(child, hidden);
}
walk(doc);
assert.deepEqual(links, ['assets/doc.css', 'assets/katex/katex.min.css', 'assets/math.css']);
assert.equal(maths, 6); assert.ok(svgs > 0);
"#;
    let mut child = Command::new(node)
        .args(["--input-type=module", "-e", script])
        .arg(parser)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(err)?;
    let write = child
        .stdin
        .take()
        .ok_or("parser stdin")?
        .write_all(bundle.html().as_bytes());
    let output = child.wait_with_output().map_err(err)?;
    write.map_err(err)?;
    if !output.status.success() {
        return Err(format!(
            "HTML parser: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Fault-injected protocol data, not evidence of a real cleanup failure.
pub(super) struct CleanupFailureBridge {
    directory: std::path::PathBuf,
    script: std::path::PathBuf,
}
impl CleanupFailureBridge {
    pub(super) fn path(&self) -> &std::path::Path {
        &self.script
    }
}
impl Drop for CleanupFailureBridge {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
pub(super) fn cleanup_failure_bridge(
    real: &std::path::Path,
) -> Result<CleanupFailureBridge, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "nepl3-bundle-cleanup-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).map_err(err)?;
    let bridge = CleanupFailureBridge {
        script: directory.join("reply.cjs"),
        directory,
    };
    let original = serde_json::to_string(real.to_str().ok_or("bridge path")?).map_err(err)?;
    let script = r#"
const {spawnSync} = require('node:child_process');
const chunks = []; process.stdin.on('data', c => chunks.push(c));
process.stdin.on('end', () => {
  const result = spawnSync(process.execPath, ['--', ORIGINAL, ...process.argv.slice(2)], {
    input: Buffer.concat(chunks), maxBuffer: 2000000, timeout: 8000
  });
  if (result.error || result.status !== 0) { process.stderr.write('fixture producer failed'); process.exitCode = 1; return; }
  const value = JSON.parse(result.stdout.toString('utf8'));
  value.terminationFailure = true;
  value.diagnostics = [{level:'warn', text:'bundle preserved cleanup diagnostic'}];
  value.diagnosticBytes = Buffer.byteLength(value.diagnostics[0].text) + 1;
  value.replyBytes = Buffer.byteLength(JSON.stringify({result:value.result, diagnostics:value.diagnostics,
    diagnosticBytes:value.diagnosticBytes, implementation:value.implementation}));
  process.stdout.write(JSON.stringify(value));
});
"#.replace("ORIGINAL", &original);
    std::fs::write(&bridge.script, script).map_err(err)?;
    Ok(bridge)
}
