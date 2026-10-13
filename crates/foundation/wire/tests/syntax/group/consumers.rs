use super::*;
use nepl3_core::{
    syntax::group::{CaptureField, ForeignClosureGroup, GroupError},
    value_codec::FoundationValueCodec,
};

#[test]
fn borrowed_closure_validation_and_encoding_match_owned_boundaries() -> TestResult {
    let (_, registry, owner) = fixture()?;
    let checked = owner
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let closure = ForeignClosure::capture_at(
        &checked,
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for malformed in [false, true] {
        let mut input = closure.clone();
        if malformed {
            input.syntax.environment.id ^= 1;
        }
        let mut owned_budget = budget();
        let mut view_budget = budget();
        let owned = input
            .validate(
                &registry,
                &mut owned_budget,
                &mut SourceAdmission::default(),
            )
            .map(|_| ());
        let borrowed =
            input
                .view()
                .validate(&registry, &mut view_budget, &mut SourceAdmission::default());
        assert_eq!(owned, borrowed);
        assert_eq!(owned_budget.usage(), view_budget.usage());
    }
    let sources = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&registry, &sources, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
    let owned = codec
        .encode_foreign_closure(&closure, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut fresh = SourceAdmission::default();
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(&registry, &sources, &mut fresh)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        owned,
        codec
            .encode_foreign_view(&closure.view(), &mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    for reason in [
        StopReason::Cancelled,
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::SourceLimit,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::DepthLimit => limits.depth = 0,
            StopReason::SourceLimit => limits.source_bytes = 0,
            _ => {}
        }
        let mut a = Budget::new(limits);
        let mut z = Budget::new(limits);
        if reason == StopReason::Cancelled {
            a.cancel();
            z.cancel();
        }
        let owned = closure
            .validate(&registry, &mut a, &mut SourceAdmission::default())
            .map(|_| ());
        let borrowed = closure
            .view()
            .validate(&registry, &mut z, &mut SourceAdmission::default());
        assert_eq!(owned, borrowed);
        assert_eq!(a.usage(), z.usage());
        assert_eq!(a.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn capture_source_depths_compose_caller_and_preserve_exact_stops() -> TestResult {
    let (_, registry, owner) = fixture()?;
    let checked = owner
        .validate(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let fields = [CaptureField {
        owner: 0,
        node: NodeRef(0),
        field: 0,
    }; 2];
    assert_eq!(
        ForeignClosureGroup::capture_at_depths(
            &[&checked],
            &fields,
            &[1],
            &registry,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(GroupError::DepthCount)
    );
    let run = |b: &mut Budget| {
        b.with_depth_at_least(9, |b| {
            ForeignClosureGroup::capture_at_depths(
                &[&checked],
                &fields,
                &[1, 17],
                &registry,
                b,
                &mut SourceAdmission::default(),
            )
        })
    };
    let mut complete = budget();
    let group = run(&mut complete).map_err(|e| format!("{e:?}"))?;
    assert_eq!(complete.current_depth(), 0);
    assert!(complete.usage().depth >= 26);
    assert_eq!(group.entries.len(), 2);
    assert_eq!(group.provenances.len(), 1);
    let used = complete.usage();
    for exact in [true, false] {
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::SourceLimit,
        ] {
            let mut limits = budget().limits();
            let shortage = u64::from(!exact);
            match reason {
                StopReason::WorkLimit => limits.work = used.work - shortage,
                StopReason::AllocationLimit => {
                    limits.allocation_units = used.allocation_units - shortage
                }
                StopReason::DepthLimit => limits.depth = used.depth - shortage,
                StopReason::SourceLimit => limits.source_bytes = used.source_bytes - shortage,
                _ => {}
            }
            let mut limited = Budget::new(limits);
            let result = run(&mut limited);
            if exact {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, group);
            } else {
                assert_eq!(result, Err(GroupError::Stopped(reason)));
                assert_eq!(limited.poll(), Err(reason));
            }
            assert_eq!(limited.current_depth(), 0);
        }
    }
    Ok(())
}

#[test]
fn capture_and_group_copy_store_growing_provenance_once_per_owner() -> TestResult {
    let run = |origins: usize, guests: usize| -> Result<_, String> {
        let (_, registry, mut owner) = fixture()?;
        let origin = owner.origins.first().ok_or("origin")?.clone();
        owner.origins.resize(origins, origin);
        let checked = owner
            .validate(&registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let fields = vec![
            CaptureField {
                owner: 0,
                node: NodeRef(0),
                field: 0
            };
            guests
        ];
        let mut capture = budget();
        let group = ForeignClosureGroup::capture(
            &[&checked],
            &fields,
            &registry,
            &mut capture,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(group.provenances.len(), 1);
        assert_eq!(group.provenances[0].origins.len(), origins);
        let mut copy = budget();
        assert_eq!(
            group
                .clone_with_budget(&mut copy)
                .map_err(|e| format!("{e:?}"))?,
            group
        );
        Ok((
            capture.usage().allocation_units,
            copy.usage().allocation_units,
        ))
    };
    let a = run(128, 16)?;
    let b = run(256, 16)?;
    let c = run(128, 32)?;
    let d = run(256, 32)?;
    assert!(b.0 > a.0 && b.1 > a.1);
    assert_eq!(b.0 - a.0, d.0 - c.0);
    assert_eq!(b.1 - a.1, d.1 - c.1);
    Ok(())
}
