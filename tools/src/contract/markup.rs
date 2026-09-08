//! Markup domain arena and foundation source-closure schema projection.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/output/markup/src/schema/descriptor.rs";
fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.markup" || descriptor.revision != 2 {
        return Err("unexpected Markup package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/markup.json.",
        )
        .replace("foundation --write", "markup --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/markup.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("Markup production projection differs; run markup --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&value)?,
    ] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("Markup schema identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("Markup schema registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("Markup schema closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value = json(root, "interfaces/markup.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(path.parent().ok_or("missing Markup projection directory")?)?;
    fs::write(path, projection(&value)?)?;
    check(root)
}
