use super::*;

#[test]
fn owned_proof_moves_storage_and_raw_edits_require_validation() -> Result<(), String> {
    let (registry, schema) = registry().map_err(|e| format!("{e:?}"))?;
    let input = bundle(&schema);
    let storage = input.nodes.as_ptr();
    let expected = input.clone();
    let mut borrowed_budget = budget();
    expected
        .validate_with_sources(
            &registry,
            &mut borrowed_budget,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let mut owned_budget = budget();
    let proof = input
        .try_into_validated(
            &registry,
            &mut owned_budget,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("{:?}", e.error))?;
    assert_eq!(owned_budget.usage(), borrowed_budget.usage());
    assert!(core::ptr::eq(proof.registry(), &registry));
    assert_eq!(proof.bundle(), &expected);
    assert_eq!(proof.bundle().nodes.as_ptr(), storage);
    assert!(core::ptr::eq(proof.as_validated().bundle(), proof.bundle()));
    let mut raw = proof.into_inner();
    assert_eq!(raw.nodes.as_ptr(), storage);
    raw.root = NodeRef(u64::MAX);
    let expected = raw.clone();
    let Err(failure) =
        raw.try_into_validated(&registry, &mut budget(), &mut SourceAdmission::default())
    else {
        return Err("an edited root must be checked again".into());
    };
    assert_eq!(failure.error, SyntaxError::Reference);
    assert_eq!(failure.bundle, expected);
    assert_eq!(failure.bundle.nodes.as_ptr(), storage);
    Ok(())
}

#[test]
fn owned_validation_retains_input_at_every_work_and_allocation_boundary() -> Result<(), String> {
    let (registry, schema) = registry().map_err(|e| format!("{e:?}"))?;
    let mut input = bundle(&schema);
    let source = SourceSnapshot::new(
        SourceId("owned-source".into()),
        1,
        "memory:owned".into(),
        b"x".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    input.origins[0] = Origin::Direct(source.span(0, 1).map_err(|e| format!("{e:?}"))?);
    input.sources.push(source);
    let mut complete = budget();
    input
        .validate_with_sources(&registry, &mut complete, &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    for resource in [Resource::Work, Resource::AllocationUnits] {
        let ceiling = match resource {
            Resource::Work => complete.usage().work,
            Resource::AllocationUnits => complete.usage().allocation_units,
            _ => unreachable!(),
        };
        for limit in 0..ceiling {
            let mut limits = budget().limits();
            let reason = match resource {
                Resource::Work => {
                    limits.work = limit;
                    StopReason::WorkLimit
                }
                Resource::AllocationUnits => {
                    limits.allocation_units = limit;
                    StopReason::AllocationLimit
                }
                _ => unreachable!(),
            };
            let raw = input.clone();
            let nodes = raw.nodes.as_ptr();
            let sources = raw.sources.as_ptr();
            let mut stopped = Budget::new(limits);
            let mut admission = SourceAdmission::default();
            let Err(failure) = raw.try_into_validated(&registry, &mut stopped, &mut admission)
            else {
                return Err("every insufficient ceiling must stop".into());
            };
            assert_eq!(failure.error.stop_reason(), Some(reason));
            assert_eq!(stopped.poll(), Err(reason));
            assert_eq!(failure.bundle, input);
            assert_eq!(failure.bundle.nodes.as_ptr(), nodes);
            assert_eq!(failure.bundle.sources.as_ptr(), sources);
            // Retrying uses a fresh operation and never repairs or discards input.
            failure
                .bundle
                .try_into_validated(&registry, &mut budget(), &mut SourceAdmission::default())
                .map_err(|e| format!("{:?}", e.error))?;
        }
    }
    Ok(())
}
