#[test]
fn review_foreign_generated_source_survives_return_to_host() -> TestResult {
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
    let ReadSpec::Builtin { reader, .. } = &mut guest.reads[0] else { return Err("builtin".into()); };
    *reader = nepl3_reader::builtin::BuiltinReader::Text;
    guest.forms[0].fields[0].read = ReadSpecId(0);
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
    let input = "pair :wrap @\"x\\n\" ~y trailing";
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
    let mut session = ParseSession::new(
        "foreign-session".into(),
        &resolved,
        &environments,
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut operation = budget();
    let mut admission = SourceAdmission::default();
    let mut reply = session
        .read(
            ParseRequest {
                snapshot: &source,
                start: 0,
                limit: input.len() as u64,
                final_input: true,
                entry: &entry,
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
            &sources,
            &mut operation,
            &mut admission,
        )
        .map_err(|e| format!("{e:?}"))?;
    while let ParseOutcome::Reserve { continuation, .. } = &reply.outcome {
        reply = session.reserve(continuation, &nepl3_core::source::SourceReservation { source_id: SourceId("guest-decoded".into()), revision: 0, uri: "memory:guest-decoded".into() }, &sources, &mut operation, &mut admission).map_err(|e|format!("reserve {e:?}"))?;
    }
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err(format!("{reply:?}").into());
    };
    assert_eq!(cursor, 20);
    let FieldValue::Foreign(guest) = &tree.bundle.nodes[0].fields[0] else {
        return Err("foreign field".into());
    };
    assert_eq!(guest.bundle.nodes[0].kind, "Form:Wrap");
    assert_eq!(guest.bundle.tokens[1].payload, NdfValue::Text("x\n".into()));
    assert_eq!(tree.bundle.tokens[1].payload, NdfValue::Text("y".into()));
    assert_eq!(tree.bundle.tokens[1].head.start(), 19);
    let guest_context = tree
        .contexts
        .iter()
        .find(|v| !v.path.is_empty())
        .ok_or("guest context")?;
    assert_eq!(guest_context.nodes[0].entry.mode, "Alt");
    assert_eq!(guest_context.nodes[1].entry.mode, "Code");
    assert_eq!(guest_context.nodes[1].entry.alias, "Guest");
    assert_eq!(reply.report.usage.source_bytes, input.len() as u64 + 2);
    assert!(guest.bundle.sources.iter().any(|s|s.identity().source.0 == "guest-decoded" && s.text() == "x\n"));
    assert!(tree.bundle.sources.iter().any(|s|s.identity().source.0 == "guest-decoded"));
    assert_eq!(guest.bundle.source_maps.len(), 2);
    assert_eq!(tree.bundle.source_maps, guest.bundle.source_maps);
    Ok(())
}
