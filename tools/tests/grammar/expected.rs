use super::*;
use nepl3_engine::{
    analysis::{BindingOptions, PreparedBindingRequest, expected::*},
    parse::{ParseCompletion, ParseOutcome},
    portable::analysis,
    recovery::ParseTree,
};
use nepl3_wire::foundation::FoundationCodec;
#[path = "expected/list.rs"]
mod list;
#[path = "expected/portable.rs"]
mod portable;
#[path = "expected/shared.rs"]
mod shared;
fn err(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}
fn with_input<T>(
    text: &str,
    mutate: impl FnOnce(&mut ParseTree) -> Result<(), String>,
    finish: impl FnOnce(
        &PreparedBindingRequest<'_, '_>,
        &SourceSnapshot,
        &nepl3_engine::profile::ResolvedParseProfile<'_>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_configured_input(text, |_| Ok(()), mutate, finish)
}
fn with_configured_input<T>(
    text: &str,
    configure: impl FnOnce(&mut CompiledLanguage) -> Result<(), String>,
    mutate: impl FnOnce(&mut ParseTree) -> Result<(), String>,
    finish: impl FnOnce(
        &PreparedBindingRequest<'_, '_>,
        &SourceSnapshot,
        &nepl3_engine::profile::ResolvedParseProfile<'_>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let doc = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/grammar/binding/execution.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut compiled = nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.expected",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    configure(&mut compiled)?;
    super::binding::with_completed_input(&compiled, "x", |_, profile, _, _| {
        let source = SourceSnapshot::new(
            SourceId("expected-input".into()),
            0,
            "memory:expected-input".into(),
            text.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        let parsed = super::binding::parse_any_with_aux(
            &source,
            &[],
            &[],
            profile,
            &mut budget(),
            &mut SourceAdmission::default(),
        )?;
        let mut tree = match parsed {
            ParseCompletion::Continue(value) => value.tree().clone(),
            ParseCompletion::Break(reply) => match reply.outcome {
                ParseOutcome::Recovered { tree, .. } => tree,
                outcome => return Err(err(outcome)),
            },
        };
        mutate(&mut tree)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let input = analysis::prepare(
            "expected",
            &tree,
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        finish(&input, &source, profile)
    })
}
#[test]
fn direct_missing_preserves_first_incoming_read_and_foreign_path() -> Result<(), String> {
    for (text, field, foreign_count) in [
        ("let", 0, 0),
        ("let x", 1, 0),
        ("lambda", 0, 0),
        ("lambda x", 1, 0),
        ("guest guest lambda", 0, 2),
    ] {
        with_input(
            text,
            |_| Ok(()),
            |input, source, _| {
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: text.len() as u64,
                };
                let result = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let ExpectedReadOutcome::Complete(Some(value)) = &result.outcome else {
                    return Err(err(result));
                };
                assert_eq!(value.path.last(), Some(&ExpectedReadStep::Child { field }));
                assert_eq!(
                    value
                        .path
                        .iter()
                        .filter(|v| matches!(v, ExpectedReadStep::Foreign { .. }))
                        .count(),
                    foreign_count
                );
                assert!(
                    matches!(value.origin, ExpectedReadOrigin::Field { field: f, .. } if f == field)
                );
                assert_eq!(result.sources, vec![source.clone()]);
                Ok(())
            },
        )?;
    }
    with_input(
        "",
        |_| Ok(()),
        |input, source, _| {
            let request = ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 0,
            };
            let result = expected_read(
                input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let ExpectedReadOutcome::Complete(Some(value)) = result.outcome else {
                return Err(err(result));
            };
            assert_eq!(value.origin, ExpectedReadOrigin::Root);
            assert!(value.path.is_empty());
            Ok(())
        },
    )
}
#[test]
fn direct_query_checks_boundaries_even_when_no_missing_exists() -> Result<(), String> {
    with_input(
        "あ",
        |_| Ok(()),
        |input, source, _| {
            for offset in [0, 3] {
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset,
                };
                let result = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                assert_eq!(result.outcome, ExpectedReadOutcome::Complete(None));
                assert_eq!(result.sources, vec![source.clone()]);
            }
            for offset in [1, 2, 4] {
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset,
                };
                let result = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                assert!(matches!(
                    result.outcome,
                    ExpectedReadOutcome::Invalid(ExpectedReadError::Source(_))
                ));
                assert!(result.sources.is_empty());
            }
            Ok(())
        },
    )
}
#[test]
fn direct_query_rejects_stale_keys_limits_and_retains_atomic_stops() -> Result<(), String> {
    with_input(
        "lambda",
        |_| Ok(()),
        |input, source, _| {
            let request = ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 6,
            };
            let mut stale = request.clone();
            stale.key.execution_digest = Digest::of(b"other");
            assert!(matches!(
                expected_read(
                    input,
                    &stale,
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .outcome,
                ExpectedReadOutcome::Invalid(_)
            ));
            let mut limits = budget().limits();
            limits.nodes -= 1;
            assert_eq!(
                expected_read(
                    input,
                    &request,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default()
                )
                .outcome,
                ExpectedReadOutcome::Invalid(ExpectedReadError::Access(
                    nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                ))
            );
            for limits in [
                Limits {
                    work: 0,
                    ..budget().limits()
                },
                Limits {
                    depth: 0,
                    ..budget().limits()
                },
            ] {
                let mut b = Budget::new(limits);
                let result =
                    expected_read(input, &request, &mut b, &mut SourceAdmission::default());
                assert_eq!(
                    result.outcome,
                    ExpectedReadOutcome::Invalid(ExpectedReadError::Access(
                        nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                    ))
                );
                assert_eq!(result.report.usage, Usage::default());
                assert!(result.sources.is_empty());
            }
            for resource in [Resource::Work, Resource::Nodes, Resource::AllocationUnits] {
                let mut b = budget();
                let amount = match resource {
                    Resource::Work => b.limits().work,
                    Resource::Nodes => b.limits().nodes,
                    _ => b.limits().allocation_units,
                };
                b.charge(resource, amount).map_err(err)?;
                let result =
                    expected_read(input, &request, &mut b, &mut SourceAdmission::default());
                assert!(matches!(result.outcome, ExpectedReadOutcome::Stopped(_)));
                assert!(result.sources.is_empty());
                assert_eq!(result.report.usage, b.usage());
            }
            let mut b = budget();
            b.cancel();
            let result = expected_read(input, &request, &mut b, &mut SourceAdmission::default());
            assert_eq!(
                result.outcome,
                ExpectedReadOutcome::Stopped(StopReason::Cancelled)
            );
            assert!(result.sources.is_empty());
            let baseline = expected_read(
                input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(matches!(
                baseline.outcome,
                ExpectedReadOutcome::Complete(Some(_))
            ));
            for (resource, used) in [
                (Resource::Work, baseline.report.usage.work),
                (
                    Resource::AllocationUnits,
                    baseline.report.usage.allocation_units,
                ),
            ] {
                let mut b = budget();
                let limit = if resource == Resource::Work {
                    b.limits().work
                } else {
                    b.limits().allocation_units
                };
                b.charge(resource, limit - used + 1).map_err(err)?;
                let result =
                    expected_read(input, &request, &mut b, &mut SourceAdmission::default());
                assert!(matches!(result.outcome, ExpectedReadOutcome::Stopped(_)));
                assert!(result.sources.is_empty());
                assert_eq!(b.current_depth(), 0);
                assert_eq!(result.report.usage, b.usage());
            }
            let mut b = budget();
            let base = b.limits().depth - 1;
            let result = b
                .with_depth_at_least(base, |b| {
                    Ok::<_, StopReason>(expected_read(
                        input,
                        &request,
                        b,
                        &mut SourceAdmission::default(),
                    ))
                })
                .map_err(err)?;
            assert_eq!(
                result.outcome,
                ExpectedReadOutcome::Stopped(StopReason::DepthLimit)
            );
            assert!(result.sources.is_empty());
            assert_eq!(b.current_depth(), 0);
            Ok(())
        },
    )
}
