//! Host adapter from reviewed contract metadata to the production schema registry.
use super::types::TypeExpr;
use crate::{Result, json};
use nepl3_core::{
    budget::{Budget, Limits},
    schema::{
        FieldDescriptor, NamedType, OperationDescriptor, SchemaDescriptor, SchemaRegistry,
        TypeDescriptor, TypeRef, TypeShape, VariantDescriptor,
    },
};
use serde_json::{Map, Value, json as value};
use std::{collections::BTreeSet, fs, path::Path};

pub(super) mod generate;

const PACKAGE: &str = "nepl3.foundation";
const REVISION: u64 = 1;

fn expression(text: &str, names: &BTreeSet<&str>) -> Result<Value> {
    fn convert(ty: TypeExpr, names: &BTreeSet<&str>) -> Result<Value> {
        Ok(match ty {
            TypeExpr::List(inner) => value!({"list":convert(*inner,names)?}),
            TypeExpr::Option(inner) => value!({"option":convert(*inner,names)?}),
            TypeExpr::Named(name) if names.contains(name.as_str()) => {
                value!({"named":{"package":PACKAGE,"revision":REVISION,"name":name}})
            }
            TypeExpr::Named(name)
                if matches!(
                    name.as_str(),
                    "Unit"
                        | "Bool"
                        | "U64"
                        | "Integer"
                        | "Natural"
                        | "Rational"
                        | "Text"
                        | "Bytes"
                        | "Bytes32"
                        | "NdfValue"
                        | "NdfScalar"
                        | "TypedValue"
                ) =>
            {
                value!(name)
            }
            TypeExpr::Named(name) => {
                return Err(format!("foundation: unresolved type {name}").into());
            }
        })
    }
    convert(TypeExpr::parse(text)?, names)
}

