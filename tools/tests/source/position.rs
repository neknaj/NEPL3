use nepl3_core::{
    budget::{Budget, Limits},
    source::{LineIndex, Position, PositionEncoding, SourceError, SourceId, SourceSnapshot},
};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use std::{fmt, marker::PhantomData};

// Serde's derived structs also accept positional arrays. The external JSON
// contract requires objects, while retaining duplicate-field detection.
struct Object<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}
fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<T, D::Error> {
    Object::<T>::deserialize(deserializer).map(|value| value.0)
}
fn objects<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<Object<T>>::deserialize(deserializer)
        .map(|values| values.into_iter().map(|value| value.0).collect())
}

// This is fixed evidence input, never output generated through either conversion.
const ORACLE: &str = include_str!("../../../conformance/inputs/browser/source-position.json");
const TEXTS: [&str; 4] = ["\u{feff}日🙂\r\nx\ry\nz", "", "日\r\n", "a𠮷b\r\n文書"];
const CASES: usize = 228;
const CATALOG: &str = include_str!("../../../conformance/cases.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOracle {
    version: u32,
    #[serde(deserialize_with = "objects")]
    fixtures: Vec<Fixture>,
    #[serde(deserialize_with = "objects")]
    cases: Vec<RawCase>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    id: usize,
    text: String,
    length: u64,
    lines: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    id: String,
    operation: Operation,
    fixture: usize,
    mismatch: u8,
    encoding: u8,
    args: Vec<String>,
    #[serde(deserialize_with = "object")]
    expected: Expected,
}
#[derive(Deserialize, Clone, Copy)]
#[serde(try_from = "String")]
enum Operation {
    Position,
    Offset,
}
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
enum Expected {
    Position(Coordinate),
    Offset(Offset),
    Error(Rejection),
}
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Coordinate {
    line: u64,
    character: u64,
}
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Offset {
    offset: u64,
}
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Rejection {
    error: ErrorKind,
}
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(try_from = "String")]
enum ErrorKind {
    Bounds,
    ScalarBoundary,
    LineTerminator,
    Position,
    SnapshotMismatch,
}
impl TryFrom<String> for Operation {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "position" => Ok(Self::Position),
            "offset" => Ok(Self::Offset),
            _ => Err("unknown operation".into()),
        }
    }
}
impl TryFrom<String> for ErrorKind {
    type Error = String;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "Bounds" => Ok(Self::Bounds),
            "ScalarBoundary" => Ok(Self::ScalarBoundary),
            "LineTerminator" => Ok(Self::LineTerminator),
            "Position" => Ok(Self::Position),
            "SnapshotMismatch" => Ok(Self::SnapshotMismatch),
            _ => Err("unknown source error".into()),
        }
    }
}
#[derive(Clone, Copy)]
enum Mismatch {
    None,
    Revision,
    Content,
    Source,
}
enum Input {
    Position(u64),
    Offset(Position),
}
struct Case {
    id: String,
    fixture: usize,
    mismatch: Mismatch,
    encoding: PositionEncoding,
    input: Input,
    expected: Expected,
}
struct Oracle {
    fixtures: Vec<Fixture>,
    cases: Vec<Case>,
}
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 1_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn decimal(raw: &str) -> Result<u64, String> {
    if raw.is_empty()
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("noncanonical u64".into());
    }
    raw.parse().map_err(|error| format!("invalid u64: {error}"))
}
fn parse(raw: &str) -> Result<Oracle, String> {
    let Object(raw): Object<RawOracle> =
        serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if raw.version != 1 || raw.fixtures.len() != TEXTS.len() || raw.cases.len() != CASES {
        return Err("version or inventory mismatch".into());
    }
    for (id, fixture) in raw.fixtures.iter().enumerate() {
        if fixture.id != id
            || fixture.text != TEXTS[id]
            || fixture.length != fixture.text.len() as u64
            || fixture.lines == 0
        {
            return Err("fixture identity or size mismatch".into());
        }
    }
    let mut cases = Vec::with_capacity(CASES);
    for (index, case) in raw.cases.into_iter().enumerate() {
        if case.id != format!("case-{index:03}") || case.fixture >= TEXTS.len() {
            return Err("case inventory or fixture selector mismatch".into());
        }
        let mismatch = match case.mismatch {
            0 => Mismatch::None,
            1 => Mismatch::Revision,
            2 => Mismatch::Content,
            3 => Mismatch::Source,
            _ => return Err("unknown mismatch selector".into()),
        };
        let encoding = match case.encoding {
            0 => PositionEncoding::Utf8,
            1 => PositionEncoding::Utf16,
            2 => PositionEncoding::Utf32,
            _ => return Err("unknown encoding selector".into()),
        };
        let input = match (case.operation, case.args.as_slice()) {
            (Operation::Position, [offset]) => Input::Position(decimal(offset)?),
            (Operation::Offset, [line, character]) => Input::Offset(Position {
                line: decimal(line)?,
                character: decimal(character)?,
            }),
            _ => return Err("operation argument count".into()),
        };
        match (&input, &case.expected) {
            (Input::Position(_), Expected::Offset(_))
            | (Input::Offset(_), Expected::Position(_)) => {
                return Err("operation result shape".into());
            }
            _ => {}
        }
        cases.push(Case {
            id: case.id,
            fixture: case.fixture,
            mismatch,
            encoding,
            input,
            expected: case.expected,
        });
    }
    Ok(Oracle {
        fixtures: raw.fixtures,
        cases,
    })
}
fn snapshot(
    text: &str,
    revision: u64,
    id: &str,
    budget: &mut Budget,
) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        revision,
        "memory:source-oracle".into(),
        text.as_bytes().to_vec(),
        budget,
    )
    .map_err(|error| format!("snapshot construction: {error:?}"))
}
fn rejected(error: SourceError) -> Result<Expected, String> {
    let error = match error {
        SourceError::Bounds => ErrorKind::Bounds,
        SourceError::ScalarBoundary => ErrorKind::ScalarBoundary,
        SourceError::LineTerminator => ErrorKind::LineTerminator,
        SourceError::Position => ErrorKind::Position,
        SourceError::SnapshotMismatch => ErrorKind::SnapshotMismatch,
        other => return Err(format!("unexpected source failure: {other:?}")),
    };
    Ok(Expected::Error(Rejection { error }))
}
fn execute(oracle: &Oracle) -> Result<usize, String> {
    for fixture in &oracle.fixtures {
        let mut budget = budget();
        let source = snapshot(&fixture.text, 0, "source-oracle", &mut budget)?;
        let index = LineIndex::new(&source, &mut budget).map_err(|error| format!("{error:?}"))?;
        if index.line_count() != fixture.lines {
            return Err(format!("fixture {} line count mismatch", fixture.id));
        }
    }
    let mut executed = 0;
    for case in &oracle.cases {
        let mut budget = budget();
        let text = &oracle.fixtures[case.fixture].text;
        let original = snapshot(text, 0, "source-oracle", &mut budget)?;
        let index = LineIndex::new(&original, &mut budget).map_err(|error| format!("{error:?}"))?;
        let received = match case.mismatch {
            Mismatch::None => original,
            Mismatch::Revision => snapshot(text, 1, "source-oracle", &mut budget)?,
            Mismatch::Content => snapshot(&format!("{text}!"), 0, "source-oracle", &mut budget)?,
            Mismatch::Source => snapshot(text, 0, "other-source", &mut budget)?,
        };
        let observed = match case.input {
            Input::Position(offset) => match index.position(&received, offset, case.encoding) {
                Ok(position) => Expected::Position(Coordinate {
                    line: position.line,
                    character: position.character,
                }),
                Err(error) => rejected(error)?,
            },
            Input::Offset(position) => match index.offset(&received, position, case.encoding) {
                Ok(offset) => Expected::Offset(Offset { offset }),
                Err(error) => rejected(error)?,
            },
        };
        if observed != case.expected {
            return Err(format!(
                "{}: expected {:?}, observed {observed:?}",
                case.id, case.expected
            ));
        }
        executed += 1;
    }
    Ok(executed)
}
#[test]
fn all_fixed_source_positions_execute() -> Result<(), String> {
    let oracle = parse(ORACLE)?;
    let count = execute(&oracle)?;
    assert_eq!(count, CASES);
    println!("source-position fixed oracle: {count} cases executed");
    Ok(())
}

