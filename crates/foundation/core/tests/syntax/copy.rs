use super::*;

fn with_source(schema: &SchemaRef, length: usize) -> Result<SyntaxBundle, SourceError> {
    let source = SourceSnapshot::new(
        SourceId("copy".into()),
        0,
        "memory:copy".into(),
        vec![b'x'; length],
        &mut budget(),
    )?;
    let mut child = bundle(schema);
    child.sources.push(source.clone());
    let mut root = bundle(schema);
    root.sources.push(source);
    root.nodes[0].fields.push(guest(schema, child));
    Ok(root)
}

#[test]
fn syntax_clone_accounts_shared_sources_separately_from_comparison() -> Result<(), SyntaxError> {
    let (_, schema) = registry()?;
    let mut work = Vec::new();
    for length in [8, 8192] {
        let original = with_source(&schema, length)?;
        let mut clone_budget = budget();
        let cloned = original.clone_with_budget(&mut clone_budget)?;
        assert_eq!(cloned, original);
        let mut comparison_budget = budget();
        original.charge_clone(&mut comparison_budget)?;
        let mut field_budget = budget();
        let field = original.nodes[0].fields[0].clone_with_budget(&mut field_budget)?;
        assert_eq!(field, original.nodes[0].fields[0]);
        let mut field_comparison_budget = budget();
        original.nodes[0].fields[0].charge_clone(&mut field_comparison_budget)?;
        // Copies preserve source admission and all non-copy counters. Cloning
        // shares immutable snapshots on pointer-atomic targets; the public
        // comparison bound must still cover independent source byte storage.
        assert_eq!(clone_budget.usage().source_bytes, 0);
        assert_eq!(clone_budget.usage().nodes, 0);
        assert_eq!(clone_budget.usage().diagnostics, 0);
        work.push((
            clone_budget.usage().work,
            comparison_budget.usage().work,
            field_budget.usage().work,
            field_comparison_budget.usage().work,
        ));
        for resource in [Resource::Work, Resource::AllocationUnits] {
            let mut limits = budget().limits();
            match resource {
                Resource::Work => limits.work = clone_budget.usage().work - 1,
                Resource::AllocationUnits => {
                    limits.allocation_units = clone_budget.usage().allocation_units - 1;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                original.clone_with_budget(&mut Budget::new(limits)),
                Err(if resource == Resource::Work {
                    StopReason::WorkLimit
                } else {
                    StopReason::AllocationLimit
                })
            );
            assert_eq!(original, cloned);
        }
    }
    assert_eq!(work[1].1 - work[0].1, 2 * (8192 - 8));
    assert_eq!(work[1].3 - work[0].3, 8192 - 8);
    #[cfg(target_has_atomic = "ptr")]
    {
        assert_eq!(work[1].0, work[0].0);
        assert_eq!(work[1].2, work[0].2);
    }
    #[cfg(not(target_has_atomic = "ptr"))]
    {
        assert_eq!(work[1].0 - work[0].0, 2 * (8192 - 8));
        assert_eq!(work[1].2 - work[0].2, 8192 - 8);
    }
    Ok(())
}
