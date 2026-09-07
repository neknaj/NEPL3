use nepl3_core::{
    budget::{Budget, Limits},
    origin::{Origin, OriginId},
    schema::{SchemaRegistry, foundation},
    source::{SourceAdmission, SourceId, SourceSnapshot},
    syntax::*,
    value::{KindRef, NdfScalar, NdfValue, SchemaRef},
    view::*,
};
use nepl3_wire::{WireError, decode, encode, environment::environment_digest, syntax::*};
#[path = "syntax/foreign.rs"]
mod foreign;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn fixture() -> Result<(SchemaRef, SchemaRegistry, SyntaxBundle), String> {
    let mut budget = budget();
    let descriptor = foundation::descriptor(&mut budget).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("doc".into()),
        1,
        "memory:syntax".into(),
        b"ab".to_vec(),
        &mut budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = |a, b| source.span(a, b).map_err(|e| format!("{e:?}"));
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: registry
            .kind_id(&schema, "Token")
            .map_err(|e| format!("{e:?}"))?,
    };
    let token = |a, b, payload| -> Result<Token, String> {
        Ok(Token {
            kind: kind.clone(),
            head: span(a, b)?,
            payload,
            views: ViewBundle {
                roots: vec![ViewRef(0)],
                elements: vec![ViewElement {
                    kind: kind.clone(),
                    span: span(a, b)?,
                    fields: vec![],
                    roles: vec![],
                    relations: vec![],
                }],
            },
            leading_trivia: vec![],
        })
    };
    // This is a shared graph fixture, not a claim that foundation Token is a language form.
    let guest = SyntaxBundle {
        source_maps: vec![],
        sources: vec![source.clone()],
        nodes: vec![SyntaxNode {
            schema: schema.clone(),
            kind: "Token".into(),
            fields: vec![FieldValue::Atom(NdfScalar::Text("guest".into()))],
            head: Some(span(1, 2)?),
            cover: Some(span(1, 2)?),
            origin: OriginId(0),
            token: Some(TokenRef(0)),
        }],
        origins: vec![Origin::Direct(span(1, 2)?)],
        root: NodeRef(0),
        environments: vec![],
        tokens: vec![token(
            1,
            2,
            NdfValue::List(vec![NdfValue::Text("guest payload".into())]),
        )?],
    };
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&environment, &schema, &registry, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let bundle = SyntaxBundle {
        source_maps: vec![],
        sources: vec![source.clone()],
        nodes: vec![SyntaxNode {
            schema: schema.clone(),
            kind: "Token".into(),
            fields: vec![FieldValue::Foreign(Box::new(ForeignSyntax {
                schema: schema.clone(),
                category: "fixture".into(),
                root: NodeRef(0),
                bundle: guest,
                environment: EnvironmentRef { id: 0, digest },
            }))],
            head: Some(span(0, 1)?),
            cover: Some(span(0, 2)?),
            origin: OriginId(0),
            token: Some(TokenRef(0)),
        }],
        origins: vec![Origin::Direct(span(0, 2)?)],
        root: NodeRef(0),
        environments: vec![EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        }],
        tokens: vec![token(
            0,
            1,
            NdfValue::List(vec![NdfValue::Text("host payload".into())]),
        )?],
    };
    Ok((schema, registry, bundle))
}
#[test]
fn foreign_bundle_roundtrip_keeps_local_token_view_origin_ids_and_payloads() -> TestResult {
    let (schema, registry, bundle) = fixture()?;
    let mut budget = budget();
    let mut admission = SourceAdmission::default();
    let bytes = encode_syntax(&bundle, &schema, &registry, &mut admission, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let decoded = decode_syntax(&bytes, &schema, &registry, &mut admission, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, bundle);
    assert_eq!(budget.usage().source_bytes, 2);
    let FieldValue::Foreign(guest) = &decoded.nodes[0].fields[0] else {
        return Err("foreign fixture".into());
    };
    assert_eq!(guest.bundle.nodes[0].token, Some(TokenRef(0)));
    assert_eq!(decoded.nodes[0].token, Some(TokenRef(0)));
    assert_ne!(guest.bundle.tokens[0].payload, decoded.tokens[0].payload);
    assert_eq!(decoded.nodes[0].fields.len(), 1); // Internal view is not an outer argument.
    Ok(())
}
#[test]
fn foreign_bundle_rejects_forged_environment_digest_token_and_origin_references() -> TestResult {
    let (schema, registry, bundle) = fixture()?;
    let bytes = encode_syntax(
        &bundle,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    if let NdfValue::Record(root) = &mut value
        && let NdfValue::List(entries) = &mut root.fields[4]
        && let NdfValue::Record(entry) = &mut entries[0]
    {
        entry.fields[1] = NdfValue::Bytes(vec![0; 32]);
    }
    let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_syntax(
            &bad,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::Syntax(SyntaxError::Environment))
    ));
    let mut bad = bundle.clone();
    bad.nodes[0].token = Some(TokenRef(1));
    assert!(
        encode_syntax(
            &bad,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    let mut bad = bundle;
    let FieldValue::Foreign(guest) = &mut bad.nodes[0].fields[0] else {
        return Err("foreign fixture".into());
    };
    guest.bundle.nodes[0].origin = OriginId(1);
    assert!(
        encode_syntax(
            &bad,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn native_arena_order_is_canonicalized_and_unreachable_nodes_are_rejected() -> TestResult {
    let (schema, registry, mut canonical) = fixture()?;
    let FieldValue::Foreign(guest) = &canonical.nodes[0].fields[0] else {
        return Err("guest fixture".into());
    };
    let mut child = guest.bundle.nodes[0].clone();
    child.token = Some(TokenRef(1));
    canonical.tokens.push(guest.bundle.tokens[0].clone());
    canonical.nodes.push(child);
    canonical.nodes[0]
        .fields
        .push(FieldValue::Child(NodeRef(1)));
    let mut shuffled = canonical.clone();
    shuffled.nodes.swap(0, 1);
    shuffled.root = NodeRef(1);
    shuffled.nodes[1].fields[1] = FieldValue::Child(NodeRef(0));
    let encode_bundle = |bundle: &SyntaxBundle| {
        encode_syntax(
            bundle,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))
    };
    let bytes = encode_bundle(&canonical)?;
    assert_eq!(bytes, encode_bundle(&shuffled)?);
    let restored = decode_syntax(
        &bytes,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored.root, NodeRef(0));
    assert_eq!(restored, canonical);
    let mut unreachable = canonical.clone();
    unreachable.nodes[0].fields.pop();
    assert!(matches!(
        encode_syntax(
            &unreachable,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::UnreachableNode)
    ));
    let mut noncanonical = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    if let NdfValue::Record(bundle) = &mut noncanonical {
        if let NdfValue::Record(root) = &mut bundle.fields[3] {
            root.fields[0] = NdfValue::U64(1);
        }
        if let NdfValue::List(nodes) = &mut bundle.fields[1] {
            nodes.swap(0, 1);
            if let NdfValue::Record(root) = &mut nodes[1]
                && let NdfValue::List(fields) = &mut root.fields[2]
                && let NdfValue::Variant(child) = &mut fields[1]
                && let NdfValue::Record(reference) = &mut child.fields[0]
            {
                reference.fields[0] = NdfValue::U64(0);
            }
        }
    }
    let bytes = encode(&noncanonical, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        decode_syntax(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::NonCanonical)
    ));
    Ok(())
}

#[test]
fn nested_generated_bundles_use_iterative_conversion_and_shared_depth_budget() -> TestResult {
    let (schema, registry, mut bundle) = fixture()?;
    let environment = bundle.environments[0].clone();
    let source = bundle.sources[0].clone();
    for _ in 0..256 {
        bundle = SyntaxBundle {
            source_maps: vec![],
            sources: vec![source.clone()],
            root: NodeRef(0),
            origins: vec![Origin::Synthetic {
                reason: "generated wrapper".into(),
                anchor: None,
            }],
            tokens: vec![],
            environments: vec![environment.clone()],
            nodes: vec![SyntaxNode {
                schema: schema.clone(),
                kind: "Token".into(),
                head: None,
                cover: None,
                origin: OriginId(0),
                token: None,
                fields: vec![FieldValue::Foreign(Box::new(ForeignSyntax {
                    schema: schema.clone(),
                    category: "generated fixture".into(),
                    root: NodeRef(0),
                    bundle,
                    environment: EnvironmentRef {
                        id: environment.id,
                        digest: environment.digest,
                    },
                }))],
            }],
        };
    }
    let encoded = encode_syntax(
        &bundle,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let restored = decode_syntax(
        &encoded,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, bundle);
    assert!(
        decode_syntax(
            &encoded[..encoded.len() - 1],
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    let mut limits = budget().limits();
    limits.depth = 32;
    assert!(matches!(
        decode_syntax(
            &encoded,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut Budget::new(limits)
        ),
        Err(WireError::Stopped(
            nepl3_core::budget::StopReason::DepthLimit
        ))
    ));
    Ok(())
}

#[test]
fn guest_arena_reordering_updates_foreign_root_and_guest_local_children() -> TestResult {
    let (schema, registry, mut canonical) = fixture()?;
    let FieldValue::Foreign(guest) = &mut canonical.nodes[0].fields[0] else {
        return Err("guest fixture".into());
    };
    guest.bundle.nodes.push(guest.bundle.nodes[0].clone());
    guest.bundle.nodes[0].head = None;
    guest.bundle.nodes[0].token = None;
    guest.bundle.nodes[0].fields = vec![FieldValue::Child(NodeRef(1))];
    let mut shuffled = canonical.clone();
    let FieldValue::Foreign(guest) = &mut shuffled.nodes[0].fields[0] else {
        return Err("guest fixture".into());
    };
    guest.bundle.nodes.swap(0, 1);
    guest.bundle.nodes[1].fields = vec![FieldValue::Child(NodeRef(0))];
    guest.bundle.root = NodeRef(1);
    guest.root = NodeRef(1);
    let run = |bundle: &SyntaxBundle| {
        encode_syntax(
            bundle,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))
    };
    let bytes = run(&canonical)?;
    assert_eq!(bytes, run(&shuffled)?);
    let decoded = decode_syntax(
        &bytes,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let FieldValue::Foreign(guest) = &decoded.nodes[0].fields[0] else {
        return Err("guest fixture".into());
    };
    assert_eq!(guest.root, NodeRef(0));
    assert_eq!(guest.bundle.root, NodeRef(0));
    assert_eq!(
        guest.bundle.nodes[0].fields,
        vec![FieldValue::Child(NodeRef(1))]
    );
    Ok(())
}

#[test]
fn mapped_views_and_maps_survive_host_and_guest_wire_boundaries() -> TestResult {
    use nepl3_core::origin::{Mapping, MappingKind};
    let (schema, registry, mut bundle) = fixture()?;
    let add = |bundle: &mut SyntaxBundle, id: &str, text: &str| -> Result<(), String> {
        let decoded = SourceSnapshot::new(
            SourceId(id.into()),
            1,
            format!("memory:{id}"),
            text.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let span = decoded
            .span(0, text.len() as u64)
            .map_err(|e| format!("{e:?}"))?;
        bundle.source_maps.push(Mapping {
            source: bundle.tokens[0].head.clone(),
            target: span.clone(),
            kind: MappingKind::Transformed,
        });
        let kind = bundle.tokens[0].kind.clone();
        bundle.tokens[0].views.elements[0].fields.push(ViewField {
            name: "decoded".into(),
            children: vec![ViewRef(1)],
        });
        bundle.tokens[0].views.elements.push(ViewElement {
            kind,
            span,
            fields: vec![],
            roles: vec![],
            relations: vec![],
        });
        bundle.sources.push(decoded);
        bundle
            .sources
            .sort_by(|a, b| a.identity().cmp(b.identity()));
        Ok(())
    };
    add(&mut bundle, "decoded-host", "X")?;
    let FieldValue::Foreign(guest) = &mut bundle.nodes[0].fields[0] else {
        return Err("guest".into());
    };
    add(&mut guest.bundle, "decoded-guest", "Y")?;
    let run = |bundle: &SyntaxBundle| {
        encode_syntax(
            bundle,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
    };
    let encoded = run(&bundle).map_err(|e| format!("{e:?}"))?;
    let decoded = decode_syntax(
        &encoded,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, bundle);
    let mut invalid = bundle.clone();
    invalid.source_maps.clear();
    assert!(run(&invalid).is_err());
    // A declared mapping is insufficient when its original range is outside this token.
    let mut unrelated = bundle.clone();
    let original = unrelated
        .sources
        .iter()
        .find(|s| s.identity().source.0 == "doc")
        .ok_or("doc")?;
    unrelated.source_maps[0].source = original.span(1, 2).map_err(|e| format!("{e:?}"))?;
    assert!(run(&unrelated).is_err());
    // The wire structural validator accepts the shape; the semantic adapter rejects missing provenance.
    let mut value = decode(&encoded, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut value else {
        return Err("bundle record".into());
    };
    root.fields[6] = NdfValue::List(vec![]);
    let bytes = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(
        decode_syntax(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    let mut wrong_kind = decode(&encoded, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut wrong_kind else {
        return Err("bundle record".into());
    };
    let NdfValue::List(maps) = &mut root.fields[6] else {
        return Err("map list".into());
    };
    let NdfValue::Record(mapping) = &mut maps[0] else {
        return Err("map record".into());
    };
    let NdfValue::Variant(kind) = &mut mapping.fields[2] else {
        return Err("kind".into());
    };
    kind.variant = "Exact".into();
    let bytes = encode(&wrong_kind, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(
        decode_syntax(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    // Guest provenance never resolves through the host's table.
    let mut missing = bundle.clone();
    let FieldValue::Foreign(guest) = &mut missing.nodes[0].fields[0] else {
        return Err("guest".into());
    };
    let original = guest
        .bundle
        .sources
        .iter()
        .position(|s| s.identity().source.0 == "doc")
        .ok_or("guest doc")?;
    guest.bundle.sources.remove(original);
    assert!(
        missing
            .sources
            .iter()
            .any(|s| s.identity().source.0 == "doc")
    );
    assert!(run(&missing).is_err());
    let (_, _, mut empty) = fixture()?;
    add(&mut empty, "decoded-empty", "")?;
    let bytes = run(&empty).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        decode_syntax(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?,
        empty
    );
    Ok(())
}
