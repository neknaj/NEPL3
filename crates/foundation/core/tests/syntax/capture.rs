use super::*;

fn owner(schema: &SchemaRef) -> Result<SyntaxBundle, SyntaxError> {
    let source = SourceSnapshot::new(
        SourceId("owner".into()),
        3,
        "memory:owner".into(),
        "値".as_bytes().to_vec(),
        &mut budget(),
    )?;
    let mut value = bundle(schema);
    value.origins = vec![
        Origin::Composite(vec![OriginId(1)]),
        Origin::Direct(source.span(0, 3)?),
    ];
    value.sources = vec![source];
    value.environments.push(environment());
    value.nodes[0].fields = vec![guest(schema, bundle(schema)), guest(schema, bundle(schema))];
    Ok(value)
}

#[test]
fn captures_share_storage_but_revalidate_each_admission_and_registry() -> Result<(), SyntaxError> {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<OwnerProvenance>();
    let (registry, schema) = registry()?;
    let input = owner(&schema)?;
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    let mut admission = SourceAdmission::default();
    let mut first_budget = budget();
    let first = captures.capture_at(NodeRef(0), 0, &registry, &mut first_budget, &mut admission)?;
    let mut second_budget = budget();
    let second =
        captures.capture_at(NodeRef(0), 1, &registry, &mut second_budget, &mut admission)?;
    assert_eq!(first, second);
    #[cfg(target_has_atomic = "ptr")]
    {
        assert!(core::ptr::eq(
            first.provenance.origins(),
            second.provenance.origins()
        ));
        assert!(first_budget.usage().allocation_units > second_budget.usage().allocation_units);
    }
    // Storage reuse does not import another operation's source admission.
    let mut new_budget = budget();
    captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut new_budget,
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(new_budget.usage().source_bytes, 3);
    let mut conflicting = SourceAdmission::default();
    conflicting.create(
        SourceId("owner".into()),
        3,
        "memory:owner".into(),
        b"bad".to_vec(),
        &mut budget(),
    )?;
    assert!(matches!(
        captures.capture_at(NodeRef(0), 0, &registry, &mut budget(), &mut conflicting),
        Err(SyntaxError::Source(SourceError::IdentityConflict)),
    ));
    let mut other_registry = SchemaRegistry::default();
    other_registry.finalize(&mut budget())?;
    assert!(
        captures
            .capture_at(
                NodeRef(0),
                0,
                &other_registry,
                &mut budget(),
                &mut admission
            )
            .is_err()
    );
    // The captured value owns its tables after both context and owner expire.
    drop(captures);
    drop(input);
    first.validate(&registry, &mut budget(), &mut SourceAdmission::default())?;
    assert_eq!(first.provenance.origins().len(), 2);
    Ok(())
}

#[test]
fn capture_storage_has_budgeted_clones_and_survives_preparation_stop() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let input = owner(&schema)?;
    let checked = input.validate(&registry, &mut budget())?;
    let mut captures = ForeignCapture::new(&checked);
    for (work, allocation, reason) in [
        (0, 1_000_000, StopReason::WorkLimit),
        (1_000_000, 0, StopReason::AllocationLimit),
    ] {
        let mut limited = Budget::new(Limits {
            work,
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            captures.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut limited,
                &mut SourceAdmission::default()
            ),
            Err(SyntaxError::Stopped(reason))
        );
    }
    let captured = captures.capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let expected = ForeignClosure::capture_at(
        &checked,
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(captured, expected);
    let mut measured = budget();
    ForeignCapture::new(&checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut measured,
        &mut SourceAdmission::default(),
    )?;
    for allocation in [1, 64, 128, measured.usage().allocation_units - 1] {
        let mut partial = ForeignCapture::new(&checked);
        let mut limited = Budget::new(Limits {
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            partial.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut limited,
                &mut SourceAdmission::default()
            ),
            Err(SyntaxError::Stopped(StopReason::AllocationLimit))
        );
        // An incomplete preparation exposes no closure. A fresh operation can
        // retry; retained storage is revalidated under that operation's ledger.
        assert_eq!(
            partial.capture_at(
                NodeRef(0),
                0,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default()
            )?,
            expected
        );
    }
    let mut limited = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert_eq!(
        captures.capture_at(
            NodeRef(0),
            1,
            &registry,
            &mut limited,
            &mut SourceAdmission::default()
        ),
        Err(SyntaxError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(captured, expected);
    for (work, allocation, reason) in [
        (0, 1_000_000, StopReason::WorkLimit),
        (1_000_000, 0, StopReason::AllocationLimit),
    ] {
        let mut limited = Budget::new(Limits {
            work,
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            captured.provenance.clone_with_budget(&mut limited),
            Err(reason)
        );
    }
    // Replacing one closure's owner tables cannot change another shared value.
    let mut changed = captured.clone_with_budget(&mut budget())?;
    changed.provenance = OwnerProvenance::from_parts(vec![], vec![], vec![]);
    assert_eq!(captured.provenance.origins(), input.origins);
    assert!(changed.provenance.origins().is_empty());
    let other_input = owner(&schema)?;
    let other_checked = other_input.validate(&registry, &mut budget())?;
    let other = ForeignCapture::new(&other_checked).capture_at(
        NodeRef(0),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(other, captured);
    #[cfg(target_has_atomic = "ptr")]
    assert!(!core::ptr::eq(
        other.provenance.origins(),
        captured.provenance.origins()
    ));
    Ok(())
}
