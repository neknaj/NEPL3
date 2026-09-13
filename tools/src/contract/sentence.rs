//! Sentence domain arena and foundation source-closure schema projection.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/languages/sentence/core/src/schema/descriptor.rs";
const READER_PATH: &str = "tools/src/sentence/reader/descriptor.rs";
fn reader_projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.sentence.reader" || descriptor.revision != 1 {
        return Err("unexpected Sentence reader adapter identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/sentence-reader.json.",
        )
        .replace("foundation --write", "sentence --write")
        .replace("crate::budget::", "nepl3_core::budget::")
        .replace("alloc::", "std::"))
}
fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.sentence" || descriptor.revision != 1 {
        return Err("unexpected Sentence package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/sentence.json.",
        )
        .replace("foundation --write", "sentence --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/sentence.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("Sentence production projection differs; run sentence --write".into());
    }
    let reader: Value = json(root, "interfaces/sentence-reader.json")?;
    if fs::read_to_string(root.join(READER_PATH))? != reader_projection(&reader)? {
        return Err("Sentence reader projection differs; run sentence --write".into());
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
            .map_err(|e| format!("Sentence schema identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("Sentence schema registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("Sentence schema closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value = json(root, "interfaces/sentence.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(
        path.parent()
            .ok_or("missing Sentence projection directory")?,
    )?;
    fs::write(path, projection(&value)?)?;
    let path = root.join(READER_PATH);
    fs::create_dir_all(path.parent().ok_or("missing Sentence reader directory")?)?;
    fs::write(
        path,
        reader_projection(&json(root, "interfaces/sentence-reader.json")?)?,
    )?;
    check(root)
}
