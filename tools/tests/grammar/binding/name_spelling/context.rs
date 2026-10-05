use super::*;
use nepl3_core::budget::{Resource, StopReason};
use nepl3_engine::{
    analysis::insertion::{
        context::{self, ContextDifference as Difference, ContextError, ParseContext},
        quality::StrictNameInsertion,
    },
    profile::ResolvedParseProfile,
};
pub(super) fn verify(
    strict: &StrictNameInsertion<'_, '_, '_, '_>,
    alternate: Option<&ResolvedParseProfile<'_>>,
    b: &Budget,
) -> Result<(), String> {
    let checked = strict.declaration().reference().insertion().checked();
    let seed = checked.original().seed();
    let before = checked.original().execution().report().clone();
    let supplied = ParseContext {
        profile: seed.profile(),
        environments: seed.environments(),
        request: seed.request(),
    };
    let mut measured = Budget::new(b.limits());
    assert_eq!(
        context::compare_original(strict, &supplied, &mut measured),
        Ok(None)
    );
    let usage = measured.usage();
    for (resource, cap, cost, reason) in [
        (
            Resource::Work,
            b.limits().work,
            usage.work,
            StopReason::WorkLimit,
        ),
        (
            Resource::AllocationUnits,
            b.limits().allocation_units,
            usage.allocation_units,
            StopReason::AllocationLimit,
        ),
        (
            Resource::Nodes,
            b.limits().nodes,
            usage.nodes,
            StopReason::NodeLimit,
        ),
    ] {
        if cost == 0 {
            continue;
        }
        let mut exact = Budget::new(b.limits());
        exact.charge(resource, cap - cost).map_err(err)?;
        assert_eq!(
            context::compare_original(strict, &supplied, &mut exact),
            Ok(None)
        );
        let mut short = Budget::new(b.limits());
        short.charge(resource, cap - (cost - 1)).map_err(err)?;
        assert_eq!(
            context::compare_original(strict, &supplied, &mut short),
            Err(ContextError::Stopped(reason))
        );
    }
    if usage.depth > 0 {
        let base = b.limits().depth - usage.depth;
        let mut exact = Budget::new(b.limits());
        assert_eq!(
            exact.with_depth_at_least(base, |b| context::compare_original(strict, &supplied, b)),
            Ok(None)
        );
        assert_eq!(exact.current_depth(), 0);
        let mut short = Budget::new(b.limits());
        assert_eq!(
            short.with_depth_at_least(base + 1, |b| context::compare_original(
                strict, &supplied, b
            )),
            Err(ContextError::Stopped(StopReason::DepthLimit))
        );
        assert_eq!(short.current_depth(), 0);
    }
    let mut cancelled = Budget::new(b.limits());
    cancelled.stop(StopReason::Cancelled);
    assert_eq!(
        context::compare_original(strict, &supplied, &mut cancelled),
        Err(ContextError::Stopped(StopReason::Cancelled))
    );
    let mut limits = b.limits();
    limits.work -= 1;
    assert_eq!(
        context::compare_original(strict, &supplied, &mut Budget::new(limits)),
        Err(ContextError::LimitsMismatch)
    );
    let check = |current: ParseContext<'_>, expected| {
        assert_eq!(
            context::compare_original(strict, &current, &mut Budget::new(b.limits())),
            Ok(Some(expected))
        );
    };
    let make = || ParseContext {
        profile: seed.profile(),
        environments: seed.environments(),
        request: seed.request(),
    };
    if let Some(profile) = alternate {
        let mut current = make();
        current.profile = profile;
        check(current, Difference::ProfileInstance);
    }
    let inputs: Vec<_> = seed
        .environments()
        .languages()
        .iter()
        .map(|language| EnvironmentInput {
            alias: &language.alias,
            context: &language.context,
        })
        .collect();
    let mut env_budget = Budget::new(b.limits());
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(seed.profile().registry(), seed.sources(), &mut admission)
        .map_err(err)?;
    let alternate_environments = ParseEnvironmentSet::prepare(
        seed.profile(),
        &inputs,
        seed.sources(),
        &mut codec,
        &mut env_budget,
    )
    .map_err(err)?;
    let mut current = make();
    current.environments = &alternate_environments;
    check(current, Difference::EnvironmentInstance);
    let mut current = make();
    current.request.start = current.request.start.checked_add(1).ok_or("start")?;
    check(current, Difference::Range);
    let mut current = make();
    current.request.limit = current.request.limit.checked_add(1).ok_or("limit")?;
    check(current, Difference::Range);
    let mut current = make();
    current.request.final_input = !current.request.final_input;
    check(current, Difference::FinalInput);
    let mut entry = seed.request().entry.clone();
    entry.category.push_str("Changed");
    let mut current = make();
    current.request.entry = &entry;
    check(current, Difference::Entry);
    let original = seed.request().snapshot;
    for (revision, uri, text) in [
        (
            original
                .identity()
                .revision
                .checked_add(1)
                .ok_or("revision")?,
            original.uri().to_string(),
            original.text().to_string(),
        ),
        (
            original.identity().revision,
            "memory:changed".to_string(),
            original.text().to_string(),
        ),
        (
            original.identity().revision,
            original.uri().to_string(),
            format!("{}x", original.text()),
        ),
    ] {
        let snapshot = SourceSnapshot::new(
            original.identity().source.clone(),
            revision,
            uri,
            text.into_bytes(),
            &mut Budget::new(b.limits()),
        )
        .map_err(err)?;
        let mut current = make();
        current.request.snapshot = &snapshot;
        check(current, Difference::Snapshot);
    }
    let copy = SourceSnapshot::new(
        original.identity().source.clone(),
        original.identity().revision,
        original.uri().into(),
        original.text().as_bytes().to_vec(),
        &mut Budget::new(b.limits()),
    )
    .map_err(err)?;
    let mut current = make();
    current.request.snapshot = &copy;
    assert_eq!(
        context::compare_original(strict, &current, &mut Budget::new(b.limits())),
        Ok(None)
    );
    let mut states = seed.request().states.to_vec();
    states.push(nepl3_engine::parse::LanguageReaderState {
        alias: "extra".into(),
        state: NdfValue::Unit,
    });
    let mut current = make();
    current.request.states = &states;
    check(current, Difference::ReaderStateCount);
    if !seed.request().states.is_empty() {
        let mut states = seed.request().states.to_vec();
        states[0].alias.push_str("Changed");
        let mut current = make();
        current.request.states = &states;
        check(current, Difference::ReaderState { index: 0 });
        let mut states = seed.request().states.to_vec();
        states[0].state = NdfValue::List(vec![states[0].state.clone()]);
        let mut current = make();
        current.request.states = &states;
        check(current, Difference::ReaderState { index: 0 });
    }
    assert_eq!(*checked.original().execution().report(), before);
    Ok(())
}
