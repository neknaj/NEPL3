use super::*;
use nepl3_engine::{
    analysis::{
        BindingOptions,
        expected::ExpectedReadRequest,
        insertion::{
            InsertionInput,
            name::{self, NameSpelling},
        },
        probe::{
            candidates,
            read::{self, ReadOutcome},
        },
    },
    portable::analysis as keyed,
};

#[test]
fn name_spelling_preserves_ambiguity_and_supports_same_source_foreign_paths() -> Result<(), String>
{
    let compiled = super::missing_probe::named_lambda()?;
    for (input, spelling, index, ambiguous, foreign) in [
        ("let outer 1 guest lambda inner", "inner", 0, false, true),
        (
            "recursive cons define a 1 cons define a 2 nil lambda x",
            "a",
            1,
            true,
            false,
        ),
        ("apply lambda x", "x", 0, false, false),
    ] {
        with_source_profile(&compiled, None, input, |source, profile, b, ledger| {
            let registry = profile.registry();
            let package = profile.language("B", b).map_err(err)?;
            let foundation = registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?;
            let mut store = SourceStore::default();
            store.insert(source.clone()).map_err(err)?;
            let environment = Environment {
                bindings: vec![],
                resources: vec![],
            };
            let raw = ReaderContext {
                schema: package.schema.clone(),
                category: "Expr".into(),
                mode: "Code".into(),
                origins: vec![],
                environment: EnvironmentEntry {
                    id: 0,
                    digest: environment_digest(&environment, foundation, registry, b)
                        .map_err(err)?,
                    value: environment,
                },
            };
            let environments = {
                let mut codec = FoundationCodec::new(registry, &store, ledger).map_err(err)?;
                let proof = raw.check(&mut codec, &store, registry, b).map_err(err)?;
                ParseEnvironmentSet::prepare(
                    profile,
                    &[EnvironmentInput {
                        alias: "B",
                        context: &proof,
                    }],
                    &store,
                    &mut codec,
                    b,
                )
                .map_err(err)?
            };
            let entry = profile.entry("B", None, b).map_err(err)?;
            let states = [LanguageReaderState {
                alias: "B".into(),
                state: NdfValue::Unit,
            }];
            let mut parser = RetainedParseSession::new(
                "name-foreign-old".into(),
                profile,
                &environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: input.len() as u64,
                    final_input: true,
                    entry: &entry,
                    states: &states,
                },
                &store,
                b,
            )
            .map_err(err)?;
            let old = drive(&mut parser, registry, "old", b, ledger)?;
            let empty = SourceStore::default();
            let prepared = {
                let mut codec = FoundationCodec::new(registry, &empty, ledger).map_err(err)?;
                keyed::prepare(
                    "name-foreign-old",
                    old.execution().tree(),
                    BindingOptions,
                    b.limits(),
                    profile,
                    &mut codec,
                    b,
                )
                .map_err(err)?
            };
            let bound = prepared.probe_missing_reference(b, ledger).map_err(err)?;
            let request = ExpectedReadRequest {
                key: prepared.key(),
                source: source.reference(),
                offset: input.len() as u64,
            };
            let correlation =
                read::correlate(&bound, &prepared, &request, b, ledger).map_err(err)?;
            let ReadOutcome::Hit(read) = correlation.outcome() else {
                return Err("read correspondence".into());
            };
            let candidates = candidates::names(
                &bound,
                &candidates::ProbeCandidateRequest {
                    key: prepared.key(),
                    source: &request.source,
                    offset: request.offset,
                    prefix: "",
                },
                b,
                ledger,
            )
            .map_err(err)?;
            let selected = name::select(&candidates, read, index, b).map_err(err)?;
            assert_eq!(selected.candidate().name, spelling);
            assert_eq!(
                matches!(
                    selected.candidate().resolution,
                    ReferenceResolution::Ambiguous(_)
                ),
                ambiguous
            );
            let original_resolution = selected.candidate().resolution.clone();
            let proposal = name::prepare(
                InsertionInput {
                    parsed: &old,
                    prepared: &prepared,
                },
                &request,
                selected,
                NameSpelling {
                    before: " ",
                    spelling,
                    after: "",
                },
                b,
                ledger,
            )
            .map_err(err)?;
            let draft = proposal.draft();
            let mut parser = RetainedParseSession::new(
                "name-foreign-new".into(),
                profile,
                &environments,
                ParseRequest {
                    snapshot: draft.snapshot(),
                    start: 0,
                    limit: draft.limit(),
                    final_input: true,
                    entry: &entry,
                    states: &states,
                },
                draft.sources(),
                b,
            )
            .map_err(err)?;
            let candidate = drive(&mut parser, registry, "new", b, ledger)?;
            {
                let mut codec = FoundationCodec::new(registry, &empty, ledger).map_err(err)?;
                let checked = name::check(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared,
                    },
                    &candidate,
                    &proposal,
                    &request,
                    "name-foreign-new",
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                assert_eq!(checked.choice().candidate().resolution, original_resolution);
                if input == "apply lambda x" {
                    assert!(matches!(
                        nepl3_engine::analysis::insertion::whole::check(checked.checked(), b),
                        Err(
                            nepl3_engine::analysis::insertion::whole::WholeInsertionError::Input(
                                nepl3_engine::parse::whole::WholeInputError::Recovered
                            )
                        )
                    ));
                }

                assert_eq!(
                    checked.checked().path().iter().any(|s| matches!(
                        s,
                        nepl3_engine::analysis::expected::ExpectedReadStep::Foreign { .. }
                    )),
                    foreign
                );
            }
            // Identical bytes from another private draft do not confer parser provenance.
            let selected = name::select(&candidates, read, index, b).map_err(err)?;
            let second = name::prepare(
                InsertionInput {
                    parsed: &old,
                    prepared: &prepared,
                },
                &request,
                selected,
                NameSpelling {
                    before: " ",
                    spelling,
                    after: "",
                },
                b,
                ledger,
            )
            .map_err(err)?;
            assert_eq!(second.draft().snapshot().text(), draft.snapshot().text());
            let mut codec = FoundationCodec::new(registry, &empty, ledger).map_err(err)?;
            assert!(matches!(
                name::check(
                    InsertionInput {
                        parsed: &old,
                        prepared: &prepared
                    },
                    &candidate,
                    &second,
                    &request,
                    "name-foreign-new",
                    &mut codec,
                    b
                ),
                Err(name::CheckError::Checked(
                    nepl3_engine::analysis::insertion::checked::CheckError::DraftMismatch
                ))
            ));
            assert_eq!(source.text(), input);
            Ok(())
        })?;
    }
    Ok(())
}

