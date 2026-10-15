use super::*;
use nepl3_core::value_codec::FoundationCodecError;

fn flat(nodes: usize) -> Result<(SchemaRef, SchemaRegistry, SyntaxBundle), String> {
    let (schema, registry, mut bundle) = fixture()?;
    bundle.environments.clear();
    let source = SourceSnapshot::new(
        SourceId("doc".into()),
        1,
        "memory:order".into(),
        vec![b'x'; nodes],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let template_node = bundle.nodes[0].clone();
    let template_token = bundle.tokens[0].clone();
    bundle.nodes.clear();
    bundle.tokens.clear();
    bundle.origins.clear();
    for index in 0..nodes {
        let span = source
            .span(index as u64, index as u64 + 1)
            .map_err(|e| format!("{e:?}"))?;
        let mut node = template_node.clone();
        node.head = Some(span.clone());
        node.cover = Some(if index == 0 {
            source.span(0, nodes as u64).map_err(|e| format!("{e:?}"))?
        } else {
            span.clone()
        });
        node.origin = OriginId(index as u64);
        node.token = Some(TokenRef(index as u64));
        node.fields = if index == 0 {
            vec![FieldValue::Children(
                (1..nodes as u64).map(NodeRef).collect(),
            )]
        } else {
            vec![FieldValue::Atom(NdfScalar::U64(index as u64))]
        };
        let mut token = template_token.clone();
        token.head = span.clone();
        token.views.elements[0].span = span.clone();
        bundle.nodes.push(node);
        bundle.tokens.push(token);
        bundle.origins.push(Origin::Direct(span));
    }
    bundle.sources = vec![source];
    Ok((schema, registry, bundle))
}
#[test]
fn retained_order_preserves_large_arena_permutations_and_measured_work() -> TestResult {
    for count in [1, 2, 3, 65, 257] {
        let (schema, registry, bundle) = flat(count)?;
        let mut measured = budget();
        let bytes = encode_syntax(
            &bundle,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut measured,
        )
        .map_err(|e| format!("{e:?}"))?;
        // Recorded on the unchanged two-pass encoder at 4c8d0434 using this
        // exact fixture. These are logical counters, not timing assertions.
        // A real negative-control build of that encoder must fail the broad
        // case while the singleton keeps its original allocation allowance.
        if cfg!(target_pointer_width = "64") {
            let reference = match count {
                1 => Some((8_166, 18_351)),
                65 => Some((391_756, 1_014_136)),
                257 => Some((1_541_861, 4_002_449)),
                _ => None,
            };
            if let Some((old_work, old_allocation)) = reference {
                let mut ordering = budget();
                let _mapping =
                    nepl3_core::syntax::canonical::NodeMapping::new(&bundle, &mut ordering)
                        .map_err(|e| format!("{e:?}"))?;
                let cache = core::mem::size_of::<(Vec<usize>, Vec<u64>)>() as u64;
                let saved = ordering.usage().allocation_units.saturating_sub(cache);
                let work_saved = if count == 1 { 0 } else { ordering.usage().work };
                assert!(
                    measured.usage().work <= old_work - work_saved / 2,
                    "duplicate canonical order work: {:?}",
                    measured.usage()
                );
                assert!(
                    measured.usage().allocation_units <= old_allocation - saved / 2,
                    "retention must not enlarge the singleton frame or erase broad-bundle savings"
                );
            }
        }
        let mut shuffled = bundle.clone();
        shuffled.nodes.reverse();
        shuffled.root = NodeRef((count - 1) as u64);
        let FieldValue::Children(children) = &mut shuffled.nodes[count - 1].fields[0] else {
            return Err("fixture children".into());
        };
        for id in children {
            id.0 = count as u64 - 1 - id.0;
        }
        let actual = encode_syntax(
            &shuffled,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(actual, bytes);
        let restored = decode_syntax(
            &bytes,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(restored, bundle);
    }
    Ok(())
}

#[test]
fn retained_order_keeps_exact_resource_edges_and_sticky_stops() -> TestResult {
    use nepl3_core::budget::{Resource, StopReason};
    let (schema, registry, bundle) = flat(65)?;
    let mut full = budget();
    let expected = full
        .with_depth(|b| {
            encode_syntax(
                &bundle,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                b,
            )
        })
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(full.current_depth(), 0);
    let mut consumed = budget();
    consumed
        .charge(Resource::Work, 23)
        .map_err(|e| format!("{e:?}"))?;
    consumed
        .charge(Resource::AllocationUnits, 17)
        .map_err(|e| format!("{e:?}"))?;
    let actual = consumed
        .with_depth(|b| {
            encode_syntax(
                &bundle,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                b,
            )
        })
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, expected);
    assert_eq!(consumed.usage().work, full.usage().work + 23);
    assert_eq!(
        consumed.usage().allocation_units,
        full.usage().allocation_units + 17
    );
    assert_eq!(consumed.usage().source_bytes, full.usage().source_bytes);
    assert_eq!(consumed.current_depth(), 0);

    for (kind, used, reason) in [
        (Resource::Work, full.usage().work, StopReason::WorkLimit),
        (
            Resource::AllocationUnits,
            full.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        for short in [false, true] {
            let mut limits = budget().limits();
            let cap = used - u64::from(short);
            match kind {
                Resource::Work => limits.work = cap,
                Resource::AllocationUnits => limits.allocation_units = cap,
                _ => return Err("fixture resource".into()),
            }
            let mut b = Budget::new(limits);
            let mut admission = SourceAdmission::default();
            let result =
                b.with_depth(|b| encode_syntax(&bundle, &schema, &registry, &mut admission, b));
            assert_eq!(b.current_depth(), 0);
            if short {
                let error = result.err().ok_or("one-short accepted")?;
                assert_eq!(error.stop_reason(), Some(reason));
                assert_eq!(b.poll(), Err(reason));
                assert!(
                    encode_syntax(&bundle, &schema, &registry, &mut admission, &mut b).is_err()
                );
            } else {
                assert_eq!(result.map_err(|e| format!("{e:?}"))?, expected);
            }
        }
    }
    for short in [false, true] {
        let mut limits = budget().limits();
        limits.depth = full.usage().depth - u64::from(short);
        let mut b = Budget::new(limits);
        let result = b.with_depth(|b| {
            encode_syntax(
                &bundle,
                &schema,
                &registry,
                &mut SourceAdmission::default(),
                b,
            )
        });
        assert_eq!(b.current_depth(), 0);
        if short {
            assert_eq!(
                result.err().ok_or("short depth accepted")?.stop_reason(),
                Some(StopReason::DepthLimit)
            );
        } else {
            assert_eq!(result.map_err(|e| format!("{e:?}"))?, expected);
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        encode_syntax(
            &bundle,
            &schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut cancelled
        )
        .err()
        .ok_or("cancelled encoding")?
        .stop_reason(),
        Some(StopReason::Cancelled)
    );
    Ok(())
}

fn add_guest(
    owner: &mut SyntaxBundle,
    guest: SyntaxBundle,
    schema: &SchemaRef,
    registry: &SchemaRegistry,
) -> TestResult {
    if owner.environments.is_empty() {
        let value = Environment {
            bindings: vec![],
            resources: vec![],
        };
        let digest = environment_digest(&value, schema, registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        owner.environments.push(EnvironmentEntry {
            id: 0,
            digest,
            value,
        });
    }
    let environment = EnvironmentRef {
        id: 0,
        digest: owner.environments[0].digest,
    };
    owner.nodes[0]
        .fields
        .push(FieldValue::Foreign(Box::new(ForeignSyntax {
            schema: schema.clone(),
            category: "fixture".into(),
            root: guest.root,
            bundle: guest,
            environment,
        })));
    Ok(())
}
fn reverse_arena(bundle: &mut SyntaxBundle) {
    let n = bundle.nodes.len() as u64;
    bundle.nodes.reverse();
    bundle.root = NodeRef(n - 1 - bundle.root.0);
    for node in &mut bundle.nodes {
        for field in &mut node.fields {
            match field {
                FieldValue::Child(id) => id.0 = n - 1 - id.0,
                FieldValue::Children(ids) => {
                    for id in ids {
                        id.0 = n - 1 - id.0;
                    }
                }
                FieldValue::Foreign(guest) => {
                    reverse_arena(&mut guest.bundle);
                    guest.root = guest.bundle.root;
                }
                FieldValue::Atom(_) => {}
            }
        }
    }
}
#[test]
fn retained_order_keeps_distinct_nested_and_sibling_guest_associations() -> TestResult {
    let (schema, registry, mut root) = flat(3)?;
    let (_, _, mut first) = flat(3)?;
    let (_, _, mut tiny) = flat(3)?;
    tiny.nodes.truncate(1);
    tiny.nodes[0].fields = vec![FieldValue::Atom(NdfScalar::Text("tiny nested".into()))];
    add_guest(&mut first, tiny, &schema, &registry)?;
    let (_, _, mut second) = flat(3)?;
    second.nodes[2].fields = vec![FieldValue::Atom(NdfScalar::Text("second sibling".into()))];
    add_guest(&mut root, first, &schema, &registry)?;
    add_guest(&mut root, second, &schema, &registry)?;
    let mut measured = budget();
    let expected = encode_syntax(
        &root,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.usage().source_bytes, 3);
    let mut shuffled = root.clone();
    reverse_arena(&mut shuffled);
    let actual = encode_syntax(
        &shuffled,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(actual, expected);
    let restored = decode_syntax(
        &actual,
        &schema,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(restored, root);
    Ok(())
}
