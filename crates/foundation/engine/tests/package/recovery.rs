use super::*;
use nepl3_core::budget::{StopReason, Usage};
use nepl3_engine::recovery::*;

fn plan(package: &LanguagePackage) -> RecoveryPlan {
    let mut plan = package.recovery.clone();
    plan.rules = vec![RecoveryRule {
        category: "Expr".into(),
        unexpected: UnexpectedPolicy::PreserveRemainder,
        synchronization: vec![SyncToken {
            ancestor_category: "Expr".into(),
            kind: package.leaves[0].token_kind.clone(),
            spelling: Some("end".into()),
        }],
    }];
    plan
}
fn prefix(package: &LanguagePackage) -> u64 {
    // Entry plus two category-table checks, no preceding rule duplicates.
    1 + 2 * package.categories.len() as u64 * ("Expr".len() as u64 + 1)
}
fn comparison(package: &LanguagePackage, plan: &RecoveryPlan) -> u64 {
    package.schema.package.len() as u64
        + plan.rules[0].synchronization[0].kind.schema.package.len() as u64
        + 41
}
#[test]
fn recovery_sync_kind_admits_identity_comparison_and_registry_lookup() -> TestResult {
    let (package, registry) = fixture()?;
    let plan = plan(&package);
    let mut lookup = budget();
    let kind = &plan.rules[0].synchronization[0].kind;
    registry
        .kind_name_with_budget(&kind.schema, kind.local_kind, &mut lookup)
        .map_err(|e| format!("{e:?}"))?;
    let initial = prefix(&package);
    let entry = initial + comparison(&package, &plan);
    let expected = entry + lookup.usage().work;
    let mut full = budget();
    plan.validate(&package, &registry, &mut full)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        full.usage(),
        Usage {
            work: expected,
            ..Usage::default()
        }
    );
    for (cap, accepted) in [(initial, initial), (entry, entry), (entry + 1, entry + 1)] {
        let mut limits = budget().limits();
        limits.work = cap;
        let mut b = Budget::new(limits);
        assert_eq!(
            plan.validate(&package, &registry, &mut b),
            Err(PackageError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        assert_eq!(
            b.usage(),
            Usage {
                work: accepted,
                ..Usage::default()
            }
        );
    }
    let mut limits = budget().limits();
    limits.work = expected;
    assert!(
        plan.validate(&package, &registry, &mut Budget::new(limits))
            .is_ok()
    );
    Ok(())
}
#[test]
fn recovery_long_schema_mismatch_stops_before_comparison_and_lookup() -> TestResult {
    let (mut package, registry) = fixture()?;
    package.schema.package = "p".repeat(2048);
    let mut plan = plan(&package);
    plan.rules[0].synchronization[0].kind.schema = package.schema.clone();
    let initial = prefix(&package);
    let entry = initial + comparison(&package, &plan);
    // Matching but unregistered identity proceeds to lookup only after comparison.
    let mut limits = budget().limits();
    limits.work = entry;
    let mut b = Budget::new(limits);
    assert_eq!(
        plan.validate(&package, &registry, &mut b),
        Err(PackageError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(b.usage().work, entry);
    assert!(matches!(
        plan.validate(&package, &registry, &mut budget()),
        Err(PackageError::Schema(SchemaError::UnknownSchema))
    ));
    let name = &mut plan.rules[0].synchronization[0].kind.schema.package;
    let _ = name.pop();
    name.push('q');
    let original = plan.clone();
    let mut limits = budget().limits();
    limits.work = entry - 1;
    let mut b = Budget::new(limits);
    assert_eq!(
        plan.validate(&package, &registry, &mut b),
        Err(PackageError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(b.usage().work, initial);
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    let mut exact = budget().limits();
    exact.work = entry;
    let mut b = Budget::new(exact);
    assert_eq!(
        plan.validate(&package, &registry, &mut b),
        Err(PackageError::KindShape)
    );
    assert_eq!(b.usage().work, entry);
    assert_eq!(plan, original);
    Ok(())
}
