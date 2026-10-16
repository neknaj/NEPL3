use super::*;
use nepl3_doc_core::{labels::namespace, prepare};

#[test]
fn fused_preparation_matches_default_provider_and_saves_group_encoding() -> Result<(), String> {
    let r = registry()?;
    let empty = SourceStore::default();
    let document = mapped_article(closure(&r)?, vec![], vec![]);
    let mut admission = SourceAdmission::default();
    let codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut fallback = default::DefaultCodec(codec);
    let mut old = b();
    let expected = prepare::inspect(&document, &r, &mut fallback, &mut old).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut fused = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut new = b();
    let actual = prepare::inspect(&document, &r, &mut fused, &mut new).map_err(err)?;
    assert_eq!(actual, expected);
    assert!(new.usage().work < old.usage().work);
    assert!(new.usage().allocation_units < old.usage().allocation_units);
    // The unchanged portable path is an independent serialization oracle for
    // the manually fused DocValue/DocEmbeds field layout.
    let value = portable::to_value(&document, &r, &mut fused, &mut b()).map_err(err)?;
    let digest = fused
        .canonical_value_digest(prepare::DOCUMENT_DOMAIN, &value, &mut b())
        .map_err(err)?;
    assert_eq!(actual.document_digest, digest);
    let components = fused
        .foreign_group_entry_digests(&document.value.embeds.group, &mut b())
        .map_err(err)?;
    for (requirement, component) in actual.requirements.iter().zip(components) {
        let prepare::DocRequirement::Foreign { guest_digest, .. } = requirement else {
            return Err("foreign requirement".into());
        };
        let wrapped = fused
            .canonical_value_digest(
                prepare::GUEST_DOMAIN,
                &nepl3_core::value::NdfValue::Bytes(component.0.to_vec()),
                &mut b(),
            )
            .map_err(err)?;
        assert_eq!(*guest_digest, wrapped);
    }
    eprintln!(
        "Doc preparation default-provider Work {} -> {}; Allocation {} -> {}",
        old.usage().work,
        new.usage().work,
        old.usage().allocation_units,
        new.usage().allocation_units
    );
    Ok(())
}
#[test]
fn namespace_and_ambient_scopes_preserve_fused_plan() -> Result<(), String> {
    let r = registry()?;
    let (sources, map) = mapping_pair("document")?;
    let document = mapped_article(closure(&r)?, sources.clone(), vec![map.clone()]);
    let mut store = SourceStore::default();
    for source in sources {
        store.insert_with_budget(source, &mut b()).map_err(err)?;
    }
    let mut admission = SourceAdmission::default();
    let member = namespace::inspect(&document, &r, &mut b(), &mut admission).map_err(err)?;
    let refs = [&member];
    let ns = namespace::resolve(&refs, &mut b()).map_err(err)?;
    let mut c = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let maps = [map];
    let mut scoped = c.scoped_with_mappings(&store, &maps);
    let actual = prepare::inspect_namespace(&ns, &r, &mut scoped, &mut b()).map_err(err)?;
    let mut fallback = default::DefaultCodec(scoped);
    let expected = prepare::inspect_namespace(&ns, &r, &mut fallback, &mut b()).map_err(err)?;
    assert_eq!(actual, expected);
    Ok(())
}
#[test]
fn fused_preparation_retains_exact_limits_and_sticky_stops() -> Result<(), String> {
    let r = registry()?;
    let empty = SourceStore::default();
    let doc = mapped_article(closure(&r)?, vec![], vec![]);
    let run = |budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        prepare::inspect(&doc, &r, &mut c, budget).map_err(err)
    };
    let mut measured = b();
    run(&mut measured)?;
    let u = measured.usage();
    let exact = Limits {
        work: u.work,
        allocation_units: u.allocation_units,
        source_bytes: u.source_bytes,
        depth: u.depth,
        ..b().limits()
    };
    run(&mut Budget::new(exact))?;
    for (limit, reason) in [
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
        let mut stopped = Budget::new(limit);
        assert!(run(&mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(reason));
        assert!(run(&mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(reason));
    }
    let mut cancelled = b();
    cancelled.stop(StopReason::Cancelled);
    assert!(run(&mut cancelled).is_err());
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn sentence_and_inline_plans_match_default_provider() -> Result<(), String> {
    let r = registry()?;
    let empty = SourceStore::default();
    for sentence in [false, true] {
        let mut doc = DocumentSyntax {
            value: DocValue {
                root: DocRoot::Inline(InlineRef(0)),
                nodes: vec![DocNode {
                    kind: DocKind::InlineMath {
                        syntax: EmbedRef(0),
                    },
                    locations: vec![],
                    origin: None,
                    span: None,
                }],
                embeds: embeds(EmbedKind::InlineMath, closure(&r)?),
            },
            sources: vec![],
            origins: vec![],
            views: vec![],
            source_maps: vec![],
        };
        if sentence {
            doc.value.nodes.push(DocNode {
                kind: DocKind::Sentence {
                    inlines: vec![InlineRef(0)],
                },
                locations: vec![],
                origin: None,
                span: None,
            });
            doc.value.root = DocRoot::Sentence(SentenceRef(1));
        }
        let mut admission = SourceAdmission::default();
        let mut fused = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let actual = if sentence {
            prepare::inspect_sentence(&doc, &r, &mut fused, &mut b())
        } else {
            prepare::inspect_inline(&doc, &r, &mut fused, &mut b())
        }
        .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut fallback =
            default::DefaultCodec(FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?);
        let expected = if sentence {
            prepare::inspect_sentence(&doc, &r, &mut fallback, &mut b())
        } else {
            prepare::inspect_inline(&doc, &r, &mut fallback, &mut b())
        }
        .map_err(err)?;
        assert_eq!(actual, expected);
        let value = portable::to_value(&doc, &r, &mut fallback, &mut b()).map_err(err)?;
        assert_eq!(
            actual.document_digest,
            fallback
                .canonical_value_digest(prepare::DOCUMENT_DOMAIN, &value, &mut b())
                .map_err(err)?
        );
        doc.value.embeds.group.entries[0]
            .syntax
            .environment
            .digest
            .0[0] ^= 1;
        assert!(
            if sentence {
                prepare::inspect_sentence(&doc, &r, &mut fused, &mut b())
            } else {
                prepare::inspect_inline(&doc, &r, &mut fused, &mut b())
            }
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn repeated_source_preparation_reuses_the_same_encoded_group() -> Result<(), String> {
    let r = registry()?;
    let mut guest = closure(&r)?;
    let source = SourceSnapshot::new(
        SourceId("guest".into()),
        1,
        "memory:guest".into(),
        vec![b'x'; 32768],
        &mut b(),
    )
    .map_err(err)?;
    let span = source.span(0, 32768).map_err(err)?;
    guest.syntax.bundle.sources = vec![source];
    guest.syntax.bundle.nodes[0].head = Some(span.clone());
    guest.syntax.bundle.nodes[0].cover = Some(span.clone());
    guest.syntax.bundle.origins = vec![Origin::Direct(span)];
    let mut document = mapped_article(guest, vec![], vec![]);
    for i in 1..24 {
        document
            .value
            .embeds
            .group
            .entries
            .push(document.value.embeds.group.entries[0].clone());
        document.value.embeds.kinds.push(EmbedKind::Code);
        document.value.nodes.push(DocNode {
            kind: DocKind::Code {
                syntax: EmbedRef(i),
            },
            locations: vec![],
            origin: None,
            span: None,
        });
    }
    let DocKind::Body { blocks } = &mut document.value.nodes[3].kind else {
        return Err("body".into());
    };
    blocks.extend((5..28).map(BlockRef));
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut fallback =
        default::DefaultCodec(FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?);
    let mut old = b();
    let expected = prepare::inspect(&document, &r, &mut fallback, &mut old).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut fused = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut new = b();
    let actual = prepare::inspect(&document, &r, &mut fused, &mut new).map_err(err)?;
    assert_eq!(actual, expected);
    assert!(new.usage().work < old.usage().work);
    assert!(new.usage().allocation_units < old.usage().allocation_units);
    eprintln!(
        "synthetic Doc24 preparation: Work {} -> {}; Allocation {} -> {}",
        old.usage().work,
        new.usage().work,
        old.usage().allocation_units,
        new.usage().allocation_units
    );
    Ok(())
}
