use super::*;

fn budget() -> Budget {
    Budget::new(Limits {
        depth: 100,
        work: 100,
        ..Limits::default()
    })
}

#[test]
fn measured_depth_excludes_prior_usage_and_restores_nested_observation() -> Result<(), StopReason> {
    let mut b = budget();
    b.observe_depth(90)?;
    let (_, depth) = b.measure_depth(|b| {
        b.with_depth_at_least(7, |b| {
            let (_, inner) = b.measure_depth(|b| b.with_depth(|b| b.observe_depth(2)))?;
            assert_eq!(inner, 3);
            // A host observation contributes only the newly supplied depth,
            // while Usage still includes the earlier high-water mark of 90.
            let (_, observed) = b.measure_depth(|b| {
                b.record_observed_usage(Usage {
                    depth: 11,
                    ..Usage::default()
                })
            })?;
            assert_eq!(observed, 4);
            Ok::<_, StopReason>(())
        })
    })?;
    assert_eq!(depth, 11);
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.usage().depth, 90);
    let (_, empty) = b.measure_depth(|_| Ok::<_, StopReason>(()))?;
    assert_eq!(empty, 0);
    Ok(())
}

#[test]
fn failed_nested_measurement_preserves_charges_and_sticky_stops() -> Result<(), StopReason> {
    let mut b = budget();
    let (_, depth) = b.measure_depth(|b| {
        let rejected: Result<((), u64), StopReason> = b.measure_depth(|b| {
            b.observe_depth(8)?;
            b.charge(Resource::Work, 3)?;
            Err(StopReason::Cancelled)
        });
        assert_eq!(rejected, Err(StopReason::Cancelled));
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(depth, 8);
    assert_eq!(b.usage().work, 3);
    let stopped: Result<((), u64), StopReason> = b.measure_depth(|b| {
        assert_eq!(b.observe_depth(101), Err(StopReason::DepthLimit));
        Ok(())
    });
    assert_eq!(stopped, Err(StopReason::DepthLimit));
    assert_eq!(b.usage().depth, 8);
    let mut called = false;
    let stopped: Result<((), u64), StopReason> = b.measure_depth(|_| {
        called = true;
        Ok(())
    });
    assert_eq!(stopped, Err(StopReason::DepthLimit));
    assert!(!called);
    Ok(())
}
