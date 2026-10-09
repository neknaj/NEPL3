use super::*;
use nepl3_core::{
    budget::Resource,
    operation::{Invoke, request::RequestBindingError as E},
    source::{SourceId, SourceSnapshot},
    syntax::ResourceContent,
};
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn source(id: &str, revision: u64, uri: &str, data: &[u8]) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        revision,
        uri.into(),
        data.to_vec(),
        &mut budget(),
    )
    .map_err(error)
}
fn fixture() -> Result<Invoke, String> {
    let c = saved();
    Ok(Invoke {
        request_id: 8,
        operation: c.provider,
        input: c.state.clone(),
        environment: c.state,
        sources: vec![
            source("a", 1, "memory:a", b"one")?,
            source("b", 2, "memory:b", b"two")?,
        ],
        resources: vec![
            ResourceContent {
                id: "a".into(),
                digest: Digest::of(b"one"),
                bytes: b"one".to_vec(),
            },
            ResourceContent {
                id: "b".into(),
                digest: Digest::of(b"two"),
                bytes: b"two".to_vec(),
            },
        ],
        limits: budget().limits(),
    })
}
#[test]
fn complete_request_comparison_rejects_each_field_and_order_change() -> Result<(), String> {
    let a = fixture()?;
    let mut cases = Vec::new();
    let mut x = a.clone();
    x.request_id += 1;
    cases.push((x, E::RequestId));
    let mut x = a.clone();
    x.operation.name.push('x');
    cases.push((x, E::Operation));
    let mut x = a.clone();
    x.operation.schema.package.push('x');
    cases.push((x, E::Operation));
    let mut x = a.clone();
    x.operation.schema.revision += 1;
    cases.push((x, E::Operation));
    let mut x = a.clone();
    x.operation.schema.digest.0[0] ^= 1;
    cases.push((x, E::Operation));
    let mut x = a.clone();
    match &mut x.input {
        TypedValue::Record(r) => r.fields.push(NdfValue::Unit),
        TypedValue::Variant(v) => v.fields.push(NdfValue::Unit),
    };
    cases.push((x, E::Input));
    let mut x = a.clone();
    match &mut x.environment {
        TypedValue::Record(r) => r.fields.push(NdfValue::Unit),
        TypedValue::Variant(v) => v.fields.push(NdfValue::Unit),
    };
    cases.push((x, E::Environment));
    for replacement in [
        source("a", 1, "memory:changed", b"one")?,
        source("a", 1, "memory:a", b"changed")?,
        source("a", 2, "memory:a", b"one")?,
        source("other", 1, "memory:a", b"one")?,
    ] {
        let mut x = a.clone();
        x.sources[0] = replacement;
        cases.push((x, E::Sources));
    }
    let mut x = a.clone();
    x.sources.push(source("c", 0, "memory:c", b"third")?);
    cases.push((x, E::Sources));
    let mut x = a.clone();
    x.resources.push(x.resources[0].clone());
    cases.push((x, E::Resources));
    let mut x = a.clone();
    x.sources.swap(0, 1);
    cases.push((x, E::Sources));
    let mut x = a.clone();
    x.sources.pop();
    cases.push((x, E::Sources));
    for field in 0..5 {
        let mut x = a.clone();
        match field {
            0 => x.resources[0].id.push('x'),
            1 => x.resources[0].digest.0[0] ^= 1,
            2 => x.resources[0].bytes.push(0),
            3 => x.resources.swap(0, 1),
            _ => {
                x.resources.pop();
            }
        };
        cases.push((x, E::Resources));
    }
    for field in 0..8 {
        let mut x = a.clone();
        match field {
            0 => x.limits.source_bytes -= 1,
            1 => x.limits.work -= 1,
            2 => x.limits.depth -= 1,
            3 => x.limits.nodes -= 1,
            4 => x.limits.allocation_units -= 1,
            5 => x.limits.output_bytes -= 1,
            6 => x.limits.diagnostics -= 1,
            _ => x.limits.events -= 1,
        };
        cases.push((x, E::Limits));
    }
    for (changed, reason) in cases {
        let mut b = budget();
        assert_eq!(changed.check_saved(&a, &mut b), Err(reason));
        assert!(b.usage().work > 0);
        assert_eq!(b.poll(), Ok(()));
    }
    // Equal content from another execution attempt is intentionally indistinguishable.
    assert_eq!(a.clone().check_saved(&a, &mut budget()), Ok(()));
    assert_eq!(fixture()?.check_saved(&a, &mut budget()), Ok(()));
    Ok(())
}
#[test]
fn comparison_stops_preserve_prefix_and_do_not_admit_source_bytes() -> Result<(), String> {
    let a = fixture()?;
    let mut full = budget();
    a.check_saved(&a, &mut full).map_err(error)?;
    assert_eq!(full.usage().source_bytes, 0);
    for (limits, reason) in [
        (
            Limits {
                work: 0,
                ..budget().limits()
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                work: full.usage().work - 1,
                ..budget().limits()
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: 0,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                depth: 0,
                ..budget().limits()
            },
            StopReason::DepthLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        assert_eq!(a.check_saved(&a, &mut b), Err(E::Stopped(reason)));
        let prefix = b.usage();
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(a.check_saved(&a, &mut b), Err(E::Stopped(reason)));
        assert_eq!(b.usage(), prefix);
    }
    let mut b = budget();
    b.charge(Resource::Work, 91).map_err(error)?;
    b.stop(StopReason::Cancelled);
    let prefix = b.usage();
    assert_eq!(
        a.check_saved(&a, &mut b),
        Err(E::Stopped(StopReason::Cancelled))
    );
    assert_eq!(b.usage(), prefix);
    Ok(())
}
#[test]
fn independently_stored_sources_and_long_resource_bytes_pay_before_comparison() -> Result<(), String>
{
    let a = fixture()?;
    let other = fixture()?;
    let mut shared = budget();
    a.check_saved(&a, &mut shared).map_err(error)?;
    let mut independent = budget();
    other.check_saved(&a, &mut independent).map_err(error)?;
    #[cfg(target_has_atomic = "ptr")]
    assert!(independent.usage().work > shared.usage().work);
    let mut long = a.clone();
    long.resources[0].id = "x".repeat(10_000);
    long.resources[0].bytes = vec![0; 10_000];
    let mut b = Budget::new(Limits {
        work: independent.usage().work + 100,
        ..budget().limits()
    });
    assert_eq!(
        long.check_saved(&long.clone(), &mut b),
        Err(E::Stopped(StopReason::WorkLimit))
    );
    assert!(b.usage().work < b.limits().work);
    let mut b = budget();
    b.with_depth_at_least(7, |b| a.check_saved(&other, b))
        .map_err(error)?;
    assert_eq!(b.current_depth(), 0);
    assert!(b.usage().depth >= 7);
    Ok(())
}
