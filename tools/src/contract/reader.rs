//! Reader request/provider schemas are real registered envelopes, not payload aliases.
use super::foundation;
use crate::{Result, json};
use nepl3_core::schema::SchemaRegistry;
use serde_json::Value;
use std::{fs, path::Path};
const PATH: &str = "crates/foundation/reader/src/schema/descriptor.rs";

fn projection(value: &Value) -> Result<String> {
    let descriptor = foundation::descriptor(value)?;
    if descriptor.package != "nepl3.reader" || descriptor.revision != 1 {
        return Err("unexpected reader package identity".into());
    }
    Ok(foundation::generate::source(&descriptor)?
        .replace(
            "Generated from interfaces/contracts.json via interfaces/foundation.json.",
            "Generated from interfaces/reader.json.",
        )
        .replace("foundation --write", "reader --write")
        .replace("crate::budget::", "nepl3_core::budget::"))
}
pub(crate) fn check(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/reader.json")?;
    if fs::read_to_string(root.join(PATH))? != projection(&value)? {
        return Err("reader production projection differs; run reader --write".into());
    }
    let mut budget = foundation::budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        foundation::descriptor(&json(root, "interfaces/foundation.json")?)?,
        foundation::descriptor(&value)?,
    ] {
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("reader dependency identity: {e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("reader dependency registration: {e:?}"))?;
    }
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("reader envelope closure: {e:?}"))?;
    Ok(())
}
pub(crate) fn write(root: &Path) -> Result<()> {
    let value: Value = json(root, "interfaces/reader.json")?;
    let path = root.join(PATH);
    fs::create_dir_all(path.parent().ok_or("missing reader projection directory")?)?;
    fs::write(path, projection(&value)?)?;
    check(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reader_envelope_reference_closure_rejects_nested_missing_type() -> Result<()> {
        let fixture = crate::testing::Fixture::new()?;
        let foundation: Value =
            serde_json::from_str(include_str!("../../../interfaces/foundation.json"))?;
        let mut reader: Value =
            serde_json::from_str(include_str!("../../../interfaces/reader.json"))?;
        fixture.json("interfaces/foundation.json", &foundation)?;
        fixture.json("interfaces/reader.json", &reader)?;
        write(fixture.root())?;
        reader["types"]["ReadRequest"]["record"][1][1]["list"]["named"]["name"] =
            "MissingSource".into();
        fixture.json("interfaces/reader.json", &reader)?;
        assert!(write(fixture.root()).is_err());
        Ok(())
    }
}
