use super::*;

fn closure(schema: &SchemaRef, sources: Vec<SourceSnapshot>) -> ForeignClosure {
    ForeignClosure {
        syntax: ForeignSyntax {
            schema: schema.clone(),
            category: "Expr".into(),
            root: NodeRef(0),
            bundle: bundle(schema),
            environment: EnvironmentRef {
                id: 0,
                digest: Digest([0; 32]),
            },
        },
        owner_environment: environment(),
        provenance: OwnerProvenance::from_parts(vec![], sources, vec![]),
    }
}

fn snapshot(id: &str, revision: u64, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        revision,
        format!("memory:{id}"),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}

#[test]
fn positioned_capture_preserves_closure_and_rejects_invalid_positions() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut owner = bundle(&schema);
    let original = snapshot("original", 0, "値")?;
    let transformed = snapshot("transformed", 0, "value")?;
    owner.origins = vec![
        Origin::Composite(vec![OriginId(1)]),
        Origin::Direct(original.span(0, 3)?),
    ];
    owner.source_maps.push(Mapping {
        source: original.span(0, 3)?,
        target: transformed.span(0, 5)?,
        kind: MappingKind::Transformed,
    });
    owner.sources = vec![original, transformed];
    owner.environments.push(environment());
    owner.nodes[0].fields = vec![
        FieldValue::Child(NodeRef(1)),
        guest(&schema, bundle(&schema)),
    ];
    let mut second = bundle(&schema);
    second.nodes[0]
        .fields
        .push(FieldValue::Atom(NdfScalar::Text("second".into())));
    owner
        .nodes
        .push(node(&schema, vec![guest(&schema, second.clone())]));
    let before = owner.clone();
    let checked = owner.validate(&registry, &mut budget())?;
    let FieldValue::Foreign(selected) = &owner.nodes[0].fields[1] else {
        return Err(SyntaxError::Reference);
    };
    let expected = ForeignClosure::capture(
        selected,
        &checked,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let actual = ForeignClosure::capture_at(
        &checked,
        NodeRef(0),
        1,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(actual, expected);
    assert_eq!(actual.provenance.origins(), owner.origins);
    assert_eq!(actual.provenance.sources(), owner.sources);
    assert_eq!(actual.provenance.source_maps(), owner.source_maps);
    assert_eq!(actual.owner_environment, owner.environments[0]);
    let other = ForeignClosure::capture_at(
        &checked,
        NodeRef(1),
        0,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    assert_eq!(other.syntax.bundle, second);
    assert_ne!(actual.syntax.bundle, other.syntax.bundle);
    for (node, field) in [
        (NodeRef(u64::MAX), 0),
        (NodeRef(0), usize::MAX),
        (NodeRef(0), 0),
    ] {
        assert_eq!(
            ForeignClosure::capture_at(
                &checked,
                node,
                field,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default(),
            ),
            Err(SyntaxError::Reference),
        );
    }
    for (work, allocation, reason) in [
        (0, 1_000_000, StopReason::WorkLimit),
        (1_000_000, 0, StopReason::AllocationLimit),
    ] {
        let mut b = Budget::new(Limits {
            work,
            allocation_units: allocation,
            ..budget().limits()
        });
        assert_eq!(
            ForeignClosure::capture_at(
                &checked,
                NodeRef(0),
                1,
                &registry,
                &mut b,
                &mut SourceAdmission::default(),
            ),
            Err(SyntaxError::Stopped(reason)),
        );
    }
    // A structurally identical foreign value outside this owner is not a member.
    let detached = selected.clone();
    assert_eq!(
        ForeignClosure::capture(
            &detached,
            &checked,
            &registry,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(SyntaxError::Reference),
    );
    assert_eq!(owner, before);
    Ok(())
}

#[test]
fn positioned_capture_cost_is_independent_of_unrelated_owner_nodes() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut usages = Vec::new();
    for count in [32, 128, 512] {
        let mut owner = bundle(&schema);
        owner.environments.push(environment());
        // A flat, reachable owner isolates selection cost from provenance and
        // guest size. Put the foreign operand last so a full scan grows with it.
        for index in 1..count {
            owner.nodes[0]
                .fields
                .push(FieldValue::Child(NodeRef(index)));
            owner.nodes.push(node(&schema, vec![]));
        }
        owner.nodes[0].fields.push(guest(&schema, bundle(&schema)));
        let checked = owner.validate(&registry, &mut budget())?;
        let mut b = budget();
        let result = ForeignClosure::capture_at(
            &checked,
            NodeRef(0),
            (count - 1) as usize,
            &registry,
            &mut b,
            &mut SourceAdmission::default(),
        )?;
        assert_eq!(result.syntax.bundle, bundle(&schema));
        usages.push(b.usage());
    }
    assert!(
        usages.windows(2).all(|pair| pair[0] == pair[1]),
        "{usages:?}"
    );
    Ok(())
}

#[test]
fn closure_retains_guest_proof_only_after_owner_validation() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut input = closure(&schema, vec![]);
    let mut b = budget();
    let checked = input.validate(&registry, &mut b, &mut SourceAdmission::default())?;
    let usage = b.usage();
    assert!(core::ptr::eq(
        checked.syntax().bundle(),
        &input.syntax.bundle
    ));
    assert!(core::ptr::eq(checked.value(), &input));
    assert_eq!(checked.syntax().bundle().root, NodeRef(0));
    assert_eq!(b.usage(), usage);

    // A valid guest alone cannot establish the closure proof: the owner
    // environment must still match. No guest proof is returned on that failure.
    input.owner_environment.id += 1;
    assert_eq!(
        input
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .err(),
        Some(SyntaxError::Environment)
    );
    input.owner_environment.id -= 1;
    input.syntax.bundle.root = NodeRef(u64::MAX);
    assert!(
        input
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    Ok(())
}

#[test]
fn indexed_source_validation_preserves_duplicate_conflict_and_revision_rules()
-> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let first = snapshot("日本語", 1, "値")?;
    let second = snapshot("other", 0, "x")?;
    let variants = [
        (
            vec![first.clone(), second.clone(), first.clone()],
            Some(SyntaxError::DuplicateSource),
        ),
        (
            vec![first.clone(), second.clone(), snapshot("日本語", 1, "別")?],
            Some(SyntaxError::Source(SourceError::IdentityConflict)),
        ),
        (
            vec![first.clone(), second.clone(), snapshot("日本語", 2, "別")?],
            None,
        ),
        // Equal bytes still represent separate source declarations.
        (vec![first.clone(), snapshot("別名", 1, "値")?], None),
    ];
    for (sources, expected) in variants {
        let mut input = bundle(&schema);
        input.sources = sources.clone();
        let result = input.validate(&registry, &mut budget()).map(|_| ());
        assert_eq!(result.err(), expected);
        let foreign = closure(&schema, sources);
        let result = foreign
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .map(|_| ());
        assert_eq!(result.err(), expected);
    }
    Ok(())
}

#[test]
fn indexed_source_validation_preserves_origins_and_stop_causes() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let source = snapshot("source", 0, "値")?;
    let mut foreign = closure(&schema, vec![source.clone()]);
    foreign.provenance = OwnerProvenance::from_parts(
        vec![Origin::Direct(source.span(0, 3)?)],
        vec![source],
        vec![],
    );
    foreign.validate(&registry, &mut budget(), &mut SourceAdmission::default())?;
    for (work, allocation, reason) in [
        (0, 1_000_000, StopReason::WorkLimit),
        (1_000_000, 0, StopReason::AllocationLimit),
    ] {
        let limits = Limits {
            work,
            allocation_units: allocation,
            ..budget().limits()
        };
        let before = foreign.clone();
        assert_eq!(
            foreign
                .validate(
                    &registry,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default()
                )
                .err(),
            Some(SyntaxError::Stopped(reason))
        );
        assert_eq!(foreign, before);
    }
    foreign.provenance = OwnerProvenance::from_parts(
        foreign.provenance.origins().to_vec(),
        vec![],
        foreign.provenance.source_maps().to_vec(),
    );
    assert!(
        foreign
            .validate(&registry, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    Ok(())
}

#[test]
fn admitted_ordered_sources_avoid_repeated_pairwise_duplicate_scans() -> Result<(), SyntaxError> {
    let (registry, schema) = registry()?;
    let mut usages = Vec::new();
    for count in [128, 256, 512] {
        let sources = (0..count)
            .map(|index| snapshot(&format!("source-{index:04}"), 0, "x"))
            .collect::<Result<Vec<_>, _>>()?;
        let mut input = bundle(&schema);
        input.sources = sources.clone();
        let foreign = closure(&schema, sources);
        let mut admission = SourceAdmission::default();
        for source in foreign.provenance.sources() {
            admission.admit_existing(source, &mut budget())?;
        }
        // Measure duplicate validation and index construction after operation-wide
        // admission. IDs are ordered so insertion shifts do not dominate this case.
        // This bound makes no claim about initial admission or arbitrary ordering.
        let mut syntax_budget = budget();
        input.validate_with_sources(&registry, &mut syntax_budget, &mut admission)?;
        let mut closure_budget = budget();
        foreign.validate(&registry, &mut closure_budget, &mut admission)?;
        usages.push((syntax_budget.usage().work, closure_budget.usage().work));
    }
    for pair in usages.windows(2) {
        // Doubling an ordered input permits logarithmic lookup overhead but must
        // stay below the near-fourfold cost of pairwise duplicate comparison.
        assert!(pair[1].0 < 3 * pair[0].0, "syntax: {usages:?}");
        assert!(pair[1].1 < 3 * pair[0].1, "closure: {usages:?}");
    }
    Ok(())
}
