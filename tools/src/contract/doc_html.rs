//! Doc HTML domain arena and foundation source-closure schema projection.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/languages/doc/html/src/schema/descriptor.rs";
fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.doc.html" || descriptor.revision != 1 {
        return Err("unexpected Doc HTML package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/doc-html.json.",
        )
        .replace("foundation --write", "doc-html --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/doc-html.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("Doc HTML production projection differs; run doc-html --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&json(root, "interfaces/doc.json")?)?,
        foundation::descriptor(&json(root, "interfaces/markup.json")?)?,
        foundation::descriptor(&value)?,
    ] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("Doc HTML schema identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("Doc HTML schema registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("Doc HTML schema closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value = json(root, "interfaces/doc-html.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(
        path.parent()
            .ok_or("missing Doc HTML projection directory")?,
    )?;
    fs::write(path, projection(&value)?)?;
    check(root)
}
