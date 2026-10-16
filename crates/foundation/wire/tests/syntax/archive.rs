use super::*;
use nepl3_core::schema::archive as contracts;
use nepl3_wire::syntax::archive as codec;
fn setup() -> Result<
    (
        SchemaRef,
        SchemaRef,
        SchemaRef,
        SchemaRegistry,
        SyntaxBundle,
    ),
    String,
> {
    let (f, mut r, bundle) = fixture()?;
    let mut b = budget();
    let source = contracts::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let syntax = contracts::syntax::descriptor(&mut b).map_err(|e| format!("{e:?}"))?;
    let sr = source.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    let ar = syntax.reference(&mut b).map_err(|e| format!("{e:?}"))?;
    r.register(sr.clone(), source, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    r.register(ar.clone(), syntax, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    r.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    Ok((ar, sr, f, r, bundle))
}
#[test]
fn nested_syntax_archive_restores_sources_and_legacy_identity() -> TestResult {
    let (a, s, f, r, bundle) = setup()?;
    let old = encode_syntax(
        &bundle,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let decoded = codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded, bundle);
    let restored = encode_syntax(
        &decoded,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(old, restored);
    let FieldValue::Foreign(guest) = &decoded.nodes[0].fields[0] else {
        return Err("foreign".into());
    };
    #[cfg(target_has_atomic = "ptr")]
    assert!(core::ptr::eq(
        decoded.sources[0].text().as_ptr(),
        guest.bundle.sources[0].text().as_ptr()
    ));
    assert!(
        decode_syntax(
            &bytes,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    assert!(
        codec::decode(
            &old,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn missing_local_membership_and_extra_sets_are_rejected() -> TestResult {
    let (a, s, f, r, bundle) = setup()?;
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for mode in 0..3 {
        let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(root) = &mut value else {
            return Err("archive root".into());
        };
        let NdfValue::Record(sources) = &mut root.fields[0] else {
            return Err("source sets".into());
        };
        let NdfValue::List(sets) = &mut sources.fields[1] else {
            return Err("memberships".into());
        };
        match mode {
            0 => {
                sets[0] = NdfValue::List(vec![]);
            }
            1 => {
                sets.pop();
            }
            _ => {
                sets.push(NdfValue::List(vec![]));
            }
        }
        let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            codec::decode(
                &bad,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err(),
            "mode {mode}"
        );
    }
    Ok(())
}
#[test]
fn nonempty_skeleton_declarations_reject() -> TestResult {
    let (a, s, f, r, bundle) = setup()?;
    let mut value = decode(
        &codec::encode(
            &bundle,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut value else {
        return Err("archive root".into());
    };
    root.fields[1] = NdfValue::Bytes(
        encode_syntax(
            &bundle,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?,
    );
    let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        codec::decode(
            &bad,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(WireError::NonCanonical)
    ));
    Ok(())
}

#[test]
fn distinct_foreign_siblings_preserve_source_set_association() -> TestResult {
    let (a, s, f, r, mut bundle) = setup()?;
    let FieldValue::Foreign(first) = &bundle.nodes[0].fields[0] else {
        return Err("foreign fixture".into());
    };
    let mut second = first.clone();
    let source = SourceSnapshot::new(
        SourceId("second".into()),
        1,
        "memory:second".into(),
        b"xy".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = source.span(1, 2).map_err(|e| format!("{e:?}"))?;
    second.bundle.sources = vec![source];
    second.bundle.nodes[0].head = Some(span.clone());
    second.bundle.nodes[0].cover = Some(span.clone());
    second.bundle.origins = vec![Origin::Direct(span.clone())];
    second.bundle.tokens[0].head = span.clone();
    second.bundle.tokens[0].views.elements[0].span = span;
    bundle.nodes[0].fields.push(FieldValue::Foreign(second));
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let result = codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(result, bundle);
    let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut value else {
        return Err("root".into());
    };
    let NdfValue::Record(sets) = &mut root.fields[0] else {
        return Err("sets".into());
    };
    let NdfValue::List(memberships) = &mut sets.fields[1] else {
        return Err("memberships".into());
    };
    memberships.swap(0, 1);
    let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(
        codec::decode(
            &bad,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn raw_structural_and_semantic_skeleton_errors_reject() -> TestResult {
    let (a, s, f, r, bundle) = setup()?;
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    for mode in 0..4 {
        let mut value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let NdfValue::Record(root) = &mut value else {
            return Err("archive root".into());
        };
        let NdfValue::Bytes(skeleton) = &mut root.fields[1] else {
            return Err("skeleton".into());
        };
        if mode == 0 {
            *skeleton = vec![0xff];
        } else {
            let mut graph = decode(skeleton, &mut budget()).map_err(|e| format!("{e:?}"))?;
            let NdfValue::Record(graph_root) = &mut graph else {
                return Err("graph root".into());
            };
            match mode {
                1 => graph_root.schema.revision = 9,
                2 => {
                    let NdfValue::Record(root_ref) = &mut graph_root.fields[3] else {
                        return Err("root ref".into());
                    };
                    root_ref.fields[0] = NdfValue::U64(99);
                }
                _ => {
                    let NdfValue::List(nodes) = &mut graph_root.fields[1] else {
                        return Err("nodes".into());
                    };
                    let NdfValue::Record(node) = &mut nodes[0] else {
                        return Err("node".into());
                    };
                    let NdfValue::Record(origin) = &mut node.fields[5] else {
                        return Err("origin".into());
                    };
                    origin.fields[0] = NdfValue::U64(99);
                }
            }
            *skeleton = encode(&graph, &mut budget()).map_err(|e| format!("{e:?}"))?;
        }
        let bad = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
        assert!(
            codec::decode(
                &bad,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err(),
            "mode {mode}"
        );
    }
    Ok(())
}
#[test]
fn exact_resource_edges_and_cancel_are_sticky() -> TestResult {
    use nepl3_core::budget::StopReason;
    let (a, s, f, r, bundle) = setup()?;
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut measured = budget();
    codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    let u = measured.usage();
    let exact = Limits {
        work: u.work,
        allocation_units: u.allocation_units,
        source_bytes: u.source_bytes,
        depth: u.depth,
        ..budget().limits()
    };
    codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut Budget::new(exact),
    )
    .map_err(|e| format!("{e:?}"))?;
    for (limits, reason) in [
        (
            Limits {
                work: u.work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: u.allocation_units - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                source_bytes: u.source_bytes - 1,
                ..exact
            },
            StopReason::SourceLimit,
        ),
        (
            Limits {
                depth: u.depth - 1,
                ..exact
            },
            StopReason::DepthLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        assert!(
            codec::decode(
                &bytes,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut b
            )
            .is_err()
        );
        assert_eq!(b.poll(), Err(reason));
        assert!(
            codec::decode(
                &bytes,
                &a,
                &s,
                &f,
                &r,
                &mut SourceAdmission::default(),
                &mut b
            )
            .is_err()
        );
        assert_eq!(b.poll(), Err(reason));
    }
    let mut b = budget();
    b.stop(StopReason::Cancelled);
    assert!(
        codec::decode(
            &bytes,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut b
        )
        .is_err()
    );
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}
#[test]
fn opaque_source_record_payload_is_not_rewritten() -> TestResult {
    let (a, s, f, r, mut bundle) = setup()?;
    let payload = nepl3_wire::source::encode_sources(
        &bundle.sources,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    bundle.tokens[0].payload = decode(&payload, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let decoded = codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(decoded.tokens[0].payload, bundle.tokens[0].payload);
    Ok(())
}

#[test]
fn repeated_large_source_reduces_transport_bytes_and_work() -> TestResult {
    fn repoint(bundle: &mut SyntaxBundle, source: &SourceSnapshot) -> Result<(), String> {
        let span = |old: &nepl3_core::source::Span| {
            source
                .span(old.start(), old.end())
                .map_err(|e| format!("{e:?}"))
        };
        bundle.sources = vec![source.clone()];
        for node in &mut bundle.nodes {
            node.head = node.head.as_ref().map(span).transpose()?;
            node.cover = node.cover.as_ref().map(span).transpose()?;
            for field in &mut node.fields {
                if let FieldValue::Foreign(guest) = field {
                    repoint(&mut guest.bundle, source)?;
                }
            }
        }
        for origin in &mut bundle.origins {
            let Origin::Direct(old) = origin else {
                return Err("fixture origin".into());
            };
            *old = span(old)?;
        }
        for token in &mut bundle.tokens {
            token.head = span(&token.head)?;
            for view in &mut token.views.elements {
                view.span = span(&view.span)?;
            }
        }
        Ok(())
    }
    let (a, s, f, r, mut bundle) = setup()?;
    let mut text = vec![b'x'; 32768];
    text[0] = b'a';
    text[1] = b'b';
    let source = SourceSnapshot::new(
        SourceId("doc".into()),
        1,
        "memory:syntax".into(),
        text,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    repoint(&mut bundle, &source)?;
    let FieldValue::Foreign(guest) = &bundle.nodes[0].fields[0] else {
        return Err("guest".into());
    };
    let guest = guest.clone();
    for _ in 0..23 {
        bundle.nodes[0]
            .fields
            .push(FieldValue::Foreign(guest.clone()));
    }
    let mut legacy_encode = budget();
    let legacy = encode_syntax(
        &bundle,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut legacy_encode,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut archive_encode = budget();
    let bytes = codec::encode(
        &bundle,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut archive_encode,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut legacy_decode = budget();
    let old = decode_syntax(
        &legacy,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut legacy_decode,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut archive_decode = budget();
    let new = codec::decode(
        &bytes,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut archive_decode,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(new, old);
    // Compare the same real fixture through complete endpoints, not a fabricated
    // resource counter. This does not include opaque-payload transport or Doc.
    assert!(bytes.len() < legacy.len());
    assert!(archive_encode.usage().work < legacy_encode.usage().work);
    assert!(archive_decode.usage().work < legacy_decode.usage().work);
    eprintln!(
        "direct-foreign archive: bytes {} -> {}; encode Work {} -> {}; decode Work {} -> {}",
        legacy.len(),
        bytes.len(),
        legacy_encode.usage().work,
        archive_encode.usage().work,
        legacy_decode.usage().work,
        archive_decode.usage().work
    );
    Ok(())
}
