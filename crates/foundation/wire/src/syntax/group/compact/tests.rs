use super::*;
use nepl3_core::budget::Limits;
#[test]
fn component_membership_boundaries_are_not_flattened() -> Result<(), WireError> {
    let mut b = Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    });
    let a = NdfValue::Bytes(alloc::vec![1;32]);
    let z = NdfValue::Bytes(alloc::vec![2;32]);
    let left = NdfValue::List(alloc::vec![
        NdfValue::List(alloc::vec![a.clone()]),
        NdfValue::List(alloc::vec![z.clone()])
    ]);
    let right = NdfValue::List(alloc::vec![
        NdfValue::List(alloc::vec![]),
        NdfValue::List(alloc::vec![a, z])
    ]);
    // Deliberately the same flattened Bytes32 sequence, different local boundaries.
    // Unit is an isolated framing fixture, not a claim of native syntax validity.
    assert_ne!(
        component(SYNTAX, &NdfValue::Unit, left, &mut b)?,
        component(SYNTAX, &NdfValue::Unit, right, &mut b)?
    );
    Ok(())
}

#[test]
fn identity_capture_preserves_memberships_without_unused_archive_tables() -> Result<(), WireError> {
    let limits = Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    };
    let mut setup = Budget::new(limits);
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut setup)?;
    let reference = descriptor.reference(&mut setup)?;
    let mut registry = SchemaRegistry::default();
    registry.register(reference.clone(), descriptor, &mut setup)?;
    registry.finalize(&mut setup)?;
    let mut sources = Vec::new();
    for index in 0..64 {
        sources.push(SourceSnapshot::new(
            nepl3_core::source::SourceId(alloc::format!("source-{index:03}")),
            1,
            alloc::format!("memory:{index}"),
            alloc::vec![b'x';32],
            &mut setup,
        )?);
    }
    let mut transport = CompactCapture::new(&registry);
    let mut identity = CompactCapture::identity_only(&registry);
    let mut old_budget = Budget::new(limits);
    let mut new_budget = Budget::new(limits);
    let mut old_admission = SourceAdmission::default();
    let mut new_admission = SourceAdmission::default();
    for count in 0..=sources.len() {
        assert_eq!(
            transport.take(
                &sources[..count],
                &reference,
                &mut old_admission,
                &mut old_budget
            )?,
            identity.take(
                &sources[..count],
                &reference,
                &mut new_admission,
                &mut new_budget
            )?
        );
        transport.finish_component(SourceComponent::Entry, &mut old_budget)?;
        identity.finish_component(SourceComponent::Entry, &mut new_budget)?;
    }
    let mut independent = Vec::new();
    for index in 0..64 {
        independent.push(SourceSnapshot::new(
            nepl3_core::source::SourceId(alloc::format!("source-{index:03}")),
            1,
            alloc::format!("memory:{index}"),
            alloc::vec![b'x';32],
            &mut setup,
        )?);
    }
    assert_eq!(
        transport.take(
            &independent,
            &reference,
            &mut old_admission,
            &mut old_budget
        )?,
        identity.take(
            &independent,
            &reference,
            &mut new_admission,
            &mut new_budget
        )?
    );
    transport.finish_component(SourceComponent::Entry, &mut old_budget)?;
    identity.finish_component(SourceComponent::Entry, &mut new_budget)?;
    assert_eq!(transport.entries, identity.entries);
    assert!(identity.table.is_none());
    assert!(new_budget.usage().work < old_budget.usage().work);
    assert!(new_budget.usage().allocation_units < old_budget.usage().allocation_units);
    assert_eq!(
        new_budget.usage().output_bytes,
        old_budget.usage().output_bytes
    );
    let duplicate = alloc::vec![sources[0].clone(), sources[0].clone()];
    assert!(
        transport
            .take(&duplicate, &reference, &mut old_admission, &mut old_budget)
            .is_err()
    );
    assert!(
        identity
            .take(&duplicate, &reference, &mut new_admission, &mut new_budget)
            .is_err()
    );
    let conflict = SourceSnapshot::new(
        nepl3_core::source::SourceId("source-000".into()),
        1,
        "memory:0".into(),
        alloc::vec![b'y';32],
        &mut setup,
    )?;
    let old = transport
        .take(
            core::slice::from_ref(&conflict),
            &reference,
            &mut old_admission,
            &mut old_budget,
        )
        .err();
    let new = identity
        .take(
            core::slice::from_ref(&conflict),
            &reference,
            &mut new_admission,
            &mut new_budget,
        )
        .err();
    assert!(old.is_some());
    assert_eq!(old, new);
    Ok(())
}
