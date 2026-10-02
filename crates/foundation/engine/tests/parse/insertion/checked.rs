use super::*;
use nepl3_engine::analysis::insertion::{checked, draft};

#[test]
fn caller_owned_provider_continuations_can_produce_a_checked_edit() -> TestResult {
    super::super::retained::with_options(
        "",
        true,
        false,
        |profile, environments, sources, source, entry| {
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Bool(false),
            }];
            let mut b = budget();
            let mut ledger = SourceAdmission::default();
            let mut old_session = RetainedParseSession::new(
                "checked-old".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 0,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let host = || super::super::retained::StatefulHost {
                inner: super::super::host::Host {
                    action: super::super::host::Action::Serve,
                    calls: 0,
                    minimum_depth: 0,
                },
            };
            let reply = old_session
                .read_with_host(&mut b, &mut ledger, &mut host())
                .map_err(|e| format!("{e:?}"))?;
            assert!(reply.host_error.is_none());
            let RetainedParseExecution::Continue(old) = reply.execution else {
                return Err("original proof".into());
            };
            let empty = SourceStore::default();
            let prepared = {
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                    .map_err(|e| format!("{e:?}"))?;
                analysis::prepare(
                    "checked-old",
                    old.execution().tree(),
                    BindingOptions,
                    b.limits(),
                    profile,
                    &mut codec,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?
            };
            let query = ExpectedReadRequest {
                key: prepared.key(),
                source: source.reference(),
                offset: 0,
            };
            let draft = draft::prepare(
                InsertionInput {
                    parsed: &old,
                    prepared: &prepared,
                },
                &query,
                "let x x",
                &mut b,
                &mut ledger,
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut session = RetainedParseSession::new(
                "checked-new".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: draft.snapshot(),
                    start: 0,
                    limit: draft.limit(),
                    final_input: true,
                    entry,
                    states: &states,
                },
                draft.sources(),
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut result = session
                .read(&mut b, &mut ledger)
                .map_err(|e| format!("{e:?}"))?;
            let mut awaits = 0;
            let mut host = host();
            let candidate = loop {
                match result {
                    RetainedParseExecution::Continue(candidate) => break candidate,
                    RetainedParseExecution::Break(ParseReply {
                        outcome: ParseOutcome::Await { call, continuation },
                        ..
                    }) => {
                        awaits += 1;
                        let nepl3_reader::model::ProviderCall::Read { depth_base, .. } =
                            call.as_ref()
                        else {
                            return Err("read callback".into());
                        };
                        let requirement = profile.profile().providers.first().ok_or("provider")?;
                        let reply = b
                            .with_depth_at_least(*depth_base, |b| {
                                host.provider(&call, requirement, b, &mut ledger)?
                                    .ok_or(ParseError::Context)
                            })
                            .map_err(|e| format!("{e:?}"))?;
                        result = session
                            .resume(&continuation, reply, &mut b, &mut ledger)
                            .map_err(|e| format!("{e:?}"))?;
                    }
                    _ => return Err("unexpected continuation outcome".into()),
                }
            };
            assert!(awaits > 0);
            assert_eq!(candidate.execution().kind(), ExecutionKind::Complete);
            let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                .map_err(|e| format!("{e:?}"))?;
            let checked = checked::check(
                InsertionInput {
                    parsed: &old,
                    prepared: &prepared,
                },
                &candidate,
                &draft,
                &query,
                "checked-candidate",
                &mut codec,
                &mut b,
            )
            .map_err(|e| format!("{e:?}"))?;
            assert_eq!(checked.edit().replacement, "let x x");
            assert!(core::ptr::eq(checked.candidate(), &candidate));
            assert_eq!(checked.report().usage, b.usage());
            assert_eq!(sources.snapshots().len(), 1);
            assert_eq!(source.text(), "");
            Ok(())
        },
    )
}
