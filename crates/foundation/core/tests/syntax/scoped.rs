use super::*;

#[test]
fn registry_proof_admits_nested_and_disconnected_sources() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut inner = bundle(&schema);
    inner.sources.push(SourceSnapshot::new(
        SourceId("guest".into()),
        0,
        "memory:guest".into(),
        b"guest".to_vec(),
        &mut budget(),
    )?);
    let mut outer = bundle(&schema);
    outer.sources.push(SourceSnapshot::new(
        SourceId("root".into()),
        0,
        "memory:root".into(),
        b"root".to_vec(),
        &mut budget(),
    )?);
    outer.environments.push(environment());
    // This node is disconnected from the selected root. All declared foreign
    // sources still belong to the graph validation/admission contract.
    outer.nodes.push(node(&schema, vec![guest(&schema, inner)]));
    let mut initial = budget();
    let proof =
        outer.validate_in_registry(&registry, &mut initial, &mut SourceAdmission::default())?;
    let mut current = budget();
    let mut admission = SourceAdmission::default();
    proof.validate_for(&registry, &mut current, &mut admission)?;
    assert_eq!(current.usage().source_bytes, 9);
    assert_eq!(current.usage().nodes, 0);
    assert!(current.usage().work < initial.usage().work);
    proof.validate_for(&registry, &mut current, &mut admission)?;
    assert_eq!(current.usage().source_bytes, 9);
    let mut limited = Budget::new(Limits {
        source_bytes: 8,
        ..budget().limits()
    });
    assert_eq!(
        proof.validate_for(&registry, &mut limited, &mut SourceAdmission::default()),
        Err(SyntaxError::Stopped(StopReason::SourceLimit))
    );
    assert_eq!(limited.poll(), Err(StopReason::SourceLimit));
    Ok(())
}

#[test]
fn registry_proof_revalidates_a_different_registry() -> Result<(), SyntaxError> {
    let (original, schema) = registry()?;
    let input = bundle(&schema);
    let proof =
        input.validate_in_registry(&original, &mut budget(), &mut SourceAdmission::default())?;
    let (equivalent, _) = registry()?;
    let mut checked = budget();
    let returned = proof.checked_for(&equivalent, &mut checked, &mut SourceAdmission::default())?;
    let mut full = budget();
    let expected = input.validate(&equivalent, &mut full)?;
    assert!(core::ptr::eq(returned.bundle(), &input));
    assert_eq!(returned.validation_depth(), expected.validation_depth());
    assert_eq!(checked.usage().nodes, full.usage().nodes);
    assert!(checked.usage().nodes > 0);
    let mut missing = SchemaRegistry::default();
    missing.finalize(&mut budget())?;
    assert_eq!(
        proof.validate_for(&missing, &mut budget(), &mut SourceAdmission::default()),
        Err(SyntaxError::Schema(SchemaError::UnknownSchema))
    );
    assert_eq!(
        proof.validate_for(
            &SchemaRegistry::default(),
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(SyntaxError::Schema(SchemaError::Unfinalized))
    );
    Ok(())
}

#[test]
fn owned_registry_proof_applies_caller_depth_and_resource_limits() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = bundle(&schema);
    input.nodes[0].fields.push(FieldValue::Child(NodeRef(1)));
    input.nodes.push(node(&schema, vec![]));
    let owned = input
        .try_into_validated(&registry, &mut budget(), &mut SourceAdmission::default())
        .map_err(|failure| failure.error)?;
    let proof = owned.as_registry_validated();
    for limit in [8, 9] {
        let mut current = Budget::new(Limits {
            depth: limit,
            ..budget().limits()
        });
        let result = current.with_depth_at_least(7, |b| {
            proof.validate_for(&registry, b, &mut SourceAdmission::default())
        });
        if limit == 9 {
            result?;
        } else {
            assert_eq!(result, Err(SyntaxError::Stopped(StopReason::DepthLimit)));
        }
        assert_eq!(current.current_depth(), 0);
    }
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
                allocation_units: 0,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut current = Budget::new(limits);
        assert_eq!(
            proof.validate_for(&registry, &mut current, &mut SourceAdmission::default()),
            Err(SyntaxError::Stopped(reason))
        );
        assert_eq!(current.poll(), Err(reason));
    }
    Ok(())
}
