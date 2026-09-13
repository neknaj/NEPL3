//! Correct a fixed rustdoc empty-public-implementor serialization omission.
//! This is deliberately not a general missing-resource fallback.
use super::super::{Result, hash, insert};
use std::collections::BTreeMap;

pub(super) const VERSION: &str = "rustdoc 1.97.0 (2d8144b78 2026-07-07)";
pub(super) const COMMIT: &str = "2d8144b7880597b6e6d3dfd63a9a9efae3f533d3";

#[derive(serde::Serialize)]
pub(super) struct Correction {
    page: String,
    page_sha256: String,
    resource: String,
    resource_sha256: String,
    reason: &'static str,
}

pub(super) fn correct(
    files: &mut BTreeMap<String, Vec<u8>>,
    version: &str,
    compiler: &str,
) -> Result<Vec<Correction>> {
    let mut corrections = Vec::new();
    for (owner, module, name) in [
        ("nepl3_engine", "binding", "BindingHost"),
        ("nepl3_reader", "tokenizer", "TokenizationHost"),
    ] {
        let page = format!("api/rust/{owner}/{module}/trait.{name}.html");
        let resource = format!("api/rust/trait.impl/{owner}/{module}/host/trait.{name}.js");
        if files.contains_key(&resource) || !files.contains_key(&page) {
            continue;
        }
        if version != VERSION
            || !compiler
                .lines()
                .any(|line| line == format!("commit-hash: {COMMIT}"))
        {
            return Err("unsupported rustdoc empty-implementor correction version".into());
        }
        let original = files.get(&page).ok_or("missing trait page")?;
        let html = std::str::from_utf8(original)?;
        let marker = format!(
            "<div id=\"implementors-list\"></div><script src=\"../../trait.impl/{owner}/{module}/host/trait.{name}.js\" async></script>"
        );
        if html.matches(&marker).count() != 1
            || !html.contains(&format!("data-current-crate=\"{owner}\""))
            || !html.contains("data-rustdoc-version=\"1.97.0 (2d8144b78 2026-07-07)\"")
            || !html.contains(&format!(
                "<title>{name} in {owner}::{module} - Rust</title>"
            ))
            || html.contains("id=\"foreign-impls\"")
            || html.contains("id=\"synthetic-implementors-list\"")
        {
            return Err(format!("unexpected or nonempty rustdoc trait output: {page}").into());
        }
        let page_sha256 = hash(original);
        // Public entries for this crate are empty. Do not add a Rust implementor
        // or assert that no private or out-of-workspace implementation exists.
        let data = format!("(()=>{{const entries={{{owner:?}:[]}};if(window.register_implementors){{window.register_implementors(entries);}}else{{window.pending_implementors=entries;}}}})();\n").into_bytes();
        let resource_sha256 = hash(&data);
        insert(files, &resource, data)?;
        corrections.push(Correction {
            page,
            page_sha256,
            resource,
            resource_sha256,
            reason: "rustdoc-1.97-empty-public-implementors/1",
        });
    }
    Ok(corrections)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_rustdoc_empty_and_public_implementor_outputs_are_distinguished() -> Result<()> {
        let fixture = crate::testing::Fixture::new()?;
        for (case, implementation) in [
            ("empty", ""),
            (
                "implemented",
                "pub struct Actual; impl binding::BindingHost for Actual { fn call(&mut self) {} }",
            ),
        ] {
            let source = format!(
                "pub mod binding {{ mod host {{ pub trait BindingHost {{ fn call(&mut self); }} }} pub use host::BindingHost; }} {implementation}"
            );
            fixture.write(&format!("{case}.rs"), &source)?;
            crate::command(
                fixture.root(),
                "rustdoc",
                &[
                    "--edition=2024",
                    "--crate-name",
                    "nepl3_engine",
                    &format!("{case}.rs"),
                    "-o",
                    case,
                ],
            )?;
            let mut files = super::super::collect(&fixture.root().join(case))?;
            let original = files.clone();
            let version =
                String::from_utf8(crate::command(fixture.root(), "rustdoc", &["--version"])?)?;
            let compiler = String::from_utf8(crate::command(fixture.root(), "rustc", &["-vV"])?)?;
            if case == "empty" {
                assert!(correct(&mut original.clone(), "unknown", &compiler).is_err());
                assert!(correct(&mut original.clone(), version.trim(), "unknown").is_err());
                let mut changed = original.clone();
                let page = "api/rust/nepl3_engine/binding/trait.BindingHost.html";
                let html = std::str::from_utf8(&changed[page])?.replace(
                    "<div id=\"implementors-list\"></div>",
                    "<div id=\"implementors-list\"><section>Actual</section></div>",
                );
                changed.insert(page.into(), html.into_bytes());
                let before = changed.clone();
                assert!(correct(&mut changed, version.trim(), &compiler).is_err());
                assert_eq!(changed, before);
            }
            let corrections = correct(&mut files, version.trim(), &compiler)?;
            if case == "empty" {
                assert_eq!(corrections.len(), 1);
                assert_eq!(files.len(), original.len() + 1);
                for (path, data) in original {
                    assert_eq!(files[&path], data);
                }
            } else {
                // Same-crate implementations are inlined in the HTML; rustdoc's
                // JS index contains cross-crate entries. Preserve both outputs.
                assert!(corrections.is_empty());
                assert_eq!(files, original);
                assert!(
                    std::str::from_utf8(
                        &files["api/rust/nepl3_engine/binding/trait.BindingHost.html"]
                    )?
                    .contains("Actual")
                );
            }
        }
        Ok(())
    }
}
