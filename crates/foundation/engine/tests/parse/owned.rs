use super::support::budget;
use nepl3_core::{budget::Budget, source::SourceAdmission};
use nepl3_engine::{profile::ResolvedParseProfile, recovery::ParseTree};

pub fn check(tree: &ParseTree, profile: &ResolvedParseProfile<'_>) -> Result<(), String> {
    let mut borrowed_budget = budget();
    let borrowed = tree
        .validate(
            profile,
            &mut borrowed_budget,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("borrowed: {e:?}"))?;
    let raw = tree.clone();
    let nodes = raw.bundle.nodes.as_ptr();
    let contexts = raw.contexts.as_ptr();
    let mut owned_budget = budget();
    let owned = raw
        .try_into_validated(profile, &mut owned_budget, &mut SourceAdmission::default())
        .map_err(|e| format!("owned: {e:?}"))?;
    assert_eq!(owned_budget.usage(), borrowed_budget.usage());
    assert!(core::ptr::eq(owned.profile(), profile));
    assert_eq!(owned.syntax().bundle(), &tree.bundle);
    assert_eq!(owned.contexts(), &tree.contexts);
    assert_eq!(owned.recovery(), &tree.recovery);
    assert_eq!(owned.is_recovered(), borrowed.is_recovered());
    let raw = owned.into_inner();
    assert_eq!(raw, *tree);
    assert_eq!(raw.bundle.nodes.as_ptr(), nodes);
    assert_eq!(raw.contexts.as_ptr(), contexts);

    // Independent mutation of structural and selection data must still reject.
    let mut invalid_root = tree.clone();
    invalid_root.bundle.root.0 = u64::MAX;
    let mut invalid_context = tree.clone();
    invalid_context.contexts.clear();
    let mut duplicate = tree.clone();
    let Some(context) = duplicate.contexts.first().cloned() else {
        return Err("fixture must contain a selection context".into());
    };
    duplicate.contexts.push(context);
    let mut invalid_profile = tree.clone();
    invalid_profile.profile_digest.0[0] ^= 1;
    for invalid in [invalid_root, invalid_context, duplicate, invalid_profile] {
        compare_rejection(invalid, profile, budget().limits())?;
    }

    // Exercise both structural preparation and late selection validation stops.
    let mut structural = budget();
    tree.bundle
        .validate_with_sources(
            profile.registry(),
            &mut structural,
            &mut SourceAdmission::default(),
        )
        .map_err(|e| format!("structural: {e:?}"))?;
    for work in [
        0,
        1,
        structural.usage().work,
        borrowed_budget.usage().work - 1,
    ] {
        compare_rejection(
            tree.clone(),
            profile,
            nepl3_core::budget::Limits {
                work,
                ..budget().limits()
            },
        )?;
    }
    for allocation_units in [
        0,
        structural.usage().allocation_units,
        borrowed_budget.usage().allocation_units - 1,
    ] {
        compare_rejection(
            tree.clone(),
            profile,
            nepl3_core::budget::Limits {
                allocation_units,
                ..budget().limits()
            },
        )?;
    }
    let raw = tree.clone();
    let nodes = raw.bundle.nodes.as_ptr();
    let mut cancelled = budget();
    cancelled.cancel();
    let Err(failure) =
        raw.try_into_validated(profile, &mut cancelled, &mut SourceAdmission::default())
    else {
        return Err("cancelled validation succeeded".into());
    };
    assert_eq!(
        failure.error,
        nepl3_engine::tree::TreeError::Stopped(nepl3_core::budget::StopReason::Cancelled)
    );
    assert_eq!(failure.tree, *tree);
    assert_eq!(failure.tree.bundle.nodes.as_ptr(), nodes);
    assert_eq!(cancelled.usage(), budget().usage());
    failure
        .tree
        .try_into_validated(profile, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("retry after cancellation: {e:?}"))?;
    Ok(())
}

fn compare_rejection(
    tree: ParseTree,
    profile: &ResolvedParseProfile<'_>,
    limits: nepl3_core::budget::Limits,
) -> Result<(), String> {
    let mut borrowed_budget = Budget::new(limits);
    let Err(expected) = tree.validate(
        profile,
        &mut borrowed_budget,
        &mut SourceAdmission::default(),
    ) else {
        return Err("negative fixture unexpectedly validated".into());
    };
    let original = tree.clone();
    let nodes = tree.bundle.nodes.as_ptr();
    let contexts = tree.contexts.as_ptr();
    let recovery = tree.recovery.as_ptr();
    let mut owned_budget = Budget::new(limits);
    let Err(failure) =
        tree.try_into_validated(profile, &mut owned_budget, &mut SourceAdmission::default())
    else {
        return Err("owned negative fixture unexpectedly validated".into());
    };
    assert_eq!(failure.error, expected);
    assert_eq!(owned_budget.usage(), borrowed_budget.usage());
    assert_eq!(failure.tree, original);
    assert_eq!(failure.tree.bundle.nodes.as_ptr(), nodes);
    assert_eq!(failure.tree.contexts.as_ptr(), contexts);
    assert_eq!(failure.tree.recovery.as_ptr(), recovery);
    Ok(())
}
