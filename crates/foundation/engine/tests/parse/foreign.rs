use super::support::*;
use nepl3_core::{
    schema::TypeDescriptor,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry, FieldValue},
    value::{KindRef, NdfValue},
};
use nepl3_engine::{package::*, parse::*, profile::*};
use nepl3_reader::{
    model::ReaderContext,
    plan::{ReaderExpr, ReaderId, ReaderRule},
    tokenizer::{ReaderMode, SkipRule, TokenReader},
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
fn mode(package: &mut LanguagePackage, name: &str, marker: &str) {
    let root = ReaderId(package.reader.expressions.len() as u64);
    package
        .reader
        .expressions
        .push(ReaderExpr::Literal(marker.into()));
    package.reader.rules.push(ReaderRule {
        name: name.into(),
        root,
        output: TypeDescriptor::Unit,
    });
    let mut skip = package.modes[0].skip.clone();
    skip.push(SkipRule {
        reader: TokenReader::Rule(name.into()),
    });
    package.modes.push(ReaderMode {
        name: name.into(),
        skip,
        take: package.modes[0].take.clone(),
    });
}
fn run_foreign(input: &str, accept_names: bool) -> Result<ParseReply, Box<dyn std::error::Error>> {
    run_foreign_inspect(input, accept_names, |_, _, _| Ok(()))
}
fn run_foreign_inspect(
    input: &str,
    accept_names: bool,
    inspect: impl FnOnce(&ParseReply, &ResolvedParseProfile<'_>, &SourceSnapshot) -> TestResult,
) -> Result<ParseReply, Box<dyn std::error::Error>> {
    with_foreign_context(
        input,
        accept_names,
        |resolved, environments, sources, source, entry, setup| {
            let mut session =
                ParseSession::new("foreign-session".into(), resolved, environments, setup)
                    .map_err(|e| format!("{e:?}"))?;
            let mut operation = budget();
            let mut admission = SourceAdmission::default();
            let reply = session
                .read(
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: input.len() as u64,
                        final_input: true,
                        entry,
                        states: &[
                            LanguageReaderState {
                                alias: "Host".into(),
                                state: NdfValue::Unit,
                            },
                            LanguageReaderState {
                                alias: "Guest".into(),
                                state: NdfValue::Unit,
                            },
                        ],
                    },
                    sources,
                    &mut operation,
                    &mut admission,
                )
                .map_err(|e| format!("{e:?}"))?;
            inspect(&reply, resolved, source)?;
            Ok(reply)
        },
    )
}

fn with_foreign_context<T>(
    input: &str,
    accept_names: bool,
    f: impl FnOnce(
        &ResolvedParseProfile<'_>,
        &ParseEnvironmentSet<'_>,
        &SourceStore,
        &SourceSnapshot,
        &EntryContext,
        &mut nepl3_core::budget::Budget,
    ) -> Result<T, Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    let (mut host, registry) = fixture()?;
    let mut guest = host.clone();
    let kind = |name: &str| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: host.schema.clone(),
            local_kind: registry
                .kind_id(&host.schema, name)
                .map_err(|e| format!("{e:?}"))?,
        })
    };
    let pair = kind("Form:Pair")?;
    let wrap = kind("Form:Wrap")?;
    // Same Code name, distinct readers: host ~, guest @; only guest root Alt accepts :.
    mode(&mut host, "HostCode", "~");
    host.modes.remove(0);
    host.modes[0].name = "Code".into();
    mode(&mut guest, "GuestCode", "@");
    mode(&mut guest, "Alt", ":");
    guest.modes.remove(0);
    guest.modes[0].name = "Code".into();
    host.reads.push(ReadSpec::Foreign {
        alias: "Guest".into(),
        category: "Expr".into(),
    });
    host.reads.push(ReadSpec::WithMode {
        mode: "Alt".into(),
        read: ReadSpecId(2),
    });
    host.bindings = vec![
        Binding::None,
        Binding::Visit("first".into()),
        Binding::Visit("second".into()),
        Binding::Group(vec![BindingId(1), BindingId(2)]),
    ];
    guest.bindings = vec![Binding::None, Binding::Visit("value".into())];
    host.leaves[0].binding = BindingId(0);
    guest.leaves[0].binding = BindingId(0);
    if !accept_names {
        guest.leaves.clear();
    }
    host.forms = vec![Form {
        category: "Expr".into(),
        kind: pair,
        spelling: "pair".into(),
        fields: vec![
            FieldSpec {
                name: "first".into(),
                read: ReadSpecId(3),
            },
            FieldSpec {
                name: "second".into(),
                read: ReadSpecId(1),
            },
        ],
        binding: BindingId(3),
        selection_rules: vec![],
        styles: vec![],
    }];
    guest.forms = vec![Form {
        category: "Expr".into(),
        kind: wrap,
        spelling: "wrap".into(),
        fields: vec![FieldSpec {
            name: "value".into(),
            read: ReadSpecId(1),
        }],
        binding: BindingId(1),
        selection_rules: vec![],
        styles: vec![],
    }];
    let mut setup = budget();
    let identity = |p: &LanguagePackage| -> Result<PackageIdentity, String> {
        p.check(&registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .semantic_identity(&mut budget())
            .map_err(|e| format!("{e:?}"))
    };
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let profile = ParseProfile {
        id: "foreign-parse".into(),
        languages: vec![
            LanguageRegistration {
                alias: "Host".into(),
                package: identity(&host)?,
                default_category: "Expr".into(),
            },
            LanguageRegistration {
                alias: "Guest".into(),
                package: identity(&guest)?,
                default_category: "Expr".into(),
            },
        ],
        schemas: vec![
            host.schema.clone(),
            foundation.clone(),
            registry
                .selected("nepl3.reader", 1)
                .ok_or("reader schema")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = [&host, &guest];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut setup,
        )
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("foreign-input".into()),
        0,
        "memory:foreign".into(),
        input.as_bytes().to_vec(),
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut sources = SourceStore::default();
    sources
        .insert(source.clone())
        .map_err(|e| format!("{e:?}"))?;
    let value = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&value, &foundation, &registry, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let raw = ReaderContext {
        schema: host.schema.clone(),
        category: "Expr".into(),
        mode: "Code".into(),
        origins: vec![],
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value,
        },
    };
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let proof = raw
        .check(&mut codec, &sources, &registry, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let environments = ParseEnvironmentSet::prepare(
        &resolved,
        &[
            EnvironmentInput {
                alias: "Host",
                context: &proof,
            },
            EnvironmentInput {
                alias: "Guest",
                context: &proof,
            },
        ],
        &sources,
        &mut codec,
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let entry = resolved
        .entry("Host", None, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    f(
        &resolved,
        &environments,
        &sources,
        &source,
        &entry,
        &mut setup,
    )
}

#[test]
fn foreign_root_mode_is_guest_owned_and_normal_child_and_host_restore_defaults() -> TestResult {
    let input = "pair :wrap @x ~y trailing";
    let reply = run_foreign(input, true)?;
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(cursor, 16);
    let FieldValue::Foreign(guest) = &tree.bundle.nodes[0].fields[0] else {
        return Err("foreign field".into());
    };
    assert_eq!(guest.bundle.nodes[0].kind, "Form:Wrap");
    assert_eq!(guest.bundle.tokens[1].payload, NdfValue::Text("x".into()));
    assert_eq!(tree.bundle.tokens[1].payload, NdfValue::Text("y".into()));
    assert_eq!(tree.bundle.tokens[1].head.start(), 15);
    let guest_context = tree
        .contexts
        .iter()
        .find(|v| !v.path.is_empty())
        .ok_or("guest context")?;
    assert_eq!(guest_context.nodes[0].entry.mode, "Alt");
    assert_eq!(guest_context.nodes[1].entry.mode, "Code");
    assert_eq!(guest_context.nodes[1].entry.alias, "Guest");
    assert_eq!(reply.report.usage.source_bytes, input.len() as u64);
    Ok(())
}

#[test]
fn foreign_root_recovery_preserves_actual_schema_and_expected_guest() -> TestResult {
    for (input, kind) in [
        ("pair unknown", "RecoveryUnparsed"),
        ("pair", "RecoveryMissing"),
    ] {
        let reply = run_foreign(input, false)?;
        let ParseOutcome::Recovered { tree, .. } = reply.outcome else {
            return Err(format!("expected recovered: {reply:?}").into());
        };
        let FieldValue::Foreign(guest) = &tree.bundle.nodes[0].fields[0] else {
            return Err("guest".into());
        };
        let root = guest
            .bundle
            .node(guest.root)
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(guest.schema, root.schema);
        assert_eq!(root.schema.package, "nepl3.engine");
        assert_eq!(root.kind, kind);
        let context = tree
            .contexts
            .iter()
            .find(|c| !c.path.is_empty())
            .ok_or("guest selection")?;
        assert_eq!(context.nodes[0].entry.alias, "Guest");
        assert_eq!(context.nodes[0].entry.category, "Expr");
        assert_eq!(context.nodes[0].entry.mode, "Alt");
        assert_ne!(context.nodes[0].entry.package.schema, root.schema);
        assert!(!reply.report.diagnostics.is_empty());
    }
    Ok(())
}

#[test]
fn expected_read_enters_guest_before_host_and_retains_with_mode_declaration() -> TestResult {
    use nepl3_engine::{
        analysis::{BindingOptions, expected::*},
        portable::analysis,
    };
    run_foreign_inspect("pair", false, |reply, profile, source| {
        let ParseOutcome::Recovered { tree, .. } = &reply.outcome else {
            return Err("recovered".into());
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let input = analysis::prepare(
            "expected-foreign",
            tree,
            BindingOptions,
            budget().limits(),
            profile,
            &mut c,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let query = ExpectedReadRequest {
            key: input.key(),
            source: source.reference(),
            offset: 4,
        };
        let result = expected_read(
            &input,
            &query,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let ExpectedReadOutcome::Complete(Some(value)) = result.outcome else {
            return Err(format!("{result:?}").into());
        };
        assert_eq!(value.path, vec![ExpectedReadStep::Foreign { field: 0 }]);
        assert_eq!(value.expected.alias, "Guest");
        assert_eq!(value.expected.mode, "Alt");
        let ExpectedReadOrigin::Field {
            parent_bundle,
            parent_node,
            field,
            owner,
            declared,
            resolved_read,
            foreign,
        } = value.origin
        else {
            return Err("outer field".into());
        };
        assert_eq!((parent_bundle, parent_node, field), (0, 0, 0));
        assert_eq!(declared, ReadSpecId(3)); // Preserve WithMode, not inner Foreign ReadSpecId(2).
        assert_eq!(resolved_read, None);
        assert!(foreign);
        assert_ne!(owner, value.expected.package);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn declared_alternatives_use_guest_package_after_with_mode_resolution() -> TestResult {
    use nepl3_engine::{
        analysis::{BindingOptions, alternatives::*, expected::ExpectedReadRequest},
        portable::analysis,
    };
    run_foreign_inspect("pair", false, |reply, profile, source| {
        let ParseOutcome::Recovered { tree, .. } = &reply.outcome else {
            return Err("recovered".into());
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let input = analysis::prepare(
            "alternatives-foreign",
            tree,
            BindingOptions,
            budget().limits(),
            profile,
            &mut c,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let result = declared_alternatives(
            &input,
            &ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 4,
            },
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let DeclaredAlternativesOutcome::Complete(Some(value)) = result.outcome else {
            return Err(format!("{result:?}").into());
        };
        assert_eq!(value.read.expected.alias, "Guest");
        assert_eq!(value.read.expected.mode, "Alt");
        let DeclaredReadAlternatives::Category {
            forms,
            leaf_declarations,
            dynamic_fallback_registered,
        } = value.alternatives
        else {
            return Err("category".into());
        };
        assert_eq!(
            forms,
            vec![DeclaredFormAlternative {
                index: 0,
                spelling: "wrap".into()
            }]
        );
        assert_eq!(leaf_declarations, 0);
        assert!(!dynamic_fallback_registered);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn insertion_observation_accepts_engine_recovery_to_guest_schema_transition() -> TestResult {
    use nepl3_core::source::{Digest, TextEdit};
    use nepl3_engine::{
        analysis::{
            BindingOptions,
            expected::{ExpectedReadRequest, ExpectedReadStep},
            insertion::*,
        },
        portable::analysis,
    };
    with_foreign_context(
        "pair",
        true,
        |profile, environments, sources, source, entry, _| {
            let states = [
                LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                },
                LanguageReaderState {
                    alias: "Guest".into(),
                    state: NdfValue::Unit,
                },
            ];
            let text = "pair :wrap @x";
            let next = SourceSnapshot::new(
                source.identity().source.clone(),
                1,
                source.uri().into(),
                text.as_bytes().to_vec(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut next_sources = SourceStore::default();
            next_sources
                .insert(next.clone())
                .map_err(|e| format!("{e:?}"))?;
            let mut old_session = RetainedParseSession::new(
                "foreign-old".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start: 0,
                    limit: 4,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Continue(old) = old_session
                .read(&mut budget(), &mut SourceAdmission::default())
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("old execution".into());
            };
            let mut new_session = RetainedParseSession::new(
                "foreign-new".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: &next,
                    start: 0,
                    limit: text.len() as u64,
                    final_input: true,
                    entry,
                    states: &states,
                },
                &next_sources,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Continue(new) = new_session
                .read(&mut budget(), &mut SourceAdmission::default())
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("new execution".into());
            };
            let first = |tree: &nepl3_engine::recovery::ParseTree| -> Result<nepl3_core::value::SchemaRef, String> {
            let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
            let FieldValue::Foreign(value) = &root.fields[0] else { return Err("foreign first child".into()); };
            Ok(value.schema.clone())
        };
            assert_ne!(
                first(old.execution().tree())?,
                first(new.execution().tree())?
            );
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
            let old_prepared = analysis::prepare(
                "foreign-old",
                old.execution().tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let new_prepared = analysis::prepare(
                "foreign-new",
                new.execution().tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let result = observe(
                InsertionInput {
                    parsed: &old,
                    prepared: &old_prepared,
                },
                InsertionInput {
                    parsed: &new,
                    prepared: &new_prepared,
                },
                &ExpectedReadRequest {
                    key: old_prepared.key(),
                    source: source.reference(),
                    offset: 4,
                },
                &TextEdit {
                    span: source.span(4, 4).map_err(|e| format!("{e:?}"))?,
                    expected_digest: Digest::of(b""),
                    replacement: " :wrap @x".into(),
                },
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("observation: {e:?}"))?;
            assert_eq!(result.path(), &[ExpectedReadStep::Foreign { field: 0 }]);
            assert_eq!((result.cover().start(), result.cover().end()), (6, 13));
            Ok(())
        },
    )
}
