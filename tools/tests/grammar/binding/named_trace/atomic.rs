use super::*;

fn assert_coupled(trace: &named::NamedTrace<'_, '_>) {
    let entities = trace
        .references()
        .reply()
        .facts()
        .map(|v| v.entities.as_slice())
        .unwrap_or(&[]);
    assert_eq!(entities.len(), trace.births().len());
    for entity in entities {
        assert_eq!(
            trace
                .births()
                .iter()
                .filter(|v| v.entity == entity.id)
                .count(),
            1
        );
    }
    for birth in trace.births() {
        assert_eq!(entities.iter().filter(|v| v.id == birth.entity).count(), 1);
    }
}

#[test]
fn entity_and_birth_publish_together_before_visibility_occurrence_and_memo() -> Result<(), String> {
    let compiled = execution()?;
    for input in ["lambda x x", "recursive cons define x x nil x"] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut full_budget = budget();
            let full = named::analyze(
                "atomic-birth",
                tree,
                profile,
                None,
                &mut full_budget,
                &mut SourceAdmission::default(),
            );
            assert!(full.references().complete().is_some());
            let usage = full_budget.usage();
            let run = |resource: Resource, cap: u64| {
                let mut limits = budget().limits();
                match resource {
                    Resource::Work => limits.work = cap,
                    Resource::Nodes => limits.nodes = cap,
                    Resource::AllocationUnits => limits.allocation_units = cap,
                    _ => {}
                }
                named::analyze(
                    "atomic-birth",
                    tree,
                    profile,
                    None,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                )
            };
            for (resource, total, width) in [
                (Resource::Work, usage.work, 64),
                (Resource::Nodes, usage.nodes, 8),
                (Resource::AllocationUnits, usage.allocation_units, 512),
            ] {
                let (mut lo, mut hi) = (0, total);
                while lo < hi {
                    let mid = lo + (hi - lo) / 2;
                    let trace = run(resource, mid);
                    assert_coupled(&trace);
                    if trace.births().is_empty() {
                        lo = mid + 1;
                    } else {
                        hi = mid;
                    }
                }
                let first = lo;
                assert!(first > 0);
                assert!(run(resource, first - 1).births().is_empty());
                for cap in first.saturating_sub(width)..=first.saturating_add(width).min(total) {
                    let trace = run(resource, cap);
                    assert_coupled(&trace);
                }
                if resource == Resource::AllocationUnits {
                    let trace = run(resource, first);
                    let BindingOutcome::Stopped {
                        reason: StopReason::AllocationLimit,
                        progress,
                    } = &trace.references().reply().outcome
                    else {
                        return Err("stop immediately after atomic birth".into());
                    };
                    let facts = progress.facts.as_ref().ok_or("partial facts")?;
                    let birth = trace.births().first().ok_or("birth")?;
                    assert_eq!(facts.entities.len(), 1);
                    assert!(facts.occurrences.is_empty());
                    assert!(
                        progress
                            .stages
                            .iter()
                            .all(|stage| !stage.introduced.contains(&birth.entity))
                    );
                    if input.starts_with("recursive") {
                        assert!(matches!(birth.phase, BirthPhase::Header { .. }));
                        // Export memo is installed only after its occurrence, which is absent.
                        assert_eq!(birth.kind, BirthKind::Export);
                    } else {
                        assert_eq!(birth.kind, BirthKind::Bind);
                    }
                    assert!(matches!(
                        trace.entity_birth(
                            birth.entity,
                            &mut Budget::new({
                                let mut l = budget().limits();
                                l.allocation_units = first;
                                l
                            })
                        ),
                        Err(BirthAccessError::Incomplete)
                    ));
                }
            }
            for cap in 0..=usage.depth {
                let mut limits = budget().limits();
                limits.depth = cap;
                let trace = named::analyze(
                    "atomic-birth",
                    tree,
                    profile,
                    None,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                );
                assert_coupled(&trace);
            }
            Ok(())
        })?;
    }
    Ok(())
}
