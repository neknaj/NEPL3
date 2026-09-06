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
            .filter(|s| !s.is_empty())
            .ok_or("field name must be nonempty text")?;
        let ty = pair[1]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("field type must be nonempty text")?;
        if !names.insert(name) {
            return Err(format!("{context}: duplicate field {name}").into());
        }
        // Deliberately no closure claim: unresolved references are design review blockers.
        let mut depth = 0_u32;
        for character in ty.chars() {
            match character {
                '<' => depth += 1,
                '>' => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| format!("{context}: unbalanced type {ty}"))?
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(format!("{context}: unbalanced type {ty}").into());
        }
    }
    Ok(())
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
        if let Some(record) = description.get("record") {
            ordered_fields(record, name)?;
        }
        if let Some(variants) = description.get("sum") {
            for (variant, fields) in variants.as_object().ok_or("sum must map variant names")? {
                ordered_fields(fields, &format!("{name}/{variant}"))?;
            }
        }
    }
    let contracts: Value = json(root, "interfaces/contracts.json")?;
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
        "Contract structure: {} categories, {} model types, {} operation descriptions. Schema closure and language parsing are not established by these checks.",
        forms_file.categories.len(),
        types.len(),
        operations.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
