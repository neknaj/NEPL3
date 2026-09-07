pub(crate) mod doc;
pub(crate) mod engine;
pub(crate) mod foundation;
pub(crate) mod grammar;
mod intrinsic;
pub(crate) mod reader;
mod types;

use crate::{Result, json, repository::local_path, task::unique};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Deserialize)]
struct Forms {
    roots: BTreeMap<String, String>,
    categories: BTreeMap<String, Category>,
}
#[derive(Deserialize)]
struct Category {
    forms: BTreeMap<String, Form>,
    leaf: Option<String>,
}
#[derive(Deserialize)]
struct Form {
    kind: String,
    fields: Vec<Field>,
}
#[derive(Deserialize)]
struct Field {
    name: String,
    read: Read,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Read {
    Category(String),
    List(List),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    list: String,
}

fn forms(forms: &Forms) -> Result<()> {
    let categories: BTreeSet<_> = forms.categories.keys().map(String::as_str).collect();
    let builtins = ["@Name", "@Nat", "@Text", "@Lang"];
    for root in forms.roots.values() {
        if !categories.contains(root.as_str()) {
            return Err(format!("undefined root category {root}").into());
        }
    }
    for (name, category) in &forms.categories {
        if !matches!(
            category.leaf.as_deref(),
            None | Some("name" | "sentence" | "math-number-or-name")
        ) {
            return Err(format!("{name}: unknown leaf reader").into());
        }
        for (head, form) in &category.forms {
            if head.is_empty() || form.kind.is_empty() {
                return Err(format!("{name}: empty form head/kind").into());
            }
            unique(form.fields.iter().map(|f| f.name.as_str()), "form fields")?;
            for field in &form.fields {
                let reference = match &field.read {
                    Read::Category(name) => name,
                    Read::List(list) => &list.list,
                };
                if !categories.contains(reference.as_str())
                    && !builtins.contains(&reference.as_str())
                {
                    return Err(format!(
                        "{name}/{head}/{}: undefined read category {reference}",
                        field.name
                    )
                    .into());
                }
                if matches!(&field.read, Read::List(_)) && builtins.contains(&reference.as_str()) {
                    return Err("list reference must name a category".into());
                }
            }
        }
    }
    Ok(())
}

fn ordered_fields(value: &Value, context: &str) -> Result<()> {
    let fields = value
        .as_array()
        .ok_or_else(|| format!("{context}: fields must be ordered [name, type] pairs"))?;
    let mut names = BTreeSet::new();
    for field in fields {
        let pair = field
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| format!("{context}: expected [name, type] pair"))?;
        let name = pair[0]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or("field name must be nonempty text")?;
        let ty = pair[1]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or("field type must be nonempty text")?;
        if !names.insert(name) {
            return Err(format!("{context}: duplicate field {name}").into());
        }
        types::TypeExpr::parse(ty)?;
    }
    Ok(())
}

fn variants(value: &Value, context: &str) -> Result<()> {
    let variants = value
        .as_object()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("{context}: variants must be a nonempty name map"))?;
    for (name, fields) in variants {
        if name.trim().is_empty() {
            return Err(format!("{context}: variant name cannot be empty").into());
        }
        ordered_fields(fields, &format!("{context}/{name}"))?;
    }
    Ok(())
}

fn model_type(description: &Value, context: &str) -> Result<()> {
    let shape = description
        .as_object()
        .filter(|v| v.len() == 1)
        .ok_or_else(|| format!("{context}: expected exactly one model type shape"))?;
    if let Some(record) = shape.get("record") {
        return ordered_fields(record, context);
    }
    if let Some(sum) = shape.get("sum") {
        return variants(sum, context);
    }
    if let Some(union) = shape.get("union") {
        let variants = union
            .as_object()
            .filter(|v| !v.is_empty())
            .ok_or("union must be a nonempty name map")?;
        for (name, ty) in variants {
            if name.trim().is_empty() || ty.as_str().is_none_or(|s| s.trim().is_empty()) {
                return Err(
                    format!("{context}: union names and types must be nonempty text").into(),
                );
            }
        }
        return Ok(());
    }
    Err(format!("{context}: unknown model type shape").into())
}

