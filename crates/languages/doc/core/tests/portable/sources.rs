use super::*;

#[test]
fn source_index_is_reused_within_document_encoding() -> Result<(), String> {
    let r = registry()?;
    for count in [1, 65] {
        let mut document = literal(&r)?;
        for index in 1..count {
            document.sources.push(
                SourceSnapshot::new(
                    SourceId(format!("extra/{index:04}")),
                    1,
                    "memory:extra".into(),
                    vec![b'x'; 1024],
                    &mut b(),
                )
                .map_err(err)?,
            );
        }
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let mut measured = b();
        let value = portable::to_value(&document, &r, &mut codec, &mut measured).map_err(err)?;
        // Measured with this exact fixture on the unchanged two-index
        // encoder at 4c8d0434. The old path is a real negative control.
        if cfg!(target_pointer_width = "64") {
            let (work, allocation) = if count == 1 {
                (31_092, 60_737)
            } else {
                (100_308, 210_497)
            };
            let mut rebuild = b();
            let mut store = SourceStore::default();
            for source in &document.sources {
                store
                    .insert_with_budget(
                        source.clone_with_budget(&mut rebuild).map_err(err)?,
                        &mut rebuild,
                    )
                    .map_err(err)?;
            }
            assert_eq!(
                measured.usage().work,
                work - rebuild.usage().work,
                "source index must not be rebuilt"
            );
            assert_eq!(
                measured.usage().allocation_units,
                allocation - rebuild.usage().allocation_units
            );
        }
        let mut incoming = SourceAdmission::default();
        let mut decoder = FoundationCodec::new(&r, &empty, &mut incoming).map_err(err)?;
        assert_eq!(
            portable::from_value(&value, &r, &mut decoder, &mut b()).map_err(err)?,
            document
        );
    }
    Ok(())
}

fn encode(
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
    portable::to_value(document, r, &mut codec, budget).map_err(err)
}

#[test]
fn reused_source_index_preserves_budget_edges_and_prior_usage() -> Result<(), String> {
    use nepl3_core::budget::Resource;
    let r = registry()?;
    let document = literal(&r)?;
    let mut measured = b();
    let expected = encode(&document, &r, &mut measured).map_err(err)?;
    let mut prior = b();
    prior.charge(Resource::Work, 17).map_err(err)?;
    prior.charge(Resource::AllocationUnits, 23).map_err(err)?;
    assert_eq!(encode(&document, &r, &mut prior).map_err(err)?, expected);
    assert_eq!(prior.usage().work, measured.usage().work + 17);
    assert_eq!(
        prior.usage().allocation_units,
        measured.usage().allocation_units + 23
    );
    for allocation in [false, true] {
        for short in [false, true] {
            let mut limits = b().limits();
            if allocation {
                limits.allocation_units = measured.usage().allocation_units - u64::from(short);
            } else {
                limits.work = measured.usage().work - u64::from(short);
            }
            let mut limited = Budget::new(limits);
            let result = encode(&document, &r, &mut limited);
            if short {
                assert!(result.is_err());
                assert_eq!(
                    limited.poll(),
                    Err(if allocation {
                        StopReason::AllocationLimit
                    } else {
                        StopReason::WorkLimit
                    })
                );
                assert!(encode(&document, &r, &mut limited).is_err());
            } else {
                assert_eq!(result.map_err(err)?, expected);
            }
        }
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert!(encode(&document, &r, &mut cancelled).is_err());
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn source_reuse_does_not_bypass_fresh_document_validation() -> Result<(), String> {
    let r = registry()?;
    let mut document = literal(&r)?;
    let _ = encode(&document, &r, &mut b()).map_err(err)?;
    document.sources.push(document.sources[0].clone());
    assert!(encode(&document, &r, &mut b()).is_err());
    document.sources.clear();
    assert!(encode(&document, &r, &mut b()).is_err());
    let document = literal(&r)?;
    assert!(encode(&document, &SchemaRegistry::default(), &mut b()).is_err());
    Ok(())
}
