use super::*;
use crate::doc::source::{budget, compiled};
use nepl3_core::budget::Resource;
fn entry(id: &str) -> Entry {
    Entry {
        id: id.into(),
        source: format!("{id}.nepld"),
        route: format!("{id}.html"),
        input: None,
    }
}
fn total(attempts: &[PageAttempt]) -> Usage {
    attempts
        .iter()
        .flat_map(|a| &a.stages)
        .fold(Usage::default(), |sum, stage| {
            super::super::add(sum, stage.usage)
        })
}
#[test]
fn completed_page_survives_repeated_stops_and_resume() -> Result<(), String> {
    let compiled = compiled()?;
    let first = r#"article en "First" body nil"#.to_string();
    let second =
        r#"article en "Second" body cons paragraph cons "Longer second page." nil nil"#.to_string();
    let mut phases = PreparationLimits::default();
    let inputs = || {
        vec![
            (entry("first"), first.clone()),
            (entry("second"), second.clone()),
        ]
    };
    let Progress::Prepared(baseline) =
        Preparation::new(&compiled, inputs(), budget().limits(), phases)?.run()
    else {
        return Err("baseline must prepare".into());
    };
    let first_source = baseline.attempts()[0].stages[0].usage.source_bytes;
    let second_source = baseline.attempts()[1].stages[0].usage.source_bytes;
    assert!(second_source > first_source);
    phases.parse_limits.source_bytes = first_source;
    let Progress::Paused { job, .. } =
        Preparation::new(&compiled, inputs(), budget().limits(), phases)?.run()
    else {
        return Err("second page must pause".into());
    };
    assert_eq!(job.prepared_count(), 1);
    assert_eq!(job.reason(), StopReason::SourceLimit);
    assert_eq!(job.usage(), total(job.attempts()));
    let first_profile = job.attempts()[0].stages[0].profile;
    let Progress::Paused { job, .. } = job
        .resume(budget().limits(), phases)
        .map_err(|_| "same grant")?
        .run()
    else {
        return Err("same cap still pauses".into());
    };
    assert_eq!(job.prepared_count(), 1);
    assert_eq!(
        job.attempts()
            .iter()
            .map(|a| a.page_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 1]
    );
    assert_eq!(job.usage(), total(job.attempts()));
    let mut smaller = phases;
    smaller.lower_limits.work -= 1;
    let job = job
        .resume(budget().limits(), smaller)
        .err()
        .ok_or("shrinking phase must reject")?
        .into_paused();
    assert_eq!(job.prepared_count(), 1);
    phases.parse_limits.source_bytes = second_source;
    let Progress::Prepared(ready) = job
        .resume(budget().limits(), phases)
        .map_err(|_| "larger source grant")?
        .run()
    else {
        return Err("both pages should prepare".into());
    };
    assert_eq!(ready.prepared_count(), 2);
    assert_eq!(ready.documents().count(), 2);
    assert_eq!(
        ready
            .attempts()
            .iter()
            .map(|a| a.page_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 1, 1]
    );
    assert_eq!(ready.attempts()[0].stages[0].profile, first_profile);
    assert_eq!(ready.usage(), total(ready.attempts()));
    Ok(())
}
#[test]
fn lower_and_code_stops_settle_parse_once_and_preserve_failed_attempts() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en "Host" body cons paragraph cons code Doc article en "Nested" body nil nil nil"#;
    for stopped in [Stage::Lower, Stage::Code] {
        let mut phases = PreparationLimits::default();
        match stopped {
            Stage::Lower => phases.lower_limits.work = 0,
            _ => phases.code_limits.work = 0,
        }
        let Progress::Paused { job, .. } = Preparation::new(
            &compiled,
            vec![(entry("host"), source.into())],
            budget().limits(),
            phases,
        )?
        .run() else {
            return Err(format!("expected {stopped:?} pause"));
        };
        assert_eq!(job.reason(), StopReason::WorkLimit);
        assert_eq!(
            job.attempts()[0]
                .stages
                .iter()
                .filter(|s| s.stage == Stage::Parse)
                .count(),
            1
        );
        assert_eq!(
            job.attempts()[0].stages.last().map(|s| s.stage),
            Some(stopped)
        );
        assert_eq!(job.usage(), total(job.attempts()));
        let prior = job.usage();
        let progress = job
            .resume(budget().limits(), PreparationLimits::default())
            .map_err(|_| "resume")?
            .run();
        let ready = match progress {
            Progress::Prepared(ready) => ready,
            Progress::Paused { job, error } => {
                return Err(format!(
                    "retry {stopped:?} paused {:?}: {error:?}, {:?}",
                    job.reason(),
                    job.attempts()
                ));
            }
            Progress::Failed { error, .. } => {
                return Err(format!("retry {stopped:?} failed: {error}"));
            }
            Progress::Cancelled(_) => return Err("retry cancelled".into()),
        };
        assert_eq!(ready.attempts().len(), 2);
        assert!(ready.usage().work > prior.work);
        assert_eq!(ready.usage(), total(ready.attempts()));
    }
    Ok(())
}
#[test]
fn independent_depth_peaks_and_remaining_caps_are_settled_exactly() -> Result<(), String> {
    let mut cap = budget().limits();
    cap.work = 10;
    cap.depth = 20;
    let mut observations = Vec::new();
    let Attempt::Complete { running, .. } = Running::new(cap).run(cap, |meter| {
        stage(
            meter,
            Stage::Parse,
            cap,
            Digest::of(b"profile"),
            &mut observations,
            |b| {
                b.charge(Resource::Work, 6).map_err(err)?;
                b.observe_depth(15).map_err(err)
            },
        )?;
        let mut low = cap;
        low.depth = 3;
        stage(
            meter,
            Stage::Lower,
            low,
            Digest::of(b"profile"),
            &mut observations,
            |b| {
                assert_eq!(b.limits().work, 4);
                b.charge(Resource::Work, 4).map_err(err)?;
                b.observe_depth(2).map_err(err)
            },
        )
    }) else {
        return Err("independent lower depth should fit".into());
    };
    assert_eq!(running.usage().work, 10);
    assert_eq!(running.usage().depth, 15);
    assert_eq!(observations[1].usage.depth, 2);
    assert_eq!(observations[1].effective.work, 4);
    assert_eq!(observations[1].requested.work, 10);
    Ok(())
}
#[test]
fn malformed_input_is_terminal_and_cancelled_pause_has_no_continuation() -> Result<(), String> {
    let compiled = compiled()?;
    let failed = Preparation::new(
        &compiled,
        vec![(entry("bad"), "not-a-document".into())],
        budget().limits(),
        PreparationLimits::default(),
    )?
    .run();
    let Progress::Failed {
        usage, attempts, ..
    } = failed
    else {
        return Err("malformed source must fail".into());
    };
    assert_eq!(usage, total(&attempts));
    assert_eq!(attempts.len(), 1);
    assert!(attempts[0].stages[0].failed);
    let mut phases = PreparationLimits::default();
    phases.parse_limits.work = 0;
    let Progress::Paused { job, .. } = Preparation::new(
        &compiled,
        vec![(entry("good"), r#"article en "Good" body nil"#.into())],
        budget().limits(),
        phases,
    )?
    .run() else {
        return Err("zero parse work must pause".into());
    };
    assert_eq!(job.attempts()[0].stages.len(), 1);
    assert_eq!(job.usage(), total(job.attempts()));
    let cancelled = job.cancel();
    assert_eq!(
        cancelled.accounting.previous_stop,
        Some(StopReason::WorkLimit)
    );
    Ok(())
}

#[test]
fn rejected_settlement_retains_typed_invariant_evidence() -> Result<(), String> {
    let mut cap = budget().limits();
    cap.work = 1;
    let mut records = Vec::new();
    let result = Running::new(cap).run(cap, |meter| {
        let usage = Usage {
            work: 2,
            ..Usage::default()
        };
        settle(
            meter,
            Observation {
                stage: Stage::Lower,
                requested: cap,
                effective: cap,
                usage,
                stop: None,
                failed: false,
                accounting_rejected: false,
                profile: None,
            },
            &mut records,
        )
    });
    assert!(matches!(result, Attempt::Paused { .. }));
    // Preparation consumes this low-level pause as terminal failure; it never
    // exposes resumption authority when the settlement evidence is rejected.
    assert_eq!(records.len(), 1);
    assert!(records[0].accounting_rejected);
    assert_eq!(records[0].usage.work, 2);
    Ok(())
}
