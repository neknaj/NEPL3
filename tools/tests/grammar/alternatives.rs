use super::*;
use nepl3_engine::analysis::{alternatives::*, expected::ExpectedReadRequest};
use nepl3_reader::builtin::BuiltinReader;
#[path = "alternatives/portable.rs"]
mod portable;
fn err(value: impl std::fmt::Debug) -> String {
    format!("{value:?}")
}

#[test]
fn declared_alternatives_respect_actual_read_family() -> Result<(), String> {
    for (text, family) in [
        ("let", 0),
        ("lettext", 1),
        ("let x", 2),
        ("sequence", 3),
        ("", 2),
    ] {
        super::expected::with_input(
            text,
            |_| Ok(()),
            |input, source, _| {
                let result = declared_alternatives(
                    input,
                    &ExpectedReadRequest {
                        key: input.key(),
                        source: source.reference(),
                        offset: text.len() as u64,
                    },
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let DeclaredAlternativesOutcome::Complete(Some(value)) = result.outcome else {
                    return Err(err(result));
                };
                match family {
                    0 => assert_eq!(
                        value.alternatives,
                        DeclaredReadAlternatives::Builtin {
                            reader: BuiltinReader::Name
                        }
                    ),
                    1 => assert_eq!(
                        value.alternatives,
                        DeclaredReadAlternatives::Builtin {
                            reader: BuiltinReader::Text
                        }
                    ),
                    2 => {
                        let DeclaredReadAlternatives::Category {
                            forms,
                            leaf_declarations,
                            dynamic_fallback_registered,
                        } = value.alternatives
                        else {
                            return Err("category".into());
                        };
                        assert_eq!(
                            forms
                                .iter()
                                .take(3)
                                .map(|v| (v.index, v.spelling.as_str()))
                                .collect::<Vec<_>>(),
                            vec![(0, "let"), (1, "lambda"), (2, "apply")]
                        );
                        assert!(!forms.iter().any(|v| v.spelling == "define"));
                        assert!(leaf_declarations > 0);
                        assert!(!dynamic_fallback_registered);
                    }
                    _ => assert_eq!(value.alternatives, DeclaredReadAlternatives::List),
                }
                assert_eq!(result.sources, vec![source.clone()]);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn declared_form_is_not_claimed_to_tokenize_in_the_selected_mode() -> Result<(), String> {
    super::expected::with_configured_input(
        "lambda x",
        |compiled| {
            compiled.package.forms[0].spelling = "@".into();
            Ok(())
        },
        |_| Ok(()),
        |input, source, profile| {
            let result = declared_alternatives(
                input,
                &ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: source.text().len() as u64,
                },
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let DeclaredAlternativesOutcome::Complete(Some(value)) = result.outcome else {
                return Err(err(result));
            };
            let DeclaredReadAlternatives::Category { forms, .. } = value.alternatives else {
                return Err("category".into());
            };
            assert!(forms.iter().any(|v| v.index == 0 && v.spelling == "@"));
            let candidate = SourceSnapshot::new(
                SourceId("candidate".into()),
                0,
                "memory:candidate".into(),
                b"@".to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let parsed = super::binding::parse_any_with_aux(
                &candidate,
                &[],
                &[],
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
            let nepl3_engine::parse::ParseCompletion::Break(reply) = parsed else {
                return Err("reader unexpectedly accepted declaration spelling".into());
            };
            assert!(
                matches!(
                    reply.outcome,
                    nepl3_engine::parse::ParseOutcome::Recovered { .. }
                ),
                "{reply:?}"
            );
            Ok(())
        },
    )
}

#[test]
fn declaration_enumeration_stops_atomically_after_expected_read_succeeds() -> Result<(), String> {
    super::expected::with_input(
        "let x",
        |_| Ok(()),
        |input, source, _| {
            let request = ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 5,
            };
            let selected = nepl3_engine::analysis::expected::expected_read(
                input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(matches!(
                selected.outcome,
                nepl3_engine::analysis::expected::ExpectedReadOutcome::Complete(Some(_))
            ));
            for (resource, consumed, reason) in [
                (
                    Resource::Work,
                    selected.report.usage.work,
                    StopReason::WorkLimit,
                ),
                (
                    Resource::AllocationUnits,
                    selected.report.usage.allocation_units,
                    StopReason::AllocationLimit,
                ),
            ] {
                let mut b = budget();
                let limit = if resource == Resource::Work {
                    b.limits().work
                } else {
                    b.limits().allocation_units
                };
                b.charge(resource, limit - consumed - 1).map_err(err)?;
                let reply =
                    declared_alternatives(input, &request, &mut b, &mut SourceAdmission::default());
                assert_eq!(reply.outcome, DeclaredAlternativesOutcome::Stopped(reason));
                assert!(reply.sources.is_empty());
                assert_eq!(reply.report.usage, b.usage());
                assert_eq!(
                    reply.report.usage.source_bytes,
                    selected.report.usage.source_bytes
                );
                assert_eq!(b.current_depth(), 0);
                assert_eq!(b.poll(), Err(reason));
            }
            let mut b = Budget::new(Limits {
                depth: 0,
                ..budget().limits()
            });
            let reply =
                declared_alternatives(input, &request, &mut b, &mut SourceAdmission::default());
            assert!(matches!(
                reply.outcome,
                DeclaredAlternativesOutcome::Invalid(
                    nepl3_engine::analysis::expected::ExpectedReadError::Access(
                        nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                    )
                )
            ));
            assert_eq!(reply.report.usage, Usage::default());
            Ok(())
        },
    )
}
