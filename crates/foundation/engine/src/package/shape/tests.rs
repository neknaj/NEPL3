use super::*;
use alloc::{format, vec, vec::Vec};
use nepl3_core::{
    budget::{Limits, StopReason},
    schema::{NamedType, SchemaDescriptor, SchemaError},
};

fn budget(work: u64) -> Budget {
    Budget::new(Limits {
        work,
        allocation_units: 10_000_000,
        nodes: 100_000,
        depth: 100,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}
fn fixture(count: usize) -> Result<(SchemaRegistry, KindRef), SchemaError> {
    fixture_shape(count, true)
}
fn fixture_shape(count: usize, record: bool) -> Result<(SchemaRegistry, KindRef), SchemaError> {
    let mut setup = budget(10_000_000);
    let descriptor = SchemaDescriptor {
        package: "fixture.kind".into(),
        revision: 1,
        operations: vec![],
        types: (0..count)
            .map(|index| NamedType {
                name: format!("Record{index:04}"),
                shape: if record {
                    TypeShape::Record { fields: vec![] }
                } else {
                    TypeShape::Variant { variants: vec![] }
                },
                constraints: vec![],
            })
            .collect::<Vec<_>>(),
    };
    let schema = descriptor.reference(&mut setup)?;
    let mut registry = SchemaRegistry::default();
    registry.register(schema.clone(), descriptor, &mut setup)?;
    Ok((
        registry,
        KindRef {
            schema,
            local_kind: 0,
        },
    ))
}

#[test]
fn record_lookup_admits_schema_before_errors_and_keeps_stop() -> Result<(), SchemaError> {
    let (registry, mut kind) = fixture(1)?;
    kind.local_kind = u64::MAX;
    let mut zero = budget(0);
    assert_eq!(
        record_kind(&kind, &registry, &mut zero),
        Err(PackageError::Stopped(StopReason::WorkLimit))
    );
    let used = zero.usage();
    kind.local_kind = 0;
    assert_eq!(
        record_kind(&kind, &registry, &mut zero),
        Err(PackageError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(zero.usage(), used);
    let mut cancelled = budget(1000);
    cancelled.stop(StopReason::Cancelled);
    assert_eq!(
        record_kind(&kind, &registry, &mut cancelled),
        Err(PackageError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(cancelled.usage().work, 0);
    Ok(())
}

#[test]
fn numeric_record_lookup_does_not_rescan_type_names() -> Result<(), PackageError> {
    let mut costs = Vec::new();
    for count in [1, 128] {
        let (registry, kind) = fixture(count)?;
        let mut measured = budget(100_000);
        assert!(record_kind(&kind, &registry, &mut measured)?.is_empty());
        costs.push(measured.usage().work);
        assert_eq!(measured.usage().allocation_units, 0);
        // One registry entry: lookup setup + selected package comparison +
        // exact schema identity comparison + one numeric index access.
        let expected = 1
            + (2 * kind.schema.package.len() as u64 + 9)
            + (kind.schema.package.len() as u64 + 41)
            + 1;
        assert_eq!(measured.usage().work, expected);
        let mut exact = budget(expected);
        assert!(record_kind(&kind, &registry, &mut exact)?.is_empty());
        let mut short = budget(expected - 1);
        assert_eq!(
            record_kind(&kind, &registry, &mut short),
            Err(PackageError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(short.usage().work, expected - 1);
        let mut invalid = kind.clone();
        invalid.local_kind = u64::MAX;
        assert_eq!(
            record_kind(&invalid, &registry, &mut budget(expected - 1)),
            Err(PackageError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(
            record_kind(&invalid, &registry, &mut budget(expected)),
            Err(PackageError::Schema(SchemaError::UnknownType))
        );
        invalid.schema.revision += 1;
        assert_eq!(
            record_kind(&invalid, &registry, &mut budget(expected)),
            Err(PackageError::Schema(SchemaError::UnknownSchema))
        );
    }
    // Numeric kind IDs already index the registry's canonical type order.
    // Adding unrelated definitions must not add a second name-search pass.
    assert_eq!(costs[0], costs[1]);
    Ok(())
}

#[test]
fn record_lookup_preserves_forged_identity_and_non_record_errors() -> Result<(), PackageError> {
    let (registry, mut kind) = fixture_shape(1, false)?;
    assert_eq!(
        record_kind(&kind, &registry, &mut budget(1000)),
        Err(PackageError::KindShape)
    );
    kind.schema.digest.0[0] ^= 1;
    assert_eq!(
        record_kind(&kind, &registry, &mut budget(1000)),
        Err(PackageError::Schema(SchemaError::UnknownSchema))
    );
    Ok(())
}
