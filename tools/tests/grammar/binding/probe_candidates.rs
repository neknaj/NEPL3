use super::*;
use nepl3_core::budget::{Resource, StopReason};
use nepl3_engine::{
    analysis::{BindingOptions, completion::CandidateError, probe::candidates::*},
    binding::probe::ProbeOutcome,
    portable::analysis as keyed,
};

#[test]
fn missing_reference_candidates_preserve_visibility_and_late_stops() -> Result<(), String> {
    let lexical = super::missing_probe::named_lambda()?;
    let global = super::missing_probe::named_lambda_from(super::global::compiled()?)?;
    for (input, prefix, is_global, expected) in [
        ("let a 1 lambda b", "", false, vec!["b", "a"]),
        ("let a 1 lambda a", "", false, vec!["a"]),
        (
            "sequence cons define a b nil lambda x",
            "",
            false,
            vec!["x", "a"],
        ),
        (
            "recursive cons define a 1 cons define a 2 nil lambda x",
            "",
            false,
            vec!["x", "a"],
        ),
        (
            "repeat cons define a 1 nil lambda x",
            "",
            false,
            vec!["x", "a"],
        ),
        ("let outer 1 guest lambda inner", "", false, vec!["inner"]),
        ("lambda あ", "あ", false, vec!["あ"]),
        ("lambda é", "e\u{301}", false, vec![]),
        ("lambda A", "a", false, vec![]),
        ("lambda あ", "not", false, vec![]),
        ("let a 1 lambda b", "", true, vec!["b", "a"]),
    ] {
        let compiled = if is_global { &global } else { &lexical };
        with_source_profile(compiled, None, input, |source, profile, b, a| {
            let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
            let ParseCompletion::Break(ParseReply {
                outcome: ParseOutcome::Recovered { tree, .. },
                ..
            }) = parsed
            else {
                return Err(format!("recovered {input}"));
            };
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let limits = budget().limits();
            let prepared = keyed::prepare(
                "probe-names",
                &tree,
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = prepared
                .probe_missing_reference(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            let ProbeOutcome::Hit(hit) = &bound.reply().outcome else {
                return Err(format!("hit {input}: {:?}", bound.reply().outcome));
            };
            let original = bound.reply().report.clone();
            if input.starts_with("sequence") {
                assert!(!original.diagnostics.is_empty());
            }
            let source_ref = source.reference();
            let request = ProbeCandidateRequest {
                key: bound.key(),
                source: &source_ref,
                offset: input.len() as u64,
                prefix,
            };
            let mut query = Budget::new(limits);
            let result = names(
                &bound,
                &request,
                &mut query,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            assert_eq!(result.key(), bound.key());
            assert_eq!(result.report().usage, query.usage());
            let ProbeCandidateOutcome::Hit {
                hit: result_hit,
                candidates,
            } = result.outcome()
            else {
                return Err("candidate hit".into());
            };
            assert!(core::ptr::eq(*result_hit, hit.as_ref()));
            assert_eq!(
                candidates
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "{input}"
            );
            for candidate in candidates {
                match &candidate.resolution {
                    ReferenceResolution::Resolved(_) => {}
                    ReferenceResolution::Ambiguous(ids)
                        if input.starts_with("recursive") && candidate.name == "a" =>
                    {
                        assert_eq!(ids.len(), 2)
                    }
                    _ => return Err(format!("unexpected resolution: {input}")),
                }
            }
            if input == "let a 1 lambda a" {
                let stage = hit
                    .stages()
                    .get(hit.site().namespace_stage.0 as usize)
                    .ok_or("inner stage")?;
                let inner = *stage.introduced.first().ok_or("inner entity")?;
                assert!(
                    matches!(candidates.first().map(|c| &c.resolution), Some(ReferenceResolution::Resolved(id)) if *id == inner)
                );
            }
            if input.starts_with("recursive") {
                assert!(candidates.iter().any(|c| c.name == "a" && matches!(&c.resolution, ReferenceResolution::Ambiguous(ids) if ids.len() == 2)));
            }
            if input == "let a 1 lambda b" && !is_global {
                let measured = query.usage();
                for (resource, total, used, reason) in [
                    (
                        Resource::Work,
                        limits.work,
                        measured.work,
                        StopReason::WorkLimit,
                    ),
                    (
                        Resource::Nodes,
                        limits.nodes,
                        measured.nodes,
                        StopReason::NodeLimit,
                    ),
                    (
                        Resource::AllocationUnits,
                        limits.allocation_units,
                        measured.allocation_units,
                        StopReason::AllocationLimit,
                    ),
                ] {
                    let mut limited = Budget::new(limits);
                    limited.charge(resource, total - used + 1).map_err(err)?;
                    let failed = names(
                        &bound,
                        &request,
                        &mut limited,
                        &mut SourceAdmission::default(),
                    );
                    assert!(
                        matches!(failed, Err(ProbeCandidateError::Candidate(CandidateError::Stopped(actual))) if actual == reason),
                        "late {resource:?}"
                    );
                    assert_eq!(bound.reply().report, original);
                }
            }
            assert_eq!(bound.reply().report, original);
            Ok(())
        })?;
    }
    Ok(())
}
