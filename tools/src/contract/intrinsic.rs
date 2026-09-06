use crate::Result;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intrinsics {
    #[serde(rename = "NdfValue")]
    value: Intrinsic,
    #[serde(rename = "NdfScalar")]
    scalar: SubsetType,
    #[serde(rename = "TypedValue")]
    typed: SubsetType,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intrinsic {
    intrinsic: String,
    cases: BTreeMap<String, Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    tag: u8,
    fields: Vec<(String, String)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubsetType {
    subset: Subset,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Subset {
    of: String,
    cases: Vec<String>,
}

fn subset(actual: &Subset, expected: &[&str]) -> Result<()> {
    let cases: BTreeSet<_> = actual.cases.iter().map(String::as_str).collect();
    if actual.of != "NdfValue"
        || cases.len() != actual.cases.len()
        || cases != expected.iter().copied().collect()
    {
        return Err("NDF subset must select exactly its specified intrinsic cases".into());
    }
    Ok(())
}

// This is the fixed NDF/1 constructor vocabulary from spec09, not a generator
// that treats arbitrary schema variants as wire tags. Fields are logical values.
pub(super) fn check(value: &Value) -> Result<()> {
    let types: Intrinsics = serde_json::from_value(value.clone())?;
    if types.value.intrinsic != "nepl3.ndf/1" || types.value.cases.len() != 12 {
        return Err("expected the twelve intrinsic NDF/1 cases".into());
    }
    let cases: [(&str, &[(&str, &str)]); 12] = [
        ("Unit", &[]),
        ("Bool", &[("value", "Bool")]),
        ("U64", &[("value", "U64")]),
        ("Integer", &[("value", "Integer")]),
        ("Rational", &[("value", "Rational")]),
        ("Text", &[("value", "Text")]),
        ("Bytes", &[("value", "Bytes")]),
        ("List", &[("values", "List<NdfValue>")]),
        ("None", &[]),
        ("Some", &[("value", "NdfValue")]),
        (
            "Record",
            &[
                ("schema", "SchemaRef"),
                ("kind", "Text"),
                ("fields", "List<NdfValue>"),
            ],
        ),
        (
            "Variant",
            &[
                ("schema", "SchemaRef"),
                ("type", "Text"),
                ("variant", "Text"),
                ("fields", "List<NdfValue>"),
            ],
        ),
    ];
    for (tag, (name, expected)) in cases.into_iter().enumerate() {
        let actual = types
            .value
            .cases
            .get(name)
            .ok_or_else(|| format!("missing NDF case {name}"))?;
        if usize::from(actual.tag) != tag
            || actual
                .fields
                .iter()
                .map(|(n, t)| (n.as_str(), t.as_str()))
                .collect::<Vec<_>>()
                != expected
        {
            return Err(format!("{name}: incorrect NDF/1 tag or logical fields").into());
        }
    }
    subset(
        &types.scalar.subset,
        &[
            "Unit", "Bool", "U64", "Integer", "Rational", "Text", "Bytes",
        ],
    )?;
    subset(&types.typed.subset, &["Record", "Variant"])
}
