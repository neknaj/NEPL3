//! Doc schema is an explicit domain contract, including arena categories.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/languages/doc/core/src/schema/descriptor.rs";

fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.doc" || descriptor.revision != 1 {
        return Err("unexpected doc package identity".into());
    }
    foundation::generate::source_with(
        &descriptor,
        foundation::generate::Output::domain("interfaces/doc.json", "doc"),
    )
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/doc.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("doc production projection differs; run doc --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&value)?,
    ] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("doc dependency identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("doc dependency registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("doc package closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/doc.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(path.parent().ok_or("missing doc projection directory")?)?;
    fs::write(path, projection(&value)?)?;
    check(root)
}
