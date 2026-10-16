use super::*;
use compact::{GroupAtDepths, encode_at_depths};
use nepl3_core::value_codec::{FoundationCodecError, GroupEncodingError};
fn flatten(e: GroupEncodingError<WireError>) -> WireError {
    match e {
        GroupEncodingError::Structure(e) => e.into(),
        GroupEncodingError::Codec(e) => e,
    }
}
#[test]
fn compact_depths_preserve_bytes_identity_and_registry_authorities() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let (_, _, _, equivalent, _) = setup()?;
    let expected = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut same = budget();
    let actual = encode_at_depths(
        GroupAtDepths {
            group: &g,
            depths: &[2, 40],
            native_registry: &r,
        },
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut same,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, expected);
    assert!(same.usage().depth >= 40);
    let mut separate = budget();
    assert_eq!(
        encode_at_depths(
            GroupAtDepths {
                group: &g,
                depths: &[2, 40],
                native_registry: &equivalent
            },
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut separate
        )
        .map_err(|e| format!("{e:?}"))?,
        expected
    );
    assert!(separate.usage().work > same.usage().work);
    let mut absent = SchemaRegistry::default();
    absent
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        encode_at_depths(
            GroupAtDepths {
                group: &g,
                depths: &[2, 40],
                native_registry: &r
            },
            &a,
            &s,
            &f,
            &absent,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(GroupEncodingError::Codec(_))
    ));
    Ok(())
}
#[test]
fn compact_depths_reject_missing_depths_before_codec_boundary() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let absent = SchemaRegistry::default();
    for depths in [&[][..], &[2][..], &[2, 40, 1][..]] {
        assert!(matches!(
            encode_at_depths(
                GroupAtDepths {
                    group: &g,
                    depths,
                    native_registry: &r
                },
                &a,
                &s,
                &f,
                &absent,
                &mut SourceAdmission::default(),
                &mut budget()
            ),
            Err(GroupEncodingError::Structure(_))
        ));
    }
    Ok(())
}
#[test]
fn compact_depths_limits_stick_inside_active_outer_scope() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let run = |b: &mut Budget, admission: &mut SourceAdmission| -> Result<_, WireError> {
        b.charge(nepl3_core::budget::Resource::Work, 7)?;
        b.with_depth_at_least(4, |b| {
            encode_at_depths(
                GroupAtDepths {
                    group: &g,
                    depths: &[2, 40],
                    native_registry: &r,
                },
                &a,
                &s,
                &f,
                &r,
                admission,
                b,
            )
            .map_err(flatten)
        })
    };
    let mut measured = budget();
    let expected =
        run(&mut measured, &mut SourceAdmission::default()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.current_depth(), 0);
    let u = measured.usage();
    let limits = Limits {
        source_bytes: u.source_bytes,
        work: u.work,
        depth: u.depth,
        nodes: u.nodes,
        allocation_units: u.allocation_units,
        output_bytes: u.output_bytes,
        diagnostics: u.diagnostics,
        events: u.events,
    };
    assert_eq!(
        run(&mut Budget::new(limits), &mut SourceAdmission::default())
            .map_err(|e| format!("{e:?}"))?,
        expected
    );
    for i in 0..6 {
        let mut short = limits;
        let reason = match i {
            0 => {
                short.source_bytes -= 1;
                StopReason::SourceLimit
            }
            1 => {
                short.work -= 1;
                StopReason::WorkLimit
            }
            2 => {
                short.depth -= 1;
                StopReason::DepthLimit
            }
            3 => {
                short.nodes -= 1;
                StopReason::NodeLimit
            }
            4 => {
                short.allocation_units -= 1;
                StopReason::AllocationLimit
            }
            _ => {
                short.output_bytes -= 1;
                StopReason::OutputLimit
            }
        };
        let mut b = Budget::new(short);
        let mut admission = SourceAdmission::default();
        assert_eq!(
            run(&mut b, &mut admission)
                .err()
                .and_then(|e| e.stop_reason()),
            Some(reason)
        );
        assert_eq!(b.current_depth(), 0);
        let retained = b.usage();
        assert_eq!(
            run(&mut b, &mut admission)
                .err()
                .and_then(|e| e.stop_reason()),
            Some(reason)
        );
        assert_eq!(b.usage(), retained);
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        run(&mut cancelled, &mut SourceAdmission::default())
            .err()
            .and_then(|e| e.stop_reason()),
        Some(StopReason::Cancelled)
    );
    Ok(())
}

#[test]
fn compact_distinct_registry_second_validation_stops_are_normalized() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let (_, _, _, equivalent, _) = setup()?;
    let mut measure = budget();
    let mut admission = SourceAdmission::default();
    g.validate_at_depths(&[2, 40], &equivalent, &mut measure, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let first = measure.usage().work;
    g.validate(&r, &mut measure, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let second = measure.usage().work;
    assert!(second > first + 2);
    for work in [first, first + (second - first) / 2, second - 1] {
        let mut b = Budget::new(Limits {
            work,
            ..budget().limits()
        });
        let result = encode_at_depths(
            GroupAtDepths {
                group: &g,
                depths: &[2, 40],
                native_registry: &equivalent,
            },
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut b,
        );
        assert!(
            matches!(
                result,
                Err(GroupEncodingError::Codec(WireError::Stopped(
                    StopReason::WorkLimit
                )))
            ),
            "{result:?}"
        );
    }
    Ok(())
}

#[test]
fn compact_direct_api_cancellation_retains_structure_phase() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let mut b = budget();
    b.cancel();
    assert!(matches!(
        encode_at_depths(
            GroupAtDepths {
                group: &g,
                depths: &[2, 40],
                native_registry: &r
            },
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut b
        ),
        Err(GroupEncodingError::Structure(
            nepl3_core::syntax::group::GroupError::Stopped(StopReason::Cancelled)
        ))
    ));
    Ok(())
}
