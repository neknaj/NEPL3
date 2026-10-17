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

#[test]
fn source_cache_hint_preserves_sparse_interleaved_and_independent_memberships()
-> Result<(), WireError> {
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
    let mut plain = CompactCapture::identity_only(&registry);
    let mut hinted = CompactCapture::identity_only(&registry);
    let mut plain_admission = SourceAdmission::default();
    let mut hinted_admission = SourceAdmission::default();
    let mut plain_budget = Budget::new(limits);
    let mut hinted_budget = Budget::new(limits);
    // Initially cache even entries, then interleave odd entries, then visit a
    // sparse subset, a prefix and the complete set. Reset hints at each boundary.
    let lists: Vec<Vec<usize>> = alloc::vec![
        (0..64).step_by(2).collect(),
        (0..64).collect(),
        alloc::vec![1, 31, 63],
        (0..8).collect(),
        (0..64).collect(),
    ];
    for list in lists {
        let mut next = 0;
        for index in list {
            let source = &sources[index];
            plain_admission.admit_existing(source, &mut plain_budget)?;
            hinted_admission.admit_existing(source, &mut hinted_budget)?;
            assert_eq!(
                plain.source_digest(
                    source,
                    &reference,
                    &mut plain_admission,
                    &mut plain_budget,
                    None
                )?,
                hinted.source_digest(
                    source,
                    &reference,
                    &mut hinted_admission,
                    &mut hinted_budget,
                    Some(&mut next)
                )?,
            );
        }
    }
    // Isolate warm-cache lookup Work. The source contents and cache are equal;
    // the difference is exclusively search strategy, not omitted admission.
    let mut plain_work = Budget::new(limits);
    let mut hinted_work = Budget::new(limits);
    let mut next = 0;
    for source in &sources {
        assert_eq!(
            plain.source_digest(
                source,
                &reference,
                &mut plain_admission,
                &mut plain_work,
                None
            )?,
            hinted.source_digest(
                source,
                &reference,
                &mut hinted_admission,
                &mut hinted_work,
                Some(&mut next)
            )?,
        );
    }
    assert!(hinted_work.usage().work < plain_work.usage().work);
    for index in [0, 31, 63] {
        let source = SourceSnapshot::new(
            nepl3_core::source::SourceId(alloc::format!("source-{index:03}")),
            1,
            alloc::format!("memory:{index}"),
            alloc::vec![b'x';32],
            &mut setup,
        )?;
        let mut next = index;
        assert_eq!(
            plain.source_digest(
                &source,
                &reference,
                &mut plain_admission,
                &mut plain_budget,
                None
            )?,
            hinted.source_digest(
                &source,
                &reference,
                &mut hinted_admission,
                &mut hinted_budget,
                Some(&mut next)
            )?,
        );
    }
    let independent = SourceSnapshot::new(
        nepl3_core::source::SourceId("source-000".into()),
        1,
        "memory:0".into(),
        alloc::vec![b'x';32],
        &mut setup,
    )?;
    let mut comparison = Budget::new(limits);
    sources[0]
        .identity()
        .compare_with_budget(independent.identity(), &mut comparison)?;
    let mut measured = Budget::new(limits);
    let mut next = 0;
    let expected = hinted.source_digest(
        &independent,
        &reference,
        &mut hinted_admission,
        &mut measured,
        Some(&mut next),
    )?;
    let mut exact = Budget::new(Limits {
        work: measured.usage().work,
        ..limits
    });
    let mut next = 0;
    assert_eq!(
        hinted.source_digest(
            &independent,
            &reference,
            &mut hinted_admission,
            &mut exact,
            Some(&mut next)
        )?,
        expected
    );
    assert_eq!(exact.usage(), measured.usage());
    let cache_before: Vec<_> = hinted
        .cache
        .iter()
        .map(|(source, digest)| (source.identity().clone(), *digest))
        .collect();
    for work in [0, comparison.usage().work, measured.usage().work - 1] {
        let mut short = Budget::new(Limits { work, ..limits });
        let mut next = 0;
        assert!(
            hinted
                .source_digest(
                    &independent,
                    &reference,
                    &mut hinted_admission,
                    &mut short,
                    Some(&mut next)
                )
                .is_err()
        );
        assert_eq!(short.poll(), Err(nepl3_core::budget::StopReason::WorkLimit));
        let retained = short.usage();
        assert!(
            hinted
                .source_digest(
                    &independent,
                    &reference,
                    &mut hinted_admission,
                    &mut short,
                    Some(&mut next)
                )
                .is_err()
        );
        assert_eq!(short.usage(), retained);
        let after: Vec<_> = hinted
            .cache
            .iter()
            .map(|(source, digest)| (source.identity().clone(), *digest))
            .collect();
        assert_eq!(after, cache_before);
        let mut fresh = Budget::new(limits);
        let mut next = 0;
        assert_eq!(
            hinted.source_digest(
                &independent,
                &reference,
                &mut hinted_admission,
                &mut fresh,
                Some(&mut next)
            )?,
            expected
        );
    }
    let mut cancelled = Budget::new(limits);
    cancelled.cancel();
    let mut next = 0;
    assert!(
        hinted
            .source_digest(
                &independent,
                &reference,
                &mut hinted_admission,
                &mut cancelled,
                Some(&mut next)
            )
            .is_err()
    );
    assert_eq!(
        cancelled.poll(),
        Err(nepl3_core::budget::StopReason::Cancelled)
    );
    // Equal portable identity with a different locator must fail even when the
    // next-index probe hits. This directly exercises equality, without admission
    // rejecting the fixture before the cache is reached.
    let conflict = SourceSnapshot::new(
        nepl3_core::source::SourceId("source-000".into()),
        1,
        "other:0".into(),
        alloc::vec![b'x';32],
        &mut setup,
    )?;
    let mut next = 0;
    assert_eq!(
        plain
            .source_digest(
                &conflict,
                &reference,
                &mut plain_admission,
                &mut plain_budget,
                None
            )
            .err(),
        Some(nepl3_core::source::SourceError::IdentityConflict.into()),
    );
    assert_eq!(
        hinted
            .source_digest(
                &conflict,
                &reference,
                &mut hinted_admission,
                &mut hinted_budget,
                Some(&mut next)
            )
            .err(),
        Some(nepl3_core::source::SourceError::IdentityConflict.into()),
    );
    Ok(())
}
