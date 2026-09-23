use super::*;
use nepl3_engine::profile::{ProviderImplementation, ProviderRequirement};

#[test]
fn parsing_and_recursive_evaluation_share_resolved_profile() -> Result<(), String> {
    for (input, expected) in [
        ("add framed frame neg 7 2", -5_i64),
        ("mul 3 framed frame add 1 1", 6),
        ("framed frame framed frame neg 9", -9),
    ] {
        check(input, expected, 0)?;
    }
    Ok(())
}

#[test]
fn profile_authority_rejects_missing_identity_allowlist_and_limits() -> Result<(), String> {
    for case in 1..5 {
        check("add framed frame neg 7 2", -5, case)?;
    }
    Ok(())
}

fn check(input: &str, expected: i64, case: u8) -> Result<(), String> {
    let mut languages = composition::languages("Expr", "Frame")?;
    let expr = languages.packages[0].1.schema.clone();
    let frame = languages.packages[1].1.schema.clone();
    let implementations = [
        Digest::of(b"MiniExpr test evaluator"),
        Digest::of(b"Frame test evaluator"),
    ];
    let runtime = Runtime::register(&mut languages.registry, implementations, &mut budget())
        .map_err(error)?;
    languages.registry.finalize(&mut budget()).map_err(error)?;
    let mut profile = languages.profile("composition")?;
    profile.schemas.push(runtime.operations()[0].schema.clone());
    let mut hosts = Vec::new();
    for (index, operation) in runtime.operations().iter().enumerate() {
        let provider = format!("test.evaluator.{index}");
        profile.providers.push(ProviderRequirement {
            provider: provider.clone(),
            revision: 1,
            implementation_digest: implementations[index],
            operation: operation.clone(),
        });
        profile.allowlist.push(operation.clone());
        hosts.push(ProviderImplementation {
            provider,
            revision: 1,
            implementation_digest: implementations[index],
            operations: vec![operation.clone()],
        });
    }
    match case {
        1 => {
            hosts.pop();
        }
        2 => {
            hosts[1].implementation_digest = Digest::of(b"unapproved implementation");
        }
        3 => {
            profile.allowlist.pop();
        }
        _ => {}
    }
    let entered = core::cell::Cell::new(false);
    let outcome = external_hello_language::with_registered_profile(
        input,
        true,
        false,
        (languages, profile),
        ("Expr", "composition"),
        &hosts,
        |reply, resolved, registry, _| {
            entered.set(true);
            let ParseOutcome::Complete { tree, .. } = reply.outcome else {
                return Err("expected complete composition".into());
            };
            let proof = tree
                .validate(resolved, &mut budget(), &mut SourceAdmission::default())
                .map_err(error)?;
            let cursor = Cursor::root(&proof, &expr, &frame, &mut budget()).map_err(error)?;
            let plan = program::compile(cursor, &mut budget()).map_err(error)?;
            let mut sources = SourceStore::default();
            for source in &tree.bundle.sources {
                sources
                    .insert_ref_with_budget(source, &mut budget())
                    .map_err(error)?;
            }
            let mut execution = budget();
            if case == 4 {
                let mut limits = execution.limits();
                limits.work += 1;
                execution = Budget::new(limits);
            }
            let session = runtime
                .prepare(&plan, &sources, registry, execution.limits(), &mut budget())
                .map_err(error)?;
            let mut awaits = Vec::new();
            let mut cancellations = Vec::new();
            let result = session.run_in_profile(
                resolved,
                &mut execution,
                &mut budget(),
                |id, _| awaits.push(id),
                |id| cancellations.push(id),
            );
            if case == 3 || case == 4 {
                assert!(matches!(
                    (case, result),
                    (
                        3,
                        Err(Error::Profile(nepl3_suite::profile::Error::Profile(
                            nepl3_engine::profile::ProfileError::NotAllowed
                        )))
                    ) | (4, Err(Error::Profile(nepl3_suite::profile::Error::Limits)))
                ));
                assert_eq!(execution.usage().work, 0);
                assert!(awaits.is_empty());
                assert!(cancellations.is_empty());
                return Ok(());
            }
            let result = result.map_err(error)?;
            let OperationResult::Complete {
                value: TypedValue::Record(value),
                ..
            } = result
            else {
                return Err("expected complete evaluation".into());
            };
            // Independent integer expectations describe the language meaning;
            // parse/print roundtrip is not the arithmetic oracle.
            assert_eq!(value.fields, [NdfValue::Integer(Integer::from(expected))]);
            assert!(!awaits.is_empty());
            assert!(cancellations.is_empty());
            Ok(())
        },
    );
    if case == 1 || case == 2 {
        assert!(!entered.get());
        assert_eq!(
            outcome,
            Err(if case == 1 {
                "MissingProvider"
            } else {
                "ProviderIdentity"
            }
            .into())
        );
    } else {
        outcome?;
    }
    Ok(())
}
