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
pub(super) fn check(root: &Path) -> Result<()> {
    let archive = parity(&json(root, "interfaces/source-archive.json")?)?;
    let foundation = foundation::descriptor(&json(root, "interfaces/foundation.json")?)?;
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [foundation, archive] {
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
        Ok(())
    }
}
