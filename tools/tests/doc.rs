//! Actual standard Doc source compilation and prefix parsing. Facts signatures
//! are compile-time declarations only; this parse-only host serves reader calls.
#[path = "doc/annotated.rs"]
mod annotated;
#[path = "doc/annotated_blocks.rs"]
mod annotated_blocks;
#[path = "doc/annotated_pages.rs"]
mod annotated_pages;
#[path = "doc/export.rs"]
mod export;
#[path = "doc/external.rs"]
mod external;
#[path = "doc/html.rs"]
mod html;
#[path = "doc/input.rs"]
mod input;
#[path = "doc/labels.rs"]
mod labels;
#[path = "doc/pages.rs"]
mod pages;
#[path = "doc/phases.rs"]
mod phases;
#[path = "doc/prepare.rs"]
mod prepare;
#[path = "doc/print.rs"]
mod print;
#[path = "doc/projection.rs"]
mod projection;
#[path = "doc/resources.rs"]
mod resources;
#[path = "doc/retention.rs"]
mod retention;
use retention::assert_doc_retention;
use nepl3_core::value::NdfValue;
use nepl3_core::{budget::*, source::*};
use nepl3_engine::profile::*;
use nepl3_tools::doc::source::{
    Compiled, budget, compiled, err, parse_source_as, with_input, with_input_route,
};
use nepl3_wire::foundation::FoundationCodec;
#[test]
fn native_doc_host_preserves_owned_parse_tree_and_uses_less_allocation() -> Result<(), String> {
    let compiled = compiled()?;
    let source = include_str!("../../examples/document/line-break.nepld");
    let run = |native| {
        with_input_route(native, &compiled, source, "Article", |tree, _, b, _| {
            Ok((tree.tree().clone(), b.usage()))
        })
    };
    let (owned, owned_usage) = run(false)?;
    let (native, native_usage) = run(true)?;
    assert_eq!(native, owned);
    assert!(native_usage.allocation_units < owned_usage.allocation_units);
    println!("Doc identical tree: owned={owned_usage:?}; native={native_usage:?}");
    Ok(())
}
#[test]
#[cfg(not(target_family = "wasm"))]
fn doc_suite_seed_artifacts_match_the_original_sources() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("root")?;
    for (name, stored) in [
        (
            "doc",
            include_bytes!("../../conformance/fixtures/doc/syntax.json").as_slice(),
        ),
        (
            "math",
            include_bytes!("../../conformance/fixtures/doc/math.json").as_slice(),
        ),
        (
            "circuit",
            include_bytes!("../../conformance/fixtures/doc/circuit.json").as_slice(),
        ),
        (
            "grammar",
            include_bytes!("../../conformance/fixtures/doc/grammar.json").as_slice(),
        ),
    ] {
        let path = format!("languages/{name}/syntax.neplg");
        let output = std::process::Command::new("python")
            .arg(root.join("tools/bootstrap/grammar.py"))
            .arg(root.join(&path))
            .args([
                "--source-id",
                &path,
                "--uri",
                &format!("memory:{name}-grammar"),
            ])
            .env("PYTHONIOENCODING", "utf-8")
            .output()
            .map_err(err)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        // JSON whitespace is transport framing; the embedded original source
        // bytes/identity and every typed constructor must be exactly equal.
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(err)?;
        let saved: serde_json::Value = serde_json::from_slice(stored).map_err(err)?;
        assert_eq!(actual, saved, "{name}");
    }
    Ok(())
}
