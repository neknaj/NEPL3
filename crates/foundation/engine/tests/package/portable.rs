use super::{TestResult, budget, fixture};
use nepl3_core::{origin::*, schema::*, source::*, syntax::*, value::*, view::*};
use nepl3_engine::{
    package::*,
    portable::{self, PortableError},
    profile::*,
    recovery::*,
    selection::*,
    tree::TreeError,
};
use nepl3_wire::foundation::FoundationCodec;
#[path = "portable/facts.rs"]
mod facts;

fn profile(package: &LanguagePackage, registry: &SchemaRegistry) -> Result<ParseProfile, String> {
    let identity = package
        .check(registry, &mut budget())
        .map_err(err)?
        .semantic_identity(&mut budget())
        .map_err(err)?;
    Ok(ParseProfile {
        id: "portable-tree".into(),
        languages: vec![
            LanguageRegistration {
                alias: "Host".into(),
                package: identity.clone(),
                default_category: "Expr".into(),
            },
            LanguageRegistration {
                alias: "Guest".into(),
                package: identity,
                default_category: "Expr".into(),
            },
        ],
        schemas: vec![
            package.schema.clone(),
            registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn tree(
    package: &LanguagePackage,
    registry: &SchemaRegistry,
    profile: &ResolvedParseProfile<'_>,
) -> Result<ParseTree, String> {
    let host = SourceSnapshot::new(
        SourceId("host".into()),
        0,
        "memory:host".into(),
        b"cons nil".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let guest = SourceSnapshot::new(
        SourceId("guest".into()),
        0,
        "memory:guest".into(),
        b"let x y".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let kind = package.leaves[0].token_kind.clone();
    let token = |source: &SourceSnapshot, start, end, text: &str| -> Result<Token, String> {
        Ok(Token {
            kind: kind.clone(),
            head: source.span(start, end).map_err(err)?,
            payload: NdfValue::Text(text.into()),
            views: ViewBundle {
                elements: vec![],
                roots: vec![],
            },
            leading_trivia: vec![],
        })
    };
    let tokens = vec![
        token(&guest, 0, 3, "let")?,
        token(&guest, 4, 5, "x")?,
        token(&guest, 6, 7, "y")?,
    ];
    let node = |name: &str, token: usize, fields: Vec<FieldValue>, tokens: &[Token]| SyntaxNode {
        schema: package.schema.clone(),
        kind: name.into(),
        fields,
        head: Some(tokens[token].head.clone()),
        cover: Some(tokens[token].head.clone()),
        origin: OriginId(token as u64),
        token: Some(TokenRef(token as u64)),
    };
    let mut parent = node(
        "Form:Let",
        0,
        vec![FieldValue::Child(NodeRef(0)), FieldValue::Child(NodeRef(1))],
        &tokens,
    );
    parent.cover = Some(guest.span(0, 7).map_err(err)?);
    let guest_bundle = SyntaxBundle {
        sources: vec![guest],
        nodes: vec![
            node("Builtin:Name", 1, vec![], &tokens),
            node("Leaf:Name", 2, vec![], &tokens),
            parent,
        ],
        origins: tokens
            .iter()
            .map(|t| Origin::Direct(t.head.clone()))
            .collect(),
        root: NodeRef(2),
        environments: vec![],
        tokens,
        source_maps: vec![],
    };
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = nepl3_wire::environment::environment_digest(
        &environment,
        registry
            .selected("nepl3.foundation", 1)
            .ok_or("foundation")?,
        registry,
        &mut budget(),
    )
    .map_err(err)?;
    let tokens = vec![token(&host, 0, 4, "cons")?, token(&host, 5, 8, "nil")?];
    let mut cons = node(
        "List:Cons",
        0,
        vec![
            FieldValue::Foreign(Box::new(ForeignSyntax {
                schema: package.schema.clone(),
                category: "Expr".into(),
                root: NodeRef(2),
                bundle: guest_bundle,
                environment: EnvironmentRef { id: 9, digest },
            })),
            FieldValue::Child(NodeRef(0)),
        ],
        &tokens,
    );
    cons.cover = Some(host.span(0, 8).map_err(err)?);
    let host_bundle = SyntaxBundle {
        sources: vec![host],
        nodes: vec![node("List:Nil", 1, vec![], &tokens), cons],
        origins: tokens
            .iter()
            .map(|t| Origin::Direct(t.head.clone()))
            .collect(),
        root: NodeRef(1),
        environments: vec![EnvironmentEntry {
            id: 9,
            digest,
            value: environment,
        }],
        tokens,
        source_maps: vec![],
    };
    let host_entry = profile.entry("Host", None, &mut budget()).map_err(err)?;
    let guest_entry = profile.entry("Guest", None, &mut budget()).map_err(err)?;
    let execution = profile
        .execution_digest("Host", &mut budget())
        .map_err(err)?;
    let selection = |node, entry: &EntryContext, shape| NodeSelection {
        node: NodeRef(node),
        entry: entry.clone(),
        execution_digest: execution,
        shape,
    };
    Ok(ParseTree {
        profile_digest: profile.digest(),
        bundle: host_bundle,
        recovery: vec![],
        contexts: vec![
            BundleContext {
                path: vec![ForeignStep {
                    node: NodeRef(1),
                    field: "head".into(),
                }],
                nodes: vec![
                    selection(1, &guest_entry, ShapeSelection::Leaf { index: 0 }),
                    selection(2, &guest_entry, ShapeSelection::Form { index: 0 }),
                    selection(
                        0,
                        &guest_entry,
                        ShapeSelection::Builtin {
                            read: ReadSpecId(0),
                        },
                    ),
                ],
            },
            BundleContext {
                path: vec![],
                nodes: vec![
                    selection(
                        0,
                        &host_entry,
                        ShapeSelection::List {
                            read: ReadSpecId(3),
                            cons: false,
                        },
                    ),
                    selection(
                        1,
                        &host_entry,
                        ShapeSelection::List {
                            read: ReadSpecId(3),
                            cons: true,
                        },
                    ),
                ],
            },
        ],
    })
}
fn extended() -> Result<(LanguagePackage, SchemaRegistry), String> {
    let (mut package, registry) = fixture()?;
    let kind = |name: &str| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: package.schema.clone(),
            local_kind: registry.kind_id(&package.schema, name).map_err(err)?,
        })
    };
    let cons = kind("List:Cons")?;
    let nil = kind("List:Nil")?;
    package.reads.push(ReadSpec::Foreign {
        alias: "Guest".into(),
        category: "Expr".into(),
    });
    package.reads.push(ReadSpec::ListOf {
        element: ReadSpecId(2),
        cons,
        nil,
    });
    Ok((package, registry))
}
#[test]
fn typed_tree_roundtrip_remaps_every_local_owner_and_rejects_stale_wrapper_indices() -> TestResult {
    let (package, registry) = extended()?;
    let profile = profile(&package, &registry)?;
    let packages = [&package];
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &[],
        resources: &[],
    };
    let resolved = profile
        .resolve(&catalog, &registry, &mut budget())
        .map_err(err)?;
    let tree = tree(&package, &registry, &resolved)?;
    tree.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
        .map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    let mut codec = FoundationCodec::new(&registry, &empty, &mut admission).map_err(err)?;
    let value = portable::tree::to_value(&tree, &resolved, &mut codec, &mut b).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
    let decoded =
        portable::tree::from_value(&received, &resolved, &mut codec, &mut b).map_err(err)?;
    assert_eq!(decoded.bundle.root, NodeRef(0));
    let FieldValue::Foreign(guest) = &decoded.bundle.nodes[0].fields[0] else {
        return Err("foreign".into());
    };
    assert_eq!(guest.root, NodeRef(0));
    assert_eq!(guest.bundle.root, NodeRef(0));
    assert_eq!(
        guest.bundle.nodes[0].fields[0],
        FieldValue::Child(NodeRef(1))
    );
    assert_eq!(decoded.contexts[1].path[0].node, NodeRef(0));
    assert_eq!(decoded.contexts[1].nodes[0].node, NodeRef(0));
    assert_eq!(guest.environment.id, 9);
    assert_eq!(
        guest.bundle.tokens,
        match &tree.bundle.nodes[1].fields[0] {
            FieldValue::Foreign(g) => g.bundle.tokens.clone(),
            _ => return Err("foreign".into()),
        }
    );
    assert_eq!(
        nepl3_wire::encode(
            &portable::tree::to_value(&decoded, &resolved, &mut codec, &mut b).map_err(err)?,
            &mut b
        )
        .map_err(err)?,
        bytes
    );
    assert_eq!(b.usage().source_bytes, 15);
    // The typed value boundary preserves each shared stop reason; decode may
    // consume structural work before it reaches source admission.
    use nepl3_core::budget::{Budget, StopReason};
    for reason in [
        StopReason::SourceLimit,
        StopReason::WorkLimit,
        StopReason::DepthLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        for encode in [true, false] {
            let mut limits = budget().limits();
            match reason {
                StopReason::SourceLimit => limits.source_bytes = 0,
                StopReason::WorkLimit => limits.work = 0,
                StopReason::DepthLimit => limits.depth = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                _ => {}
            }
            let mut limited = Budget::new(limits);
            if reason == StopReason::Cancelled {
                limited.cancel();
            }
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&registry, &empty, &mut admission).map_err(err)?;
            let result = if encode {
                portable::tree::to_value(&tree, &resolved, &mut codec, &mut limited).map(|_| ())
            } else {
                portable::tree::from_value(&received, &resolved, &mut codec, &mut limited)
                    .map(|_| ())
            };
            assert!(
                matches!(result, Err(PortableError::Stopped(r)) if r == reason),
                "{reason:?} encode={encode}: {result:?}"
            );
            assert_eq!(limited.poll(), Err(reason));
        }
    }
    let mut bad = received.clone();
    fn fields(v: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
        match v {
            NdfValue::Record(r) => Ok(&mut r.fields),
            _ => Err("record".into()),
        }
    }
    fn list(v: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
        match v {
            NdfValue::List(r) => Ok(r),
            _ => Err("list".into()),
        }
    }
    let contexts = list(&mut fields(&mut bad)?[3])?;
    let path = list(&mut fields(&mut contexts[1])?[0])?;
    fields(&mut fields(&mut path[0])?[0])?[0] = NdfValue::U64(1);
    assert!(portable::tree::from_value(&bad, &resolved, &mut codec, &mut b).is_err());
    let mut forged = decoded.clone();
    forged.contexts[1].nodes[0].execution_digest = Digest::of(b"wrong");
    assert!(matches!(
        portable::tree::to_value(&forged, &resolved, &mut codec, &mut b),
        Err(PortableError::Tree(TreeError::ExecutionIdentity))
    ));
    Ok(())
}
#[test]
fn typed_tree_recovery_uses_the_guest_source_table_and_canonical_node_reference() -> TestResult {
    let (package, registry) = extended()?;
    let profile = profile(&package, &registry)?;
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(err)?;
    let mut tree = tree(&package, &registry, &resolved)?;
    let FieldValue::Foreign(guest) = &mut tree.bundle.nodes[1].fields[0] else {
        return Err("foreign".into());
    };
    let anchor = guest.bundle.sources[0].span(6, 6).map_err(err)?;
    guest.bundle.nodes[1].schema = registry
        .selected("nepl3.engine", 1)
        .ok_or("engine")?
        .clone();
    guest.bundle.nodes[1].kind = "RecoveryMissing".into();
    guest.bundle.nodes[1].head = None;
    guest.bundle.nodes[1].cover = Some(anchor.clone());
    guest.bundle.nodes[1].token = None;
    tree.contexts[0].nodes[0].shape = ShapeSelection::Recovery;
    tree.recovery.push(BundleRecovery {
        path: tree.contexts[0].path.clone(),
        entries: vec![RecoveryEntry {
            node: NodeRef(1),
            kind: RecoveryKind::Missing {
                expected: tree.contexts[0].nodes[0].entry.clone(),
                anchor,
            },
        }],
    });
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut b = budget();
    let mut codec = FoundationCodec::new(&registry, &empty, &mut admission).map_err(err)?;
    let value = portable::tree::to_value(&tree, &resolved, &mut codec, &mut b).map_err(err)?;
    let decoded = portable::tree::from_value(&value, &resolved, &mut codec, &mut b).map_err(err)?;
    assert_eq!(decoded.recovery[0].path[0].node, NodeRef(0));
    assert_eq!(decoded.recovery[0].entries[0].node, NodeRef(2));
    assert!(
        matches!(&decoded.recovery[0].entries[0].kind,RecoveryKind::Missing{anchor,..} if anchor.snapshot_ref().source.0=="guest")
    );
    assert_eq!(
        portable::tree::to_value(&decoded, &resolved, &mut codec, &mut b).map_err(err)?,
        value
    );
    Ok(())
}
