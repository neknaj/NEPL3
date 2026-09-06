use super::ordered_fields;
use crate::Result;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const PRIMITIVES: [&str; 9] = [
    "Bool", "Bytes", "Bytes32", "Integer", "Natural", "Rational", "Text", "U64", "Unit",
];

#[derive(Debug, PartialEq)]
pub(super) enum TypeExpr {
    Named(String),
    List(Box<Self>),
    Option(Box<Self>),
}

impl TypeExpr {
    pub(super) fn parse(text: &str) -> Result<Self> {
        fn inner(text: &str, cursor: &mut usize, depth: usize) -> Result<TypeExpr> {
            if depth > 128 {
                return Err("type expression exceeds 128 nested constructors".into());
            }
            let start = *cursor;
            while text
                .as_bytes()
                .get(*cursor)
                .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b':' | b'/'))
            {
                *cursor += 1;
            }
            let name = &text[start..*cursor];
            identifier(name)?;
            if text.as_bytes().get(*cursor) == Some(&b'<') {
                *cursor += 1;
                let child = inner(text, cursor, depth + 1)?;
                if text.as_bytes().get(*cursor) != Some(&b'>') {
                    return Err(
                        "generic type must have exactly one argument and a closing >".into(),
                    );
                }
                *cursor += 1;
                return match name {
                    "List" => Ok(TypeExpr::List(Box::new(child))),
                    "Option" => Ok(TypeExpr::Option(Box::new(child))),
                    _ => Err(format!("undeclared generic constructor {name}").into()),
                };
            }
            if matches!(name, "List" | "Option") {
                return Err(format!("{name} requires one type argument").into());
            }
            Ok(TypeExpr::Named(name.to_owned()))
        }
        let mut cursor = 0;
        let expression = inner(text, &mut cursor, 0)?;
        if cursor != text.len() {
            return Err(format!("trailing or invalid type syntax in {text}").into());
        }
        Ok(expression)
    }

    fn resolve(&self, names: &BTreeSet<String>) -> Result<()> {
        match self {
            Self::Named(name) if names.contains(name) => Ok(()),
            Self::Named(name) => Err(format!("undefined type {name}").into()),
            Self::List(child) | Self::Option(child) => child.resolve(names),
        }
    }
}

fn identifier(name: &str) -> Result<()> {
    for part in name.split([':', '/']) {
        if !part.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(format!("invalid type identifier {name:?}").into());
        }
    }
    Ok(())
}

fn object<'a>(document: &'a Value, key: &str) -> Result<&'a Map<String, Value>> {
    document[key]
        .as_object()
        .ok_or_else(|| format!("{key} must be an object").into())
}

fn references(fields: &Value, context: &str, names: &BTreeSet<String>) -> Result<()> {
    ordered_fields(fields, context)?;
    for field in fields.as_array().ok_or("expected ordered fields")? {
        let ty = field[1].as_str().ok_or("expected type text")?;
        TypeExpr::parse(ty)?
            .resolve(names)
            .map_err(|error| format!("{context}: {error}"))?;
    }
    Ok(())
}

