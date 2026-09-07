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
