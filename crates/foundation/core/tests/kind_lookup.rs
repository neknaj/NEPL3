use nepl3_core::{budget::*, schema::*, value::SchemaRef};
fn setup_budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 10_000,
        depth: 100,
        ..Limits::default()
    })
}
fn work_budget(work: u64) -> Budget {
    Budget::new(Limits {
        work,
        ..Limits::default()
    })
}
fn fixture(padding: usize, names: &[&str]) -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut registry = SchemaRegistry::default();
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("pad{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut setup_budget())?, d, &mut setup_budget())?;
    }
    let d = SchemaDescriptor {
        package: "owner".into(),
        revision: 1,
        types: names
            .iter()
            .map(|name| NamedType {
                name: (*name).into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            })
            .collect(),
        operations: vec![],
    };
    let r = d.reference(&mut setup_budget())?;
    registry.register(r.clone(), d, &mut setup_budget())?;
    Ok((registry, r))
}
#[test]
fn named_kind_lookup_charges_exact_schema_and_binary_probes() -> Result<(), SchemaError> {
    // Deliberately unsorted: register establishes scalar order A, Alpha,
    // AlphaZZZZZZZZ, β, 界. Probe paths are specified independently here.
    let names = ["界", "Alpha", "β", "A", "AlphaZZZZZZZZ"];
    let cases: [(&str, u64, &[&str]); 5] = [
        ("A", 0, &["AlphaZZZZZZZZ", "Alpha", "A"]),
        ("Alpha", 1, &["AlphaZZZZZZZZ", "Alpha"]),
        ("AlphaZZZZZZZZ", 2, &["AlphaZZZZZZZZ"]),
        ("β", 3, &["AlphaZZZZZZZZ", "界", "β"]),
        ("界", 4, &["AlphaZZZZZZZZ", "界"]),
    ];
    for padding in [0, 32] {
        let (registry, schema) = fixture(padding, &names)?;
        for (name, id, probes) in cases {
            // Entry 1, every 5-byte package comparison 5+5+9,
            // exact identity comparison 5+41, then UTF-8 name comparisons.
            let expected = 1
                + (padding as u64 + 1) * 19
                + 46
                + probes
                    .iter()
                    .map(|p| (p.len() + name.len() + 1) as u64)
                    .sum::<u64>();
            let mut b = work_budget(expected);
            assert_eq!(registry.kind_id_with_budget(&schema, name, &mut b)?, id);
            assert_eq!(
                b.usage(),
                Usage {
                    work: expected,
                    ..Usage::default()
                }
            );
            assert_eq!(registry.kind_id(&schema, name)?, id);
            for limit in [0, 1, expected - 1] {
                let mut b = work_budget(limit);
                assert_eq!(
                    registry.kind_id_with_budget(&schema, name, &mut b),
                    Err(SchemaError::Stopped(StopReason::WorkLimit))
                );
                assert!(b.usage().work <= limit);
                assert_eq!(b.usage().allocation_units, 0);
            }
            let mut cancelled = work_budget(expected);
            cancelled.cancel();
            assert_eq!(
                registry.kind_id_with_budget(&schema, name, &mut cancelled),
                Err(SchemaError::Stopped(StopReason::Cancelled))
            );
            assert_eq!(cancelled.usage(), Usage::default());
        }
    }
    Ok(())
}
#[test]
fn named_kind_lookup_preserves_identity_and_missing_errors() -> Result<(), SchemaError> {
    let (registry, schema) = fixture(0, &["β", "A"])?;
    for case in 0..3 {
        let mut wrong = schema.clone();
        match case {
            0 => wrong.package.push('x'),
            1 => wrong.revision += 1,
            _ => wrong.digest.0[0] ^= 1,
        }
        assert_eq!(
            registry.kind_id_with_budget(&wrong, "A", &mut work_budget(1000)),
            Err(SchemaError::UnknownSchema)
        );
    }
    for name in ["", "Aa", "γ", "界"] {
        assert_eq!(
            registry.kind_id_with_budget(&schema, name, &mut work_budget(1000)),
            Err(SchemaError::UnknownType)
        );
    }
    let (empty, schema) = fixture(0, &[])?;
    let mut b = work_budget(66);
    assert_eq!(
        empty.kind_id_with_budget(&schema, "A", &mut b),
        Err(SchemaError::UnknownType)
    );
    assert_eq!(
        b.usage(),
        Usage {
            work: 66,
            ..Usage::default()
        }
    );
    Ok(())
}

#[test]
fn named_kind_lookup_accounts_for_type_inventory_and_utf8_width() -> Result<(), SchemaError> {
    let long = "界".repeat(512);
    for names in [vec![long.as_str()], vec!["A", "Z", long.as_str()]] {
        let (r, s) = fixture(0, &names)?;
        // One type probes the long target directly. Three types first probe Z.
        let expected = 66
            + (long.len() * 2 + 1) as u64
            + if names.len() == 3 {
                (1 + long.len() + 1) as u64
            } else {
                0
            };
        let mut b = work_budget(expected);
        assert_eq!(
            r.kind_id_with_budget(&s, &long, &mut b)?,
            (names.len() - 1) as u64
        );
        assert_eq!(
            b.usage(),
            Usage {
                work: expected,
                ..Usage::default()
            }
        );
        let mut b = work_budget(expected - 1);
        assert_eq!(
            r.kind_id_with_budget(&s, &long, &mut b),
            Err(SchemaError::Stopped(StopReason::WorkLimit))
        );
        let stopped = b.usage();
        assert_eq!(
            r.kind_id_with_budget(&s, "A", &mut b),
            Err(SchemaError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(b.usage(), stopped);
    }
    Ok(())
}