fn catalog_matches_oracle(catalog: &str, oracle: &Oracle) -> Result<(), String> {
    use serde_json::{Value, json};
    let catalog: Value = serde_json::from_str(catalog).map_err(|error| error.to_string())?;
    let cases = catalog["cases"].as_array().ok_or("catalog cases array")?;
    let selected: Vec<_> = cases
        .iter()
        .filter(|case| case["id"] == "E-unicode")
        .collect();
    if selected.len() != 1 {
        return Err("one E-unicode catalog case required".into());
    }
    let selected = selected[0];
    // Independent byte/unit counts for a + supplementary-plane 𠮷 + b +
    // CRLF + 文書. Lock the named catalog input to the executed fixed oracle.
    let fixture = oracle.fixtures.get(3).ok_or("catalog fixture missing")?;
    if selected["source"] != fixture.text
        || selected["byte_range"] != json!([1, 5])
        || selected["utf16_range"] != json!([[0, 1], [0, 3]])
        || selected["utf32_range"] != json!([[0, 1], [0, 2]])
        || fixture.length != 14
        || fixture.lines != 2
    {
        return Err("E-unicode catalog and fixed fixture differ".into());
    }
    for (encoding, columns) in [
        (PositionEncoding::Utf16, [1, 3]),
        (PositionEncoding::Utf32, [1, 2]),
    ] {
        for (offset, character) in [1, 5].into_iter().zip(columns) {
            let position = Expected::Position(Coordinate { line: 0, character });
            let reverse = Expected::Offset(Offset { offset });
            let bound = |case: &&Case| {
                case.fixture == 3
                    && matches!(case.mismatch, Mismatch::None)
                    && case.encoding == encoding
            };
            let forward = oracle.cases.iter().filter(bound).any(|case| {
                matches!(case.input, Input::Position(value) if value == offset)
                    && case.expected == position
            });
            let backward = oracle.cases.iter().filter(bound).any(|case| {
                matches!(case.input, Input::Offset(Position { line: 0, character: value }) if value == character)
                    && case.expected == reverse
            });
            if !forward || !backward {
                return Err("catalog range endpoint missing from a conversion direction".into());
            }
        }
    }
    Ok(())
}

