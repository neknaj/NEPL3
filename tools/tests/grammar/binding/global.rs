use super::*;
pub(super) fn compiled() -> Result<CompiledLanguage, String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../../conformance/fixtures/grammar/binding/global.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.binding-global",
        &mut budget(),
        &mut SourceAdmission::default(),
    )
}
type ReferencePositions = [(u64, &'static [u64])];
struct Case {
    input: &'static str,
    references: &'static ReferencePositions,
    duplicate: Option<(u64, u64)>,
}
#[test]
fn global_visibility_is_ordered_and_duplicates_keep_both_locations() -> Result<(), String> {
    let compiled = compiled()?;
    // These raw declaration locations follow spec04, not assigned Entity IDs.
    let cases = [
        Case {
            input: "lettext \"\\u{3042}\" lettext \"\u{3042}\" \u{3042}",
            references: &[],
            duplicate: Some((27, 8)),
        },
        Case {
            input: "apply lambda x x x",
            references: &[(15, &[13]), (17, &[13])],
            duplicate: None,
        },
        Case {
            input: "apply x lambda x x",
            references: &[(6, &[]), (17, &[15])],
            duplicate: None,
        },
        Case {
            input: "lambda x apply lambda y x y",
            references: &[(24, &[7]), (26, &[22])],
            duplicate: None,
        },
        Case {
            input: "let x x x",
            references: &[(6, &[]), (8, &[4])],
            duplicate: None,
        },
        Case {
            input: "imported define x x x",
            references: &[(18, &[]), (20, &[16])],
            duplicate: None,
        },
        Case {
            input: "apply unimported define x 1 x",
            references: &[(28, &[])],
            duplicate: None,
        },
        Case {
            input: "recursive cons define a b cons define b a nil a",
            references: &[(24, &[38]), (40, &[22]), (46, &[22])],
            duplicate: None,
        },
        Case {
            input: "sequence cons define a b cons define b a nil a",
            references: &[(23, &[]), (39, &[21]), (45, &[21])],
            duplicate: None,
        },
        Case {
            input: "lambda x apply guest x x",
            references: &[(21, &[]), (23, &[7])],
            duplicate: None,
        },
        Case {
            input: "lambda x guest lambda x x",
            references: &[(24, &[22])],
            duplicate: None,
        },
        Case {
            input: "apply \u{3042} lambda \u{3042} \u{3042}",
            references: &[(6, &[]), (21, &[17])],
            duplicate: None,
        },
        Case {
            input: "lambda x lambda x x",
            references: &[],
            duplicate: Some((16, 7)),
        },
        Case {
            input: "twice x x x",
            references: &[],
            duplicate: Some((8, 6)),
        },
        Case {
            input: "sequence cons define x 1 cons define x 2 nil x",
            references: &[],
            duplicate: Some((37, 21)),
        },
        Case {
            input: "recursive cons define x 1 cons define x 2 nil x",
            references: &[],
            duplicate: Some((38, 22)),
        },
    ];
    for case in cases {
        with_input(&compiled, case.input, |tree, profile, b, a| {
            let reply = analyze("global", tree, profile, b, a);
            match (&reply.outcome, case.duplicate) {
                (BindingOutcome::Complete(analysis), None) => {
                    let facts = analysis.facts();
                    let references: Vec<_> = facts
                        .occurrences
                        .iter()
                        .filter(|o| o.role == OccurrenceRole::Reference)
                        .collect();
                    assert_eq!(references.len(), case.references.len(), "{}", case.input);
                    for (start, expected) in case.references {
                        let reference = references
                            .iter()
                            .find(|o| o.span.start() == *start)
                            .ok_or("reference")?;
                        let ids = match &reference.resolution {
                            ReferenceResolution::Resolved(id) => vec![*id],
                            ReferenceResolution::Unresolved(_) => vec![],
                            other => return Err(format!("{other:?}")),
                        };
                        let actual: Vec<_> = ids
                            .iter()
                            .map(|id| {
                                facts.entities[id.0 as usize]
                                    .selection
                                    .as_ref()
                                    .map(|s| s.start())
                            })
                            .collect();
                        let expected: Vec<_> = expected.iter().map(|v| Some(*v)).collect();
                        assert_eq!(actual, expected, "{} at {start}", case.input);
                    }
                    for entity in &facts.entities {
                        assert_eq!(
                            entity.scope,
                            facts.namespaces[entity.namespace.0 as usize].root
                        );
                    }
                    for location in &analysis.result().occurrence_stages {
                        let occurrence = &facts.occurrences[location.occurrence.0 as usize];
                        assert_eq!(
                            analysis.result().stages[location.stage.0 as usize].scope,
                            occurrence.scope
                        );
                        assert_eq!(
                            analysis.result().stages[location.namespace_stage.0 as usize].scope,
                            facts.namespaces[occurrence.namespace.0 as usize].root
                        );
                    }
                }
                (
                    BindingOutcome::Invalid {
                        error: BindingError::DuplicateGlobal,
                        ..
                    },
                    Some((primary, related)),
                ) => {
                    let diagnostic = reply
                        .report
                        .diagnostics
                        .iter()
                        .find(|d| d.code == "DuplicateGlobalName")
                        .ok_or("duplicate diagnostic")?;
                    assert_eq!(
                        diagnostic.primary.as_ref().ok_or("primary")?.start(),
                        primary
                    );
                    assert_eq!(diagnostic.related.len(), 1);
                    assert_eq!(
                        diagnostic.related[0]
                            .span
                            .as_ref()
                            .ok_or("related")?
                            .start(),
                        related
                    );
                }
                other => return Err(format!("{}: {other:?}", case.input)),
            }
            if let Some(facts) = reply.facts() {
                facts.validate(&compiled.registry, b, a).map_err(err)?;
            }
            reply
                .report
                .validate(
                    &SourceStore::default(),
                    reply.sources(),
                    &compiled.registry,
                    b,
                )
                .map_err(err)?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn global_duplicate_construction_stops_keep_the_formal_prefix() -> Result<(), String> {
    let compiled = compiled()?;
    for input in [
        "lambda x lambda x x",
        "sequence cons define x 1 cons define x 2 nil x",
        "recursive cons define x 1 cons define x 2 nil x",
        "apply free lambda x lambda x x",
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut baseline = budget();
            let reply = analyze(
                "global-stop",
                tree,
                profile,
                &mut baseline,
                &mut SourceAdmission::default(),
            );
            assert!(matches!(
                reply.outcome,
                BindingOutcome::Invalid {
                    error: BindingError::DuplicateGlobal,
                    ..
                }
            ));
            for resource in 0..3 {
                let total = match resource {
                    0 => baseline.usage().work,
                    1 => baseline.usage().allocation_units,
                    _ => baseline.usage().diagnostics,
                };
                for cap in (0..=total).step_by((total / 64).max(1) as usize) {
                    let mut limits = budget().limits();
                    match resource {
                        0 => limits.work = cap,
                        1 => limits.allocation_units = cap,
                        _ => limits.diagnostics = cap,
                    };
                    let mut b = Budget::new(limits);
                    let reply = analyze(
                        "global-stop",
                        tree,
                        profile,
                        &mut b,
                        &mut SourceAdmission::default(),
                    );
                    match reply.outcome {
                        BindingOutcome::Stopped { reason, .. } => assert_eq!(b.poll(), Err(reason)),
                        BindingOutcome::Invalid {
                            error: BindingError::DuplicateGlobal,
                            ..
                        } => assert!(b.poll().is_ok()),
                        ref other => {
                            return Err(format!(
                                "{input} resource {resource} cap {cap}: {other:?}"
                            ));
                        }
                    }
                    if let Some(facts) = reply.facts() {
                        facts
                            .validate(
                                &compiled.registry,
                                &mut budget(),
                                &mut SourceAdmission::default(),
                            )
                            .map_err(err)?;
                    }
                    reply
                        .report
                        .validate(
                            &SourceStore::default(),
                            reply.sources(),
                            &compiled.registry,
                            &mut budget(),
                        )
                        .map_err(err)?;
                    assert_eq!(reply.report.usage, b.usage());
                    assert_eq!(b.current_depth(), 0);
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}
