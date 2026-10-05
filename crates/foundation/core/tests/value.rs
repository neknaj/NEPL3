use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    value::NdfValue,
};

fn limits() -> Limits {
    Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 1024,
        nodes: 1_000_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    }
}

#[test]
fn borrowed_payload_is_checked_against_the_selected_output_type() -> Result<(), String> {
    use nepl3_core::{
        schema::*,
        value::{Record, TypedValue},
    };
    let err = |e| format!("{e:?}");
    let mut b = Budget::new(limits());
    let descriptor = foundation::descriptor(&mut b).map_err(err)?;
    let schema = descriptor.reference(&mut b).map_err(err)?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut b)
        .map_err(err)?;
    registry.finalize(&mut b).map_err(err)?;
    let value = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "TraceOverflow".into(),
        fields: vec![NdfValue::U64(3)],
    });
    let named = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: schema.package.clone(),
            revision: schema.revision,
            name: name.into(),
        })
    };
    for expected in [
        named("TraceOverflow"),
        TypeDescriptor::TypedValue,
        TypeDescriptor::NdfValue,
    ] {
        registry
            .validate_typed_as(&expected, &value, &mut Budget::new(limits()))
            .map_err(err)?;
    }
    // Both types are registered. Self-schema validity alone cannot establish
    // that a provider returned the output selected by its operation contract.
    assert_eq!(
        registry.validate_typed_as(&named("Limits"), &value, &mut Budget::new(limits())),
        Err(SchemaError::WrongType)
    );
    assert_eq!(
        registry.validate_typed_as(&TypeDescriptor::U64, &value, &mut Budget::new(limits())),
        Err(SchemaError::WrongType)
    );
    let mut forged = value.clone();
    if let TypedValue::Record(v) = &mut forged {
        v.schema.digest.0[0] ^= 1;
    }
    assert_eq!(
        registry.validate_typed_as(&named("TraceOverflow"), &forged, &mut Budget::new(limits())),
        Err(SchemaError::UnknownSchema)
    );
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..limits()
    });
    assert_eq!(
        registry.validate_typed_as(&named("TraceOverflow"), &value, &mut stopped),
        Err(SchemaError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    Ok(())
}
#[test]
fn equality_charges_frontier_before_queueing_wide_input() {
    // The old borrowed implementation expanded every child at Work=1 before
    // discovering the exhausted limit on the next pop. Width must not buy an
    // unmetered scan or allocation, including when exposed by the core API.
    for width in [1, 100_000] {
        let value = NdfValue::List(vec![NdfValue::Unit; width]);
        let mut b = Budget::new(Limits {
            work: 1,
            ..limits()
        });
        assert_eq!(
            value.equal_with_budget(&value, &mut b),
            Err(StopReason::WorkLimit)
        );
        assert_eq!(b.usage().work, 1);
        assert_eq!(
            b.usage().allocation_units,
            core::mem::size_of::<(&NdfValue, &NdfValue, u64)>() as u64
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    }
}
#[test]
fn budgeted_equality_keeps_values_and_composes_caller_depth() -> Result<(), StopReason> {
    let left = NdfValue::List(vec![
        NdfValue::Text("名前".into()),
        NdfValue::Some(Box::new(NdfValue::U64(7))),
    ]);
    let right = left.clone();
    assert!(left.equal_with_budget(&right, &mut Budget::new(limits()))?);
    let right = NdfValue::List(vec![
        NdfValue::Text("名前".into()),
        NdfValue::Some(Box::new(NdfValue::U64(8))),
    ]);
    assert!(!left.equal_with_budget(&right, &mut Budget::new(limits()))?);
    let mut b = Budget::new(Limits {
        depth: 3,
        ..limits()
    });
    assert_eq!(
        b.with_depth_at_least(1, |b| left.equal_with_budget(&left, b)),
        Err(StopReason::DepthLimit)
    );
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn budgeted_scalar_equality_uses_one_logical_frontier_slot() -> Result<(), StopReason> {
    for (left, right, expected) in [
        (NdfValue::Unit, NdfValue::Unit, true),
        (NdfValue::None, NdfValue::None, true),
        (NdfValue::Unit, NdfValue::None, false),
        (NdfValue::Bool(true), NdfValue::Bool(false), false),
        (NdfValue::U64(7), NdfValue::U64(7), true),
        (
            NdfValue::Integer(7_i64.into()),
            NdfValue::Integer(8_i64.into()),
            false,
        ),
        (
            NdfValue::Text("abc".into()),
            NdfValue::Text("abc".into()),
            true,
        ),
        (
            NdfValue::Bytes(vec![1, 2]),
            NdfValue::Bytes(vec![1, 3]),
            false,
        ),
        (NdfValue::List(vec![NdfValue::Unit]), NdfValue::Unit, false),
    ] {
        let mut cap = limits();
        cap.allocation_units = core::mem::size_of::<(&NdfValue, &NdfValue, u64)>() as u64;
        let mut exact = Budget::new(cap);
        assert_eq!(left.equal_with_budget(&right, &mut exact)?, expected);
        assert_eq!(exact.usage().allocation_units, cap.allocation_units);
        cap.allocation_units -= 1;
        assert_eq!(
            left.equal_with_budget(&right, &mut Budget::new(cap)),
            Err(StopReason::AllocationLimit)
        );
    }
    Ok(())
}
