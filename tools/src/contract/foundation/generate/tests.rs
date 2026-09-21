use super::*;
use nepl3_core::schema::SchemaError;

mod host;

#[test]
fn compiled_projection_preserves_schema_values_and_identity() -> Result<()> {
    // Every string here is data, including text that previously matched a
    // whole-source replacement. The expected descriptor is constructed by hand.
    let expected = SchemaDescriptor {
        package: "fixture".into(),
        revision: 1,
        types: vec![NamedType {
            name: "alloc::Marker".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![
                "keep crate::budget:: foundation --write; Generated from interfaces/contracts.json via interfaces/foundation.json.".into(),
            ],
        }],
        operations: vec![],
    };
    let emitted = source_with(
        &expected,
        Output::domain("interfaces/fixture.json", "fixture").host(),
    )?;
    // The checked fixture is compiled as Rust, so this equality ties the emitted
    // code to a descriptor we can execute and compare with independent values.
    assert_eq!(emitted, include_str!("tests/host.rs"));
    let mut budget = crate::contract::foundation::budget();
    let actual = host::descriptor(&mut budget).map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, expected);
    assert_eq!(
        actual
            .reference(&mut budget)
            .map_err(|e| format!("{e:?}"))?,
        expected
            .reference(&mut budget)
            .map_err(|e| format!("{e:?}"))?,
    );
    Ok(())
}
