use super::*;
use nepl3_core::source::SourceStore;

fn source(id: &str, text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(id.into()),
        0,
        "memory:test".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}

#[test]
#[ignore = "host accepted-prefix growth measurement; run explicitly"]
fn accepted_prefix_growth_measurement() -> Result<(), crate::runtime::ReaderError> {
    extern crate std;
    for count in [128usize, 256, 512] {
        for proved in [false, true] {
            let mut environment = SourceStore::default();
            environment.prepare_scope(&mut budget())?;
            let input = source("input", "x")?;
            let mut measured = Budget::new(Limits {
                work: 100_000_000,
                allocation_units: 100_000_000,
                ..budget().limits()
            });
            let mut accepted = AcceptedTokenizationReport::empty(
                TokenizationScope {
                    operation_id: "measurement".into(),
                    profile_digest: Digest([0; 32]),
                    snapshot: input.reference(),
                },
                &mut measured,
            )?;
            let mut checks = crate::tokenizer::source_checks::SourceChecks::default();
            let mut elapsed = 0;
            let before = measured.usage().work;
            for index in 0..count {
                // Construction and insertion into the collector are outside
                // this conflict-check-only measurement.
                accepted
                    .sources
                    .push(source(&alloc::format!("source-{index:06}"), "x")?);
                let start = std::time::Instant::now();
                checks.check(
                    accepted.unchecked_sources(&environment)?,
                    &environment,
                    &mut measured,
                )?;
                if proved {
                    accepted.retain_checked_prefix(&environment, accepted.sources.len())?;
                }
                elapsed += start.elapsed().as_nanos();
            }
            std::println!(
                "prefix_probe proved={proved} sources={count} work={} allocation_units={} elapsed_ns={elapsed}",
                measured.usage().work - before,
                measured.usage().allocation_units
            );
        }
    }
    Ok(())
}

#[test]
fn conflict_proof_is_bound_to_owned_prefix_and_environment()
-> Result<(), crate::runtime::ReaderError> {
    let a = source("a", "a")?;
    let b = source("b", "b")?;
    let wrong_b = source("b", "wrong")?;
    let mut environment = SourceStore::default();
    environment.insert(a.clone())?;
    environment.insert(b.clone())?;
    environment.prepare_scope(&mut budget())?;
    let mut operation = budget();
    let mut accepted = AcceptedTokenizationReport::empty(
        TokenizationScope {
            operation_id: "test".into(),
            profile_digest: Digest([0; 32]),
            snapshot: a.reference(),
        },
        &mut operation,
    )?;
    accepted.sources.push(a.clone());
    let mut checks = crate::tokenizer::source_checks::SourceChecks::default();
    checks.check(
        accepted.unchecked_sources(&environment)?,
        &environment,
        &mut operation,
    )?;
    accepted.retain_checked_prefix(&environment, 1)?;
    let checkpoint = accepted.checkpoint(&mut operation)?;
    let mut branch = checkpoint.checkpoint(&mut operation)?;
    let prefix = crate::tokenizer::recovery::Prefix::capture(&accepted);
    accepted.sources.push(b.clone());
    checks.check(
        accepted.unchecked_sources(&environment)?,
        &environment,
        &mut operation,
    )?;
    accepted.retain_checked_prefix(&environment, 2)?;
    // Same length in another checkpoint branch cannot reuse the first branch's
    // checked suffix. The new conflicting declaration must be inspected.
    branch.sources.push(wrong_b.clone());
    assert_eq!(
        checks.check(
            branch.unchecked_sources(&environment)?,
            &environment,
            &mut operation
        ),
        Err(SourceError::IdentityConflict)
    );
    let (report, sources, source_maps) = accepted.into_parts();
    let mut restored = prefix
        .restore(crate::runtime::AcceptedReport {
            report,
            sources,
            source_maps,
        })
        .map_err(|()| crate::runtime::ReaderError::Continuation)?;
    restored.sources.push(wrong_b);
    assert_eq!(
        checks.check(
            restored.unchecked_sources(&environment)?,
            &environment,
            &mut operation
        ),
        Err(SourceError::IdentityConflict)
    );

    // A separately prepared store, even of equal length, grants no old proof.
    let mut foreign = SourceStore::default();
    foreign.insert(source("a", "changed")?)?;
    foreign.insert(b)?;
    foreign.prepare_scope(&mut budget())?;
    assert_eq!(
        checks.check(
            checkpoint.unchecked_sources(&foreign)?,
            &foreign,
            &mut operation
        ),
        Err(SourceError::IdentityConflict)
    );
    // Stopping a suffix check does not advance the collector's proof.
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(
        checks
            .check(
                restored.unchecked_sources(&environment)?,
                &environment,
                &mut stopped
            )
            .is_err()
    );
    assert_eq!(
        checks.check(
            restored.unchecked_sources(&environment)?,
            &environment,
            &mut operation
        ),
        Err(SourceError::IdentityConflict)
    );
    assert!(restored.retain_checked_prefix(&environment, 3).is_err());
    Ok(())
}
