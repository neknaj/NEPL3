//! Doc schema is an explicit domain contract, including arena categories.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/languages/doc/core/src/schema/descriptor.rs";
const READER_PATH: &str = "tools/src/doc/reader/descriptor.rs";

fn reader_projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.doc.reader" || descriptor.revision != 1 {
        return Err("unexpected Doc reader adapter identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/doc-reader.json.",
        )
        .replace("foundation --write", "doc --write")
        .replace("crate::budget::", "nepl3_core::budget::")
        .replace("alloc::", "std::"))
}

fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.doc" || descriptor.revision != 1 {
        return Err("unexpected doc package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/doc.json.",
        )
        .replace("foundation --write", "doc --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/doc.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("doc production projection differs; run doc --write".into());
    }
    let reader: Value = json(root, "interfaces/doc-reader.json")?;
    if fs::read_to_string(root.join(READER_PATH))? != reader_projection(&reader)? {
        return Err("Doc reader adapter projection differs; run doc --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&value)?,
        foundation::descriptor(&json(root, "interfaces/reader.json")?)?,
        foundation::descriptor(&reader)?,
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
    let path = root.join(READER_PATH);
    fs::create_dir_all(
        path.parent()
            .ok_or("missing Doc reader projection directory")?,
    )?;
    fs::write(
        path,
        reader_projection(&json(root, "interfaces/doc-reader.json")?)?,
    )?;
    check(root)
}