fn fields(input: &Value, names: &BTreeSet<&str>) -> Result<Value> {
    super::ordered_fields(input, "foundation descriptor")?;
    input
        .as_array()
        .ok_or("expected fields")?
        .iter()
        .map(|pair| {
            Ok(value!([
                pair[0],
                expression(pair[1].as_str().ok_or("expected type")?, names)?
            ]))
        })
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

/// Generate actual package data; the operation description table is deliberately
/// excluded until each operation has a fully defined input/output descriptor.
pub(super) fn generated(contracts: &Value) -> Result<Value> {
    let records = contracts["records"].as_object().ok_or("missing records")?;
    let enums = contracts["enums"].as_object().ok_or("missing enums")?;
    let names: BTreeSet<_> = records
        .keys()
        .chain(enums.keys())
        .map(String::as_str)
        .collect();
    let mut types = Map::new();
    for (name, record) in records {
        let mut ty = Map::new();
        let constraints = match contracts.get("constraints").and_then(|v| v.get(name)) {
            None => value!([]),
            Some(Value::Array(ids)) => {
                // Constraints are named obligations, not an ordered execution plan.
                // Match the production descriptor's scalar-sorted canonical set.
                let mut sorted = BTreeSet::new();
                for id in ids {
                    let id = id
                        .as_str()
                        .filter(|id| !id.is_empty())
                        .ok_or("constraint ID must be nonempty text")?;
                    if !sorted.insert(id) {
                        return Err(format!("{name}: duplicate constraint {id}").into());
                    }
                }
                value!(sorted)
            }
            Some(_) => return Err(format!("{name}: constraints must be an array").into()),
        };
        ty.insert("constraints".into(), constraints);
        if let Some(input) = record.get("fields") {
            ty.insert("record".into(), fields(input, &names)?);
        } else {
            let mut variants = Map::new();
            for (name, input) in record["variants"].as_object().ok_or("expected variants")? {
                variants.insert(name.clone(), fields(input, &names)?);
            }
            ty.insert("variant".into(), Value::Object(variants));
        }
        types.insert(name.clone(), Value::Object(ty));
    }
    for (name, variants) in enums {
        let mut values = Map::new();
        for variant in variants.as_array().ok_or("expected enum array")? {
            values.insert(
                variant.as_str().ok_or("expected variant name")?.to_owned(),
                value!([]),
            );
        }
        types.insert(name.clone(), value!({"constraints":[],"variant":values}));
    }
    Ok(value!({"package":PACKAGE,"revision":REVISION,"types":types,"operations":{}}))
}

fn typed_expression(value: &Value) -> Result<TypeDescriptor> {
    if let Some(name) = value.as_str() {
        return Ok(match name {
            "Unit" => TypeDescriptor::Unit,
            "Bool" => TypeDescriptor::Bool,
            "U64" => TypeDescriptor::U64,
            "Integer" => TypeDescriptor::Integer,
            "Natural" => TypeDescriptor::Natural,
            "Rational" => TypeDescriptor::Rational,
            "Text" => TypeDescriptor::Text,
            "Bytes" => TypeDescriptor::Bytes,
            "Bytes32" => TypeDescriptor::Bytes32,
            "NdfValue" => TypeDescriptor::NdfValue,
            "NdfScalar" => TypeDescriptor::NdfScalar,
            "TypedValue" => TypeDescriptor::TypedValue,
            _ => return Err(format!("unknown intrinsic descriptor {name}").into()),
        });
    }
    if let Some(inner) = value.get("list") {
        return Ok(TypeDescriptor::List(Box::new(typed_expression(inner)?)));
    }
    if let Some(inner) = value.get("option") {
        return Ok(TypeDescriptor::Option(Box::new(typed_expression(inner)?)));
    }
    let named = &value["named"];
    Ok(TypeDescriptor::Named(TypeRef {
        package: named["package"]
            .as_str()
            .ok_or("missing package")?
            .to_owned(),
        revision: named["revision"].as_u64().ok_or("missing revision")?,
        name: named["name"]
            .as_str()
            .ok_or("missing type name")?
            .to_owned(),
    }))
}

fn typed_fields(value: &Value) -> Result<Vec<FieldDescriptor>> {
    value
        .as_array()
        .ok_or("missing fields")?
        .iter()
        .map(|field| {
            Ok(FieldDescriptor {
                name: field[0].as_str().ok_or("missing field name")?.to_owned(),
                ty: typed_expression(&field[1])?,
            })
        })
        .collect()
}

pub(super) fn descriptor(value: &Value) -> Result<SchemaDescriptor> {
    let mut types = Vec::new();
    for (name, ty) in value["types"].as_object().ok_or("missing types")? {
        let shape = if let Some(record) = ty.get("record") {
            TypeShape::Record {
                fields: typed_fields(record)?,
            }
        } else {
            TypeShape::Variant {
                variants: ty["variant"]
                    .as_object()
                    .ok_or("missing variant map")?
                    .iter()
                    .map(|(name, fields)| {
                        Ok(VariantDescriptor {
                            name: name.clone(),
                            fields: typed_fields(fields)?,
                        })
                    })
                    .collect::<Result<_>>()?,
            }
        };
        types.push(NamedType {
            name: name.clone(),
            shape,
            constraints: ty["constraints"]
                .as_array()
                .ok_or("missing constraints")?
                .iter()
                .map(|id| {
                    id.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "constraint must be text".into())
                })
                .collect::<Result<_>>()?,
        });
    }
    let operations = value["operations"]
        .as_object()
        .ok_or("missing operations")?
        .iter()
        .map(|(name, operation)| {
            Ok(OperationDescriptor {
                name: name.clone(),
                input: typed_expression(&operation["input"])?,
                output: typed_expression(&operation["output"])?,
                pure: operation["pure"].as_bool().ok_or("missing purity")?,
            })
        })
        .collect::<Result<_>>()?;
    Ok(SchemaDescriptor {
        package: value["package"]
            .as_str()
            .ok_or("missing package")?
            .to_owned(),
        revision: value["revision"].as_u64().ok_or("missing revision")?,
        types,
        operations,
    })
}

pub(super) fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 64 * 1024 * 1024,
        work: 10_000_000,
        depth: 128,
        nodes: 1_000_000,
        allocation_units: 64 * 1024 * 1024,
        output_bytes: 64 * 1024 * 1024,
        diagnostics: 1000,
        events: 1000,
    })
}

