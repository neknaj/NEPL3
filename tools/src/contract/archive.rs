//! Source archive interface parity and closed dependency validation.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::{SchemaDescriptor, SchemaRegistry};
use serde_json::Value;
use std::path::Path;

fn parity(value: &Value) -> Result<SchemaDescriptor> {
    let declared = foundation::descriptor(value)?;
    let implemented = nepl3_core::schema::archive::descriptor(&mut foundation::budget())
        .map_err(|error| format!("source archive descriptor: {error:?}"))?;
    if declared != implemented {
        return Err("source archive JSON and Rust descriptors differ".into());
    }
    Ok(implemented)
}
fn syntax_parity(value: &Value) -> Result<SchemaDescriptor> {
    let declared = foundation::descriptor(value)?;
    let implemented = nepl3_core::schema::archive::syntax::descriptor(&mut foundation::budget())
        .map_err(|e| format!("syntax archive descriptor: {e:?}"))?;
    if declared != implemented {
        return Err("syntax archive JSON and Rust descriptors differ".into());
    }
    Ok(implemented)
}
fn group_parity(value: &Value) -> Result<SchemaDescriptor> {
    let declared = foundation::descriptor(value)?;
    let implemented = nepl3_core::schema::archive::group::descriptor(&mut foundation::budget())
        .map_err(|e| format!("group archive descriptor: {e:?}"))?;
    if declared != implemented {
        return Err("group archive JSON and Rust descriptors differ".into());
    }
    Ok(implemented)
}
fn doc_parity(value: &Value) -> Result<SchemaDescriptor> {
    let declared = foundation::descriptor(value)?;
    let implemented = nepl3_doc_core::schema::archive::descriptor(&mut foundation::budget())
        .map_err(|e| format!("doc archive descriptor: {e:?}"))?;
    if declared != implemented {
        return Err("doc archive JSON and Rust descriptors differ".into());
    }
    Ok(implemented)
}
pub(super) fn check(root: &Path) -> Result<()> {
    let archive = parity(&json(root, "interfaces/source-archive.json")?)?;
    let syntax = syntax_parity(&json(root, "interfaces/syntax-archive.json")?)?;
    let group = group_parity(&json(root, "interfaces/group-archive.json")?)?;
    let doc_archive = doc_parity(&json(root, "interfaces/doc-archive.json")?)?;
    let doc = foundation::descriptor(&json(root, "interfaces/doc.json")?)?;
    let foundation = foundation::descriptor(&json(root, "interfaces/foundation.json")?)?;
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [foundation, archive, syntax, group, doc, doc_archive] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|error| format!("source archive dependency identity: {error:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|error| format!("source archive dependency registration: {error:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|error| format!("source archive dependency closure: {error:?}"))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interface_matches_and_drift_is_rejected() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("workspace root")?;
        check(root)?;
        let mut value: Value = json(root, "interfaces/source-archive.json")?;
        value["revision"] = Value::from(2);
        assert!(parity(&value).is_err());
        let mut value: Value = json(root, "interfaces/source-archive.json")?;
        value["types"]["SourceSets"]["record"][1][1] = Value::from("Text");
        assert!(parity(&value).is_err());
        let mut syntax: Value = json(root, "interfaces/syntax-archive.json")?;
        syntax["revision"] = Value::from(2);
        assert!(syntax_parity(&syntax).is_err());
        let mut group: Value = json(root, "interfaces/group-archive.json")?;
        group["revision"] = Value::from(2);
        assert!(group_parity(&group).is_err());
        let mut doc: Value = json(root, "interfaces/doc-archive.json")?;
        doc["types"]["DocumentArchive"]["record"][0][1] = Value::from("Bytes");
        assert!(doc_parity(&doc).is_err());
        Ok(())
    }
}
