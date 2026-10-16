use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, archive, foundation},
    source::{SourceAdmission, SourceId, SourceRef, SourceSnapshot},
    value::{NdfValue, SchemaRef},
};
use nepl3_wire::{WireError, source};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1024,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn setup() -> Result<(SchemaRef, SchemaRef, SchemaRegistry), String> {
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    let f = foundation::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let a = archive::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let fs = f.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    let ars = a.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    registry
        .register(fs.clone(), f, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(ars.clone(), a, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    registry.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    Ok((ars, fs, registry))
}
fn source(id: &str) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        0,
        format!("memory:{id}"),
        "日本語原文".repeat(1024).into_bytes(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}
fn encoded() -> Result<(Vec<u8>, SchemaRef, SchemaRef, SchemaRegistry), String> {
    let (a, f, r) = setup()?;
    let s = [source("a")?];
    let m = vec![vec![s[0].reference()], vec![], vec![s[0].reference()]];
    let bytes = source::archive::encode(
        &s,
        &m,
        &a,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok((bytes, a, f, r))
}
#[test]
fn repeated_membership_decodes_shared_checked_storage_and_empty_local_set() -> TestResult {
    let (bytes, a, f, r) = encoded()?;
    let mut b = budget();
    let sets = source::archive::decode(&bytes, &a, &f, &r, &mut SourceAdmission::default(), &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(sets.len(), 3);
    assert!(sets[1].is_empty());
    assert_eq!(sets[0], sets[2]);
    assert_eq!(b.usage().source_bytes, sets[0][0].text().len() as u64);
    #[cfg(target_has_atomic = "ptr")]
    assert!(core::ptr::eq(
        sets[0][0].text().as_ptr(),
        sets[2][0].text().as_ptr()
    ));
    let old = source::encode_sources(
        &sets[0],
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(bytes.len() < 2 * old.len());
    assert!(
        source::decode_sources(
            &bytes,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    assert!(
        source::archive::decode(
            &old,
            &a,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
fn mutate(
    bytes: &[u8],
    change: impl FnOnce(&mut NdfValue) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let mut v = nepl3_wire::decode(bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    change(&mut v)?;
    nepl3_wire::encode(&v, &mut budget()).map_err(|e| format!("{e:?}"))
}
#[test]
fn malformed_memberships_contents_and_versions_reject() -> TestResult {
    let (bytes, a, f, r) = encoded()?;
    for case in 0..6 {
        let bad = mutate(&bytes, |v| {
            let NdfValue::Record(root) = v else {
                return Err("fixture root".into());
            };
            match case {
                0 => {
                    root.schema.revision = 2;
                }
                1 => {
                    let NdfValue::List(sets) = &mut root.fields[1] else {
                        return Err("sets".into());
                    };
                    for set in sets {
                        *set = NdfValue::List(vec![]);
                    }
                }
                2 => {
                    let NdfValue::List(sets) = &mut root.fields[1] else {
                        return Err("sets".into());
                    };
                    let NdfValue::List(m) = &mut sets[0] else {
                        return Err("members".into());
                    };
                    m.push(m[0].clone());
                }
                3 => {
                    let NdfValue::List(sets) = &mut root.fields[1] else {
                        return Err("sets".into());
                    };
                    let NdfValue::List(m) = &mut sets[0] else {
                        return Err("members".into());
                    };
                    let NdfValue::Record(reference) = &mut m[0] else {
                        return Err("reference".into());
                    };
                    reference.fields[2] = NdfValue::Bytes(vec![0; 32]);
                }
                4 | 5 => {
                    let NdfValue::List(sources) = &mut root.fields[0] else {
                        return Err("sources".into());
                    };
                    let NdfValue::Record(source) = &mut sources[0] else {
                        return Err("source".into());
                    };
                    source.fields[if case == 4 { 2 } else { 1 }] =
                        NdfValue::Text(if case == 4 { "forged" } else { "bad uri" }.into());
                }
                _ => return Err("unknown mutation".into()),
            }
            Ok(())
        })?;
        assert!(
            source::archive::decode(
                &bad,
                &a,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err(),
            "case {case}"
        );
    }
    Ok(())
}
#[test]
fn native_encoder_rejects_missing_duplicate_unused_and_wrong_schema() -> TestResult {
    let (a, f, r) = setup()?;
    let sources = [source("a")?, source("b")?];
    let a_ref = sources[0].reference();
    let b_ref = sources[1].reference();
    let variants: Vec<Vec<Vec<SourceRef>>> = vec![
        vec![vec![a_ref.clone()]],
        vec![vec![b_ref.clone(), a_ref.clone()]],
        vec![vec![a_ref.clone(), a_ref.clone(), b_ref.clone()]],
        vec![vec![a_ref.clone(), source("z")?.reference()]],
    ];
    for m in variants {
        assert!(
            source::archive::encode(
                &sources,
                &m,
                &a,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    assert!(
        source::archive::encode(
            &sources,
            &[vec![a_ref, b_ref]],
            &f,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn exact_decode_limits_and_sticky_failure() -> TestResult {
    let (bytes, a, f, r) = encoded()?;
    let mut b = budget();
    source::archive::decode(&bytes, &a, &f, &r, &mut SourceAdmission::default(), &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let u = b.usage();
    let exact = Limits {
        work: u.work,
        allocation_units: u.allocation_units,
        source_bytes: u.source_bytes,
        depth: u.depth,
        ..budget().limits()
    };
    source::archive::decode(
        &bytes,
        &a,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut Budget::new(exact),
    )
    .map_err(|e| format!("{e:?}"))?;
    for (limit, why) in [
        (
            Limits {
                work: u.work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: u.allocation_units - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                source_bytes: u.source_bytes - 1,
                ..exact
            },
            StopReason::SourceLimit,
        ),
        (
            Limits {
                depth: u.depth - 1,
                ..exact
            },
            StopReason::DepthLimit,
        ),
    ] {
        let mut limited = Budget::new(limit);
        assert!(
            source::archive::decode(
                &bytes,
                &a,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut limited
            )
            .is_err()
        );
        assert_eq!(limited.poll(), Err(why));
        assert_eq!(
            source::archive::decode(
                &bytes,
                &a,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut limited
            )
            .err(),
            Some(WireError::Stopped(why))
        );
    }
    Ok(())
}

#[test]
fn distinct_ids_survive_and_decoder_rejects_noncanonical_table() -> TestResult {
    let (a, f, r) = setup()?;
    let sources = [source("a")?, source("b")?];
    let members = vec![vec![sources[0].reference()], vec![sources[1].reference()]];
    let bytes = source::archive::encode(
        &sources,
        &members,
        &a,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let sets = source::archive::decode(
        &bytes,
        &a,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(sets[0][0].text(), sets[1][0].text());
    assert_ne!(sets[0][0].identity(), sets[1][0].identity());
    for duplicate in [false, true] {
        let bad = mutate(&bytes, |value| {
            let NdfValue::Record(root) = value else {
                return Err("root".into());
            };
            let NdfValue::List(table) = &mut root.fields[0] else {
                return Err("table".into());
            };
            if duplicate {
                table[1] = table[0].clone();
            } else {
                table.swap(0, 1);
            }
            Ok(())
        })?;
        assert!(
            source::archive::decode(
                &bad,
                &a,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    Ok(())
}
#[test]
fn repeated_wire_membership_has_one_text_payload_and_cancellation_is_sticky() -> TestResult {
    let (bytes, a, f, r) = encoded()?;
    let text = source("a")?;
    assert_eq!(
        bytes
            .windows(text.text().len())
            .filter(|w| *w == text.text().as_bytes())
            .count(),
        1
    );
    let mut stopped = budget();
    stopped.stop(StopReason::Cancelled);
    assert_eq!(
        source::archive::decode(
            &bytes,
            &a,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut stopped
        )
        .err(),
        Some(WireError::Stopped(StopReason::Cancelled))
    );
    assert!(
        source::archive::decode(
            &bytes,
            &a,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_ok()
    );
    Ok(())
}
