//! Math domain arena and foundation source-closure schema projection.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/languages/math/core/src/schema/descriptor.rs";
fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.math" || descriptor.revision != 1 {
        return Err("unexpected Math package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/math.json.",
        )
        .replace("foundation --write", "math --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/math.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("Math production projection differs; run math --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&value)?,
    ] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("Math schema identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("Math schema registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("Math schema closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value = json(root, "interfaces/math.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(path.parent().ok_or("missing Math projection directory")?)?;
    fs::write(path, projection(&value)?)?;
    check(root)
}