fn contract_record(description: &Value, context: &str) -> Result<()> {
    let shape = description
        .as_object()
        .filter(|v| v.len() == 1)
        .ok_or_else(|| format!("{context}: expected exactly one contract record shape"))?;
    if let Some(fields) = shape.get("fields") {
        return ordered_fields(fields, context);
    }
    if let Some(value) = shape.get("variants") {
        return variants(value, context);
    }
    Err(format!("{context}: expected fields or variants; unresolved descriptors require a reviewed contract change").into())
}

pub(crate) fn check(root: &Path) -> Result<()> {
    let forms_file: Forms = json(root, "design/forms.json")?;
    forms(&forms_file)?;
    let model: Value = json(root, "interfaces/model.json")?;
    let types = model
        .get("types")
        .and_then(Value::as_object)
        .ok_or("model types must be an object")?;
    for (name, description) in types {
        model_type(description, name)?;
    }
    let contracts: Value = json(root, "interfaces/contracts.json")?;
    let records = contracts
        .get("records")
        .and_then(Value::as_object)
        .filter(|v| !v.is_empty())
        .ok_or("contract records must be a nonempty object")?;
    for (name, description) in records {
        if name.trim().is_empty() {
            return Err("contract record name cannot be empty".into());
        }
        contract_record(description, name)?;
    }
    types::check(&model, &contracts)?;
    foundation::check(root, &contracts)?;
    let operations = contracts
        .get("operations")
        .and_then(Value::as_array)
        .ok_or("missing operations")?;
    unique(
        operations
            .iter()
            .map(|v| v.get("operation").and_then(Value::as_str).unwrap_or("")),
        "operations",
    )?;
    for operation in operations {
        let definition = operation
            .get("definition")
            .and_then(Value::as_str)
            .ok_or("operation missing definition")?;
        local_path(root, definition)?;
    }
    let cases: Value = json(root, "conformance/cases.json")?;
    let cases = cases
        .get("cases")
        .and_then(Value::as_array)
        .ok_or("missing cases")?;
    unique(
        cases
            .iter()
            .map(|v| v.get("id").and_then(Value::as_str).unwrap_or("")),
        "conformance cases",
    )?;
    for case in cases {
        for field in ["path", "left", "right", "grammar"] {
            if let Some(value) = case.get(field) {
                local_path(root, value.as_str().ok_or("case path must be text")?)?;
            }
        }
    }
    println!(
        "Contract structure: {} categories, {} model types, {} operation descriptions. Named field references and intrinsic shapes checked; operation schemas, value invariants and language parsing remain unverified.",
        forms_file.categories.len(),
        types.len(),
        operations.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;
    use serde_json::json;

    fn fixture() -> Result<Fixture> {
        let files = Fixture::new()?;
        files.json("design/forms.json", &json!({"roots":{"Test":"Test/Root"},"categories":{"Test/Root":{"forms":{"unit":{"kind":"Unit","fields":[]}},"leaf":null}}}))?;
        files.json("interfaces/model.json", &json!({"scalar_types":["U64"],"external_types":["TypedValue"],"types":{"Record":{"record":[["id","U64"]]},"Sum":{"sum":{"Some":[["value","U64"]],"None":[]}}}}))?;
        let mut contracts: Value =
            serde_json::from_str(include_str!("../../../interfaces/contracts.json"))?;
        contracts["operations"] = json!([{"operation":"test","definition":"doc/spec/test.md"}]);
        files.json("interfaces/contracts.json", &contracts)?;
        files.json(
            "interfaces/foundation.json",
            &foundation::generated(&contracts)?,
        )?;
        foundation::write_projection(files.root(), &foundation::generated(&contracts)?)?;
        files.write("doc/spec/test.md", "fixture")?;
        files.json("conformance/cases.json", &json!({"cases":[]}))?;
        Ok(files)
    }

    #[test]
    fn real_contract_entry_rejects_unresolved_fields_intrinsics_and_imports() -> Result<()> {
        let files = fixture()?;
        check(files.root())?;
        let contracts: Value = crate::json(files.root(), "interfaces/contracts.json")?;
        let model: Value = crate::json(files.root(), "interfaces/model.json")?;

        let mut changed = contracts.clone();
        changed["records"]["Span"]["fields"][0][1] = json!("List<Option<MissingType>>");
        files.json("interfaces/contracts.json", &changed)?;
        assert!(check(files.root()).is_err());

        let mut changed = contracts.clone();
        changed["intrinsic_types"]
            .as_object_mut()
            .ok_or("fixture intrinsics missing")?
            .remove("TypedValue");
        files.json("interfaces/contracts.json", &changed)?;
        assert!(check(files.root()).is_err());

        files.json("interfaces/contracts.json", &contracts)?;
        let mut changed = model;
        changed["types"]["Record"]["record"] = json!([["value", "TypedValue"]]);
        files.json("interfaces/model.json", &changed)?;
        check(files.root())?;
        changed["external_types"] = json!([]);
        files.json("interfaces/model.json", &changed)?;
        assert!(check(files.root()).is_err());
        Ok(())
    }

    #[test]
    fn real_contract_checker_rejects_malformed_contract_fields_and_variant_payloads() -> Result<()>
    {
        let files = fixture()?;
        check(files.root())?;
        let original: Value = crate::json(files.root(), "interfaces/contracts.json")?;
        for pointer in [
            "/records/SourceRef/fields",
            "/records/OperationReply/variants/Complete",
        ] {
            for bad in [
                json!({"id":"U64"}),
                json!([["id", "U64"], ["id", "Text"]]),
                json!([["id"]]),
                json!([["id", "U64", "extra"]]),
                json!([[" ", "U64"]]),
                json!([["id", 42]]),
                json!([["id", "List<Text"]]),
            ] {
                let mut document = original.clone();
                *document
                    .pointer_mut(pointer)
                    .ok_or("test pointer missing")? = bad;
                files.json("interfaces/contracts.json", &document)?;
                assert!(check(files.root()).is_err(), "{pointer}");
            }
        }
        for bad in [
            json!({}),
            json!({"fields":[],"variants":{"V":[]}}),
            json!({"variants":[]}),
            json!({"description":"accidental missing fields"}),
        ] {
            let mut document = original.clone();
            document["records"]["SourceRef"] = bad;
            files.json("interfaces/contracts.json", &document)?;
            assert!(check(files.root()).is_err());
        }
        Ok(())
    }

    #[test]
    fn real_contract_checker_uses_same_validation_for_model_fields() -> Result<()> {
        let files = fixture()?;
        let original: Value = crate::json(files.root(), "interfaces/model.json")?;
        for pointer in ["/types/Record/record", "/types/Sum/sum/Some"] {
            let mut document = original.clone();
            *document
                .pointer_mut(pointer)
                .ok_or("test pointer missing")? = json!([["id", "U64"], ["id", "Text"]]);
            files.json("interfaces/model.json", &document)?;
            assert!(check(files.root()).is_err(), "{pointer}");
        }
        Ok(())
    }

    #[test]
    fn unordered_or_duplicate_schema_fields_are_rejected() {
        assert!(ordered_fields(&serde_json::json!({"id":"U64"}), "type").is_err());
        assert!(
            ordered_fields(&serde_json::json!([["id", "U64"], ["id", "Text"]]), "type").is_err()
        );
        assert!(ordered_fields(&serde_json::json!([["items", "List<Text"]]), "type").is_err());
        assert!(
            ordered_fields(
                &serde_json::json!([["id", "U64"], ["items", "List<Text>"]]),
                "type"
            )
            .is_ok()
        );
    }

    #[test]
    fn unknown_form_reference_is_rejected() -> Result<()> {
        let value = serde_json::json!({"roots":{"Doc":"Doc/Root"},"categories":{"Doc/Root":{"forms":{"form":{"kind":"Form","fields":[{"name":"child","read":"Doc/Missing"}]}},"leaf":null}}});
        let parsed: Forms = serde_json::from_value(value)?;
        assert!(forms(&parsed).is_err());
        Ok(())
    }
}
