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
        owner_origins: vec![],
        owner_sources: sources,
        owner_source_maps: vec![],
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
    foreign
        .owner_origins
        .push(Origin::Direct(source.span(0, 3)?));
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
    foreign.owner_sources.clear();
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
        for source in &foreign.owner_sources {
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