#[test]
fn catalog_unicode_range_is_executed_in_both_directions() -> Result<(), String> {
    use serde_json::{Value, json};
    let oracle = parse(ORACLE)?;
    catalog_matches_oracle(CATALOG, &oracle)?;
    let original: Value = serde_json::from_str(CATALOG).map_err(|error| error.to_string())?;
    for (key, changed) in [
        ("source", json!("a🙂b\r\n文書")),
        ("byte_range", json!([1, 4])),
        ("utf16_range", json!([[0, 1], [0, 2]])),
        ("utf32_range", json!([[0, 1], [0, 3]])),
    ] {
        let mut changed_catalog = original.clone();
        let cases = changed_catalog["cases"]
            .as_array_mut()
            .ok_or("catalog cases")?;
        let case = cases
            .iter_mut()
            .find(|case| case["id"] == "E-unicode")
            .ok_or("catalog case")?;
        case[key] = changed;
        assert!(catalog_matches_oracle(&changed_catalog.to_string(), &oracle).is_err());
    }
    let mut missing = parse(ORACLE)?;
    missing.cases.retain(|case| case.fixture != 3);
    assert!(catalog_matches_oracle(CATALOG, &missing).is_err());
    for remove_forward in [true, false] {
        let mut missing = parse(ORACLE)?;
        missing.cases.retain(|case| {
            case.fixture != 3 || matches!(case.input, Input::Position(_)) != remove_forward
        });
        assert!(catalog_matches_oracle(CATALOG, &missing).is_err());
    }
    for encoding in [PositionEncoding::Utf16, PositionEncoding::Utf32] {
        for offset in [1, 5] {
            for forward in [true, false] {
                let mut missing = parse(ORACLE)?;
                let character = match (encoding, offset) {
                    (_, 1) => 1,
                    (PositionEncoding::Utf16, _) => 3,
                    _ => 2,
                };
                missing.cases.retain(|case| {
                    let endpoint = match case.input {
                        Input::Position(value) => forward && value == offset,
                        Input::Offset(Position {
                            line,
                            character: value,
                        }) => !forward && line == 0 && value == character,
                    };
                    !(case.fixture == 3 && case.encoding == encoding && endpoint)
                });
                assert!(catalog_matches_oracle(CATALOG, &missing).is_err());
            }
        }
    }
    for duplicate in [false, true] {
        let mut changed_catalog = original.clone();
        let cases = changed_catalog["cases"]
            .as_array_mut()
            .ok_or("catalog cases")?;
        if duplicate {
            let selected = cases
                .iter()
                .find(|case| case["id"] == "E-unicode")
                .ok_or("catalog case")?
                .clone();
            cases.push(selected);
        } else {
            cases.retain(|case| case["id"] != "E-unicode");
        }
        assert!(catalog_matches_oracle(&changed_catalog.to_string(), &oracle).is_err());
    }
    Ok(())
}
#[test]
fn malformed_inventory_and_schema_are_rejected() -> Result<(), String> {
    use serde_json::{Value, json};
    let original: Value = serde_json::from_str(ORACLE).map_err(|error| error.to_string())?;
    let mutations = [
        ("/version", json!(true)),
        ("/fixtures", json!([])),
        ("/fixtures/0/text", json!("\u{feff}月🙂\r\nx\ry\nz")),
        ("/fixtures/0/id", json!(false)),
        ("/cases", json!([])),
        ("/cases/1/id", json!("case-000")),
        ("/cases/0/id", json!("unknown")),
        ("/cases/0/fixture", json!(4)),
        ("/cases/0/mismatch", json!(4)),
        ("/cases/0/encoding", json!(3)),
        ("/cases/0/operation", json!("unknown")),
        ("/cases/0/operation", json!({"position":null})),
        ("/cases/0/args", json!([])),
        ("/cases/0/args", json!([0])),
        ("/cases/0/args", json!(["18446744073709551616"])),
        ("/cases/0/args", json!(["00"])),
        ("/cases/0/args", json!(["+0"])),
        ("/cases/0/args", json!(["-1"])),
        ("/cases/0/expected", json!({"offset":0})),
        (
            "/cases/0/expected",
            json!({"line":0,"character":0,"extra":1}),
        ),
        ("/cases/0/expected", json!({"error":"Unknown"})),
        ("/cases/0/expected", json!({"error":{"Bounds":null}})),
        ("/cases/0/expected", json!([0, 0])),
        ("/cases/1/expected", json!([0])),
        ("/cases/0/expected", json!(["Bounds"])),
        ("/fixtures/0", json!([0, TEXTS[0], 17, 4])),
        (
            "/cases/0",
            json!(["case-000", "position", 0, 0, 0, ["0"], {"line":0,"character":0}]),
        ),
    ];
    for (path, replacement) in mutations {
        let mut value = original.clone();
        *value
            .pointer_mut(path)
            .ok_or_else(|| format!("missing mutation path {path}"))? = replacement;
        assert!(
            parse(&value.to_string()).is_err(),
            "accepted mutation at {path}"
        );
    }
    assert!(parse(&json!([1, original["fixtures"], original["cases"]]).to_string()).is_err());
    for path in ["", "/fixtures/0", "/cases/0"] {
        let mut value = original.clone();
        value
            .pointer_mut(path)
            .and_then(Value::as_object_mut)
            .ok_or("mutation object")?
            .insert("extra".into(), json!(1));
        assert!(
            parse(&value.to_string()).is_err(),
            "accepted extra field at {path}"
        );
    }
    for field in [
        "\"version\": 1",
        "\"id\": 0",
        "\"operation\": \"position\"",
        "\"line\": 0",
        "\"error\": \"Bounds\"",
    ] {
        let duplicate = ORACLE.replacen(field, &format!("{field}, {field}"), 1);
        assert_ne!(duplicate, ORACLE, "duplicate fixture did not match {field}");
        assert!(parse(&duplicate).is_err(), "accepted duplicate {field}");
    }
    Ok(())
}
#[test]
fn wrong_expected_results_and_line_counts_fail_execution() -> Result<(), String> {
    for expected in [
        Expected::Position(Coordinate {
            line: 0,
            character: 1,
        }),
        Expected::Error(Rejection {
            error: ErrorKind::Bounds,
        }),
    ] {
        let mut oracle = parse(ORACLE)?;
        oracle.cases[0].expected = expected;
        let error = execute(&oracle)
            .err()
            .ok_or("wrong expectation was accepted")?;
        assert!(error.contains("case-000"));
    }
    let mut oracle = parse(ORACLE)?;
    oracle.fixtures[0].lines += 1;
    assert!(execute(&oracle).is_err());
    Ok(())
}