fn drive<'a>(
    parser: &mut RetainedParseSession<'a>,
    registry: &nepl3_core::schema::SchemaRegistry,
    tag: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<nepl3_engine::parse::RetainedParse<'a>, String> {
    let mut result = parser.read(b, a).map_err(err)?;
    let mut reservations = 0;
    loop {
        match result {
            RetainedParseExecution::Continue(parsed) => return Ok(parsed),
            RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Await { call, continuation },
                ..
            }) => {
                let ProviderCall::Read {
                    operation,
                    request,
                    depth_base,
                    ..
                } = call.as_ref()
                else {
                    return Err("read provider".into());
                };
                let mut declared = SourceStore::default();
                for source in &request.sources {
                    declared.insert(source.clone()).map_err(err)?;
                }
                let snapshot = declared
                    .resolve(&request.snapshot)
                    .ok_or("provider source")?;
                let terminal = b
                    .with_depth_at_least(*depth_base, |b| {
                        let mut codec = FoundationCodec::new(registry, &declared, a)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        let checked = request
                            .context
                            .check(&mut codec, &declared, registry, b)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        nepl3_reader::builtin::provider::read(
                            operation,
                            ReadRequest {
                                snapshot,
                                start: request.start,
                                limit: request.limit,
                                final_input: request.final_input,
                                context: &checked,
                                state: &request.state,
                            },
                            registry,
                            &declared,
                            b,
                            a,
                        )
                    })
                    .map_err(err)?;
                result = parser
                    .resume(&continuation, ProviderReply::Read(Box::new(terminal)), b, a)
                    .map_err(err)?;
            }
            RetainedParseExecution::Break(ParseReply {
                outcome: ParseOutcome::Reserve { continuation, .. },
                ..
            }) => {
                reservations += 1;
                let reservation = nepl3_core::source::SourceReservation {
                    source_id: SourceId(format!("name-{tag}-{reservations}")),
                    revision: 0,
                    uri: format!("memory:name-{tag}-{reservations}"),
                };
                result = parser
                    .reserve(&continuation, &reservation, b, a)
                    .map_err(err)?;
            }
            RetainedParseExecution::Break(reply) => return Err(err(reply)),
        }
    }
}