pub(super) fn check(model: &Value, contracts: &Value) -> Result<()> {
    let mut names: BTreeSet<String> = PRIMITIVES.iter().map(|s| (*s).to_owned()).collect();
    for (document, key) in [
        (model, "types"),
        (contracts, "records"),
        (contracts, "enums"),
        (contracts, "intrinsic_types"),
    ] {
        for name in object(document, key)?.keys() {
            identifier(name)?;
            if matches!(name.as_str(), "List" | "Option") || !names.insert(name.clone()) {
                return Err(format!("duplicate or reserved type owner {name}").into());
            }
        }
    }
    for (key, allowed) in [
        ("scalar_types", &PRIMITIVES[..]),
        ("external_types", &[][..]),
    ] {
        let mut imports = BTreeSet::new();
        for import in model[key]
            .as_array()
            .ok_or("model imports must be arrays")?
        {
            let name = import.as_str().ok_or("import must name a type")?;
            if !imports.insert(name) || !names.contains(name) {
                return Err(format!("duplicate or undefined {key} import {name}").into());
            }
            if key == "scalar_types" && !allowed.contains(&name) {
                return Err(format!("not a builtin scalar: {name}").into());
            }
            if key == "external_types"
                && (object(model, "types")?.contains_key(name) || PRIMITIVES.contains(&name))
            {
                return Err(
                    format!("external import must have an external nominal owner: {name}").into(),
                );
            }
        }
    }
    for (name, definition) in object(contracts, "scalar_aliases")? {
        if !PRIMITIVES.contains(&name.as_str())
            || !definition.as_str().is_some_and(|s| !s.trim().is_empty())
        {
            return Err(format!("invalid builtin scalar declaration {name}").into());
        }
    }
    for (name, variants) in object(contracts, "enums")? {
        let variants = variants
            .as_array()
            .filter(|v| !v.is_empty())
            .ok_or("enum must have variants")?;
        let mut seen = BTreeSet::new();
        for variant in variants {
            let variant = variant.as_str().ok_or("enum variant must be text")?;
            identifier(variant)?;
            if !seen.insert(variant) {
                return Err(format!("{name}: duplicate enum variant {variant}").into());
            }
        }
    }
    let mut model_names: BTreeSet<String> = object(model, "types")?.keys().cloned().collect();
    for key in ["scalar_types", "external_types"] {
        for import in model[key]
            .as_array()
            .ok_or("model imports must be arrays")?
        {
            model_names.insert(import.as_str().ok_or("import must be text")?.to_owned());
        }
    }
    for (document, key) in [(model, "types"), (contracts, "records")] {
        let names = if key == "types" { &model_names } else { &names };
        for (name, shape) in object(document, key)? {
            if let Some(fields) = shape.get("record").or_else(|| shape.get("fields")) {
                references(fields, name, names)?;
            } else if let Some(variants) = shape.get("sum").or_else(|| shape.get("variants")) {
                for (variant, fields) in variants.as_object().ok_or("variants must be a map")? {
                    references(fields, &format!("{name}/{variant}"), names)?;
                }
            } else if let Some(union) = shape.get("union") {
                for target in union.as_object().ok_or("union must be a map")?.values() {
                    TypeExpr::parse(target.as_str().ok_or("union target must be a type")?)?
                        .resolve(names)?;
                }
            } else {
                return Err(format!("{name}: type has no structural descriptor").into());
            }
        }
    }
    super::intrinsic::check(&contracts["intrinsic_types"])?;
    for case in contracts["intrinsic_types"]["NdfValue"]["cases"]
        .as_object()
        .ok_or("missing NDF cases")?
        .values()
    {
        references(&case["fields"], "NdfValue", &names)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn documents() -> Result<(Value, Value)> {
        Ok((
            serde_json::from_str(include_str!("../../../interfaces/model.json"))?,
            serde_json::from_str(include_str!("../../../interfaces/contracts.json"))?,
        ))
    }

    #[test]
    fn parses_nested_types_and_rejects_unknown_or_malformed_generics() -> Result<()> {
        assert_eq!(
            TypeExpr::parse("List<Option<Doc/Sentence>>")?,
            TypeExpr::List(Box::new(TypeExpr::Option(Box::new(TypeExpr::Named(
                "Doc/Sentence".into()
            )))))
        );
        for invalid in [
            "",
            "List",
            "Option",
            "List<>",
            "List<Text,U64>",
            "Pair<Text>",
            "Text<U64>",
            "List<Text>>",
            "Text trailing",
            "Text + U64",
            "Text|U64",
            "List< Text>",
            "Doc//Sentence",
            "Doc:",
            "1Type",
            "あ",
        ] {
            assert!(TypeExpr::parse(invalid).is_err(), "{invalid}");
        }
        let deepest = format!("{}Text{}", "List<".repeat(128), ">".repeat(128));
        TypeExpr::parse(&deepest)?;
        assert!(TypeExpr::parse(&format!("List<{deepest}>")).is_err());
        Ok(())
    }

    #[test]
    fn repository_descriptors_resolve_including_recursive_intrinsics_and_models() -> Result<()> {
        let (model, contracts) = documents()?;
        check(&model, &contracts)?;
        let mut contracts = contracts;
        contracts["records"]["UnitHolder"] = json!({"fields":[["value", "Unit"]]});
        check(&model, &contracts)
    }

    #[test]
    fn rejects_unresolved_nested_fields_union_targets_and_undeclared_parameters() -> Result<()> {
        let (model, contracts) = documents()?;
        for ty in [
            "List<Option<NoSuchType>>",
            "T",
            "Map<Text>",
            "List<Text,U64>",
        ] {
            let mut changed = model.clone();
            changed["types"]["Requirement"]["record"][0][1] = json!(ty);
            assert!(check(&changed, &contracts).is_err(), "{ty}");
        }
        let mut changed = model.clone();
        changed["types"]["TestUnion"] = json!({"union":{"Missing":"NoSuchType"}});
        assert!(check(&changed, &contracts).is_err());
        let mut changed = contracts.clone();
        changed["records"]["Span"]["fields"][0][1] = json!("Option<NoSuchType>");
        assert!(check(&model, &changed).is_err());
        Ok(())
    }

    #[test]
    fn rejects_missing_duplicate_or_local_external_imports_and_duplicate_owners() -> Result<()> {
        let (model, contracts) = documents()?;
        for imports in [
            json!([]),
            json!(["NdfScalar", "NdfScalar"]),
            json!(["NoSuchType"]),
            json!(["Requirement"]),
        ] {
            let mut changed = model.clone();
            changed["external_types"] = imports;
            assert!(check(&changed, &contracts).is_err());
        }
        for name in ["Span", "Text", "List", "TypedValue"] {
            let mut changed = model.clone();
            changed["types"][name] = json!({"record":[]});
            assert!(check(&changed, &contracts).is_err(), "{name}");
        }
        let mut changed = model.clone();
        changed["scalar_types"] = json!(["Requirement"]);
        assert!(check(&changed, &contracts).is_err());
        Ok(())
    }

    #[test]
    fn missing_or_description_only_intrinsic_is_not_a_closed_type() -> Result<()> {
        let (model, contracts) = documents()?;
        for name in ["NdfValue", "NdfScalar", "TypedValue"] {
            let mut changed = contracts.clone();
            changed["intrinsic_types"]
                .as_object_mut()
                .ok_or("intrinsics missing")?
                .remove(name);
            assert!(check(&model, &changed).is_err(), "{name}");
            changed["records"][name] = json!({"description":"not a shape"});
            assert!(check(&model, &changed).is_err(), "{name}");
        }
        Ok(())
    }

    #[test]
    fn intrinsic_tags_payload_order_and_subset_membership_are_fixed_by_ndf1() -> Result<()> {
        let (model, contracts) = documents()?;
        for (pointer, bad) in [
            (
                "/intrinsic_types/NdfValue/intrinsic",
                json!("domain.schema/1"),
            ),
            ("/intrinsic_types/NdfValue/cases/Unit/tag", json!(1)),
            (
                "/intrinsic_types/NdfValue/cases/Unit/fields",
                json!([["ignored", "Text"]]),
            ),
            (
                "/intrinsic_types/NdfValue/cases/Record/fields",
                json!([
                    ["kind", "Text"],
                    ["schema", "SchemaRef"],
                    ["fields", "List<NdfValue>"]
                ]),
            ),
            (
                "/intrinsic_types/NdfValue/cases/Integer/fields",
                json!([["value", "U64"]]),
            ),
            (
                "/intrinsic_types/NdfScalar/subset/cases",
                json!([
                    "Unit", "Bool", "U64", "Integer", "Rational", "Text", "Bytes", "List"
                ]),
            ),
            (
                "/intrinsic_types/TypedValue/subset/cases",
                json!(["Record", "Record"]),
            ),
            ("/intrinsic_types/TypedValue/subset/of", json!("TypedValue")),
        ] {
            let mut changed = contracts.clone();
            *changed
                .pointer_mut(pointer)
                .ok_or("mutation pointer missing")? = bad;
            assert!(check(&model, &changed).is_err(), "{pointer}");
        }
        // Subsets are sets; their order must not accidentally become a wire ID.
        let mut changed = contracts.clone();
        changed["intrinsic_types"]["TypedValue"]["subset"]["cases"] = json!(["Variant", "Record"]);
        check(&model, &changed)?;
        Ok(())
    }
}
