//! Engine package schemas describe owned syntax metadata and entry identities.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/foundation/engine/src/schema/descriptor.rs";

fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.engine" || descriptor.revision != 1 {
        return Err("unexpected engine package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/engine.json.",
        )
        .replace("foundation --write", "engine --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/engine.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("engine production projection differs; run engine --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&json(root, "interfaces/reader.json")?)?,
        foundation::descriptor(&value)?,
    ] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("engine dependency identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("engine dependency registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("engine package closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/engine.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(path.parent().ok_or("missing engine projection directory")?)?;
    fs::write(path, projection(&value)?)?;
    check(root)
}