pub(super) fn check(root: &Path, contracts: &Value) -> Result<()> {
    let actual: Value = json(root, "interfaces/foundation.json")?;
    if actual != generated(contracts)? {
        return Err("foundation descriptor differs; run foundation --write".into());
    }
    let descriptor = descriptor(&actual)?;
    if fs::read_to_string(root.join(generate::PATH))? != generate::source(&descriptor)? {
        return Err("production foundation projection differs; run foundation --write".into());
    }
    let mut budget = budget();
    let reference = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("foundation digest: {e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(reference, descriptor, &mut budget)
        .map_err(|e| format!("foundation registration: {e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("foundation reference closure: {e:?}"))?;
    Ok(())
}

pub(crate) fn write(root: &Path) -> Result<()> {
    let contracts: Value = json(root, "interfaces/contracts.json")?;
    let value = generated(&contracts)?;
    fs::write(
        root.join("interfaces/foundation.json"),
        serde_json::to_string_pretty(&value)? + "\n",
    )?;
    write_projection(root, &value)?;
    check(root, &contracts)
}

pub(super) fn write_projection(root: &Path, value: &Value) -> Result<()> {
    let path = root.join(generate::PATH);
    fs::create_dir_all(path.parent().ok_or("missing projection directory")?)?;
    fs::write(path, generate::source(&descriptor(value)?)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::value::{NdfValue, Record};

    #[test]
    fn constraint_declaration_order_is_canonical_but_duplicates_are_invalid() -> Result<()> {
        let mut contracts: Value =
            serde_json::from_str(include_str!("../../../interfaces/contracts.json"))?;
        contracts["constraints"]["SyntaxBundle"] = value!(["syntax.graph", "source.map"]);
        let forward = generated(&contracts)?;
        contracts["constraints"]["SyntaxBundle"] = value!(["source.map", "syntax.graph"]);
        let reverse = generated(&contracts)?;
        assert_eq!(forward, reverse);
        assert_eq!(
            descriptor(&forward)?
                .canonical_json(&mut budget())
                .map_err(|e| format!("{e:?}"))?,
            serde_json::to_vec(&forward)?
        );
        contracts["constraints"]["SyntaxBundle"] = value!(["source.map", "source.map"]);
        assert!(generated(&contracts).is_err());
        contracts["constraints"]["SyntaxBundle"] = value!([42]);
        assert!(generated(&contracts).is_err());
        Ok(())
    }

    #[test]
    fn generated_foundation_is_registered_and_checks_ordinary_schema_ref_records() -> Result<()> {
        let contracts: Value =
            serde_json::from_str(include_str!("../../../interfaces/contracts.json"))?;
        let data = generated(&contracts)?;
        let descriptor = descriptor(&data)?;
        let mut budget = budget();
        // Independent host JSON serializer agrees with the production canonical writer
        // on this descriptor; its names contain no control characters.
        assert_eq!(
            descriptor
                .canonical_json(&mut budget)
                .map_err(|e| format!("{e:?}"))?,
            serde_json::to_vec(&data)?
        );
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("{e:?}"))?;
        let expected = TypeDescriptor::Named(TypeRef {
            package: PACKAGE.into(),
            revision: REVISION,
            name: "SchemaRef".into(),
        });
        let value = NdfValue::Record(Record {
            schema: reference.clone(),
            kind: "SchemaRef".into(),
            fields: vec![
                NdfValue::Text("example.domain".into()),
                NdfValue::U64(7),
                NdfValue::Bytes(vec![0x42; 32]),
            ],
        });
        let mut registry = SchemaRegistry::default();
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("{e:?}"))?;
        assert!(registry.validate(&expected, &value, &mut budget).is_err());
        registry
            .finalize(&mut budget)
            .map_err(|e| format!("{e:?}"))?;
        registry
            .validate(&expected, &value, &mut budget)
            .map_err(|e| format!("{e:?}"))?;
        let mut bad = value;
        if let NdfValue::Record(record) = &mut bad {
            record.fields[2] = NdfValue::Bytes(vec![0x42; 31]);
        }
        assert!(registry.validate(&expected, &bad, &mut budget).is_err());
        Ok(())
    }

    #[test]
    fn checker_rejects_changed_foundation_descriptor_before_registry_use() -> Result<()> {
        let files = crate::testing::Fixture::new()?;
        let contracts: Value =
            serde_json::from_str(include_str!("../../../interfaces/contracts.json"))?;
        let mut descriptor = generated(&contracts)?;
        files.json("interfaces/foundation.json", &descriptor)?;
        write_projection(files.root(), &descriptor)?;
        check(files.root(), &contracts)?;
        descriptor["types"]["SchemaRef"]["record"] = value!([]);
        files.json("interfaces/foundation.json", &descriptor)?;
        assert!(check(files.root(), &contracts).is_err());
        Ok(())
    }
}
