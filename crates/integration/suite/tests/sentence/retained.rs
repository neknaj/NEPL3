use super::*;
#[path = "legacy_guests.rs"]
mod legacy;

pub(super) fn fixture()
-> Result<(SchemaRegistry, nepl3_core::value::SchemaRef, SentenceSyntax), String> {
    use nepl3_core::schema::{NamedType, SchemaDescriptor, TypeShape};
    use nepl3_core::syntax::group::*;
    let mut r = SchemaRegistry::default();
    let surface = SchemaDescriptor {
        package: "test.doc.surface".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Form:Break".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: vec![],
    };
    let selected = surface.reference(&mut b()).map_err(err)?;
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?,
        nepl3_sentence_core::schema::descriptor(&mut b()).map_err(err)?,
        nepl3_doc_core::schema::descriptor(&mut b()).map_err(err)?,
        surface,
    ] {
        r.register(
            descriptor.reference(&mut b()).map_err(err)?,
            descriptor,
            &mut b(),
        )
        .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    let origins = vec![Origin::Synthetic {
        reason: "generated".into(),
        anchor: None,
    }];
    let env = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let foundation = r.selected("nepl3.foundation", 1).ok_or("foundation")?;
    let digest =
        nepl3_wire::environment::environment_digest(&env, foundation, &r, &mut b()).map_err(err)?;
    let guest_source = SourceSnapshot::new(
        SourceId("guest-source".into()),
        1,
        "memory:retained-guest".into(),
        "界".repeat(4096).into_bytes(),
        &mut b(),
    )
    .map_err(err)?;
    let guest_origin = Origin::Direct(
        guest_source
            .span(0, guest_source.text().len() as u64)
            .map_err(err)?,
    );
    let syntax = ForeignSyntax {
        schema: selected.clone(),
        category: "Inline".into(),
        root: NodeRef(0),
        environment: EnvironmentRef { id: 0, digest },
        bundle: SyntaxBundle {
            sources: vec![guest_source],
            nodes: vec![SyntaxNode {
                schema: selected.clone(),
                kind: "Form:Break".into(),
                fields: vec![],
                head: None,
                cover: None,
                origin: OriginId(0),
                token: None,
            }],
            origins: vec![guest_origin],
            root: NodeRef(0),
            environments: vec![],
            tokens: vec![],
            source_maps: vec![],
        },
    };
    let input = SentenceSyntax {
        value: SentenceValue {
            root: Root::Inline(InlineRef(2)),
            nodes: vec![
                Kind::ForeignInline {
                    syntax: EmbedRef(0),
                },
                Kind::Concat {
                    inlines: vec![InlineRef(0), InlineRef(0)],
                },
                Kind::Concat {
                    inlines: vec![InlineRef(0), InlineRef(1)],
                },
            ],
            embeds: ForeignClosureGroup {
                provenances: vec![ForeignProvenance {
                    origins: vec![],
                    sources: vec![],
                    source_maps: vec![],
                }],
                environments: vec![ForeignOwnerEnvironment {
                    provenance: ProvenanceRef(0),
                    environment: EnvironmentEntry {
                        id: 0,
                        digest,
                        value: env,
                    },
                }],
                entries: vec![ForeignGroupEntry {
                    owner: OwnerEnvironmentRef(0),
                    syntax,
                }],
            },
        },
        locations: vec![
            NodeLocation {
                origin: OriginId(0),
                head: None,
                cover: None
            };
            3
        ],
        sources: vec![],
        origins,
        views: vec![],
        source_maps: vec![],
    };
    Ok((r, selected, input))
}

#[test]
fn retained_selected_doc_guest_matches_legacy_and_saves_work() -> Result<(), String> {
    let (r, surface, input) = fixture()?;
    let store = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let mut old = b();
    let expected = old
        .with_depth_at_least(7, |b| legacy::collect(&input, &surface, &r, &mut codec, b))
        .map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    let mut measured = b();
    let actual = measured
        .with_depth_at_least(7, |b| {
            sentence::document_guests::collect(&input, &surface, &r, &mut codec, b)
        })
        .map_err(err)?;
    assert_eq!(actual.documents(), expected.documents());
    assert_eq!(actual.documents().len(), 1);
    assert_eq!(actual.occurrences().len(), 3);
    assert_eq!(
        actual
            .occurrences()
            .iter()
            .map(|x| (x.embed, x.document.index()))
            .collect::<Vec<_>>(),
        expected
            .occurrences()
            .iter()
            .map(|x| (x.embed, x.document.index()))
            .collect::<Vec<_>>()
    );
    assert!(measured.usage().work < old.usage().work);
    assert_eq!(measured.usage().source_bytes, old.usage().source_bytes);
    assert_eq!(measured.usage().depth, old.usage().depth);
    assert_eq!(measured.current_depth(), 0);
    let _ = expected.into_parts();
    for resource in 0..3 {
        for short in [false, true] {
            let mut limits = b().limits();
            match resource {
                0 => limits.work = measured.usage().work - u64::from(short),
                1 => limits.allocation_units = measured.usage().allocation_units - u64::from(short),
                _ => limits.depth = measured.usage().depth - u64::from(short),
            }
            let mut limited = Budget::new(limits);
            let mut admission = SourceAdmission::default();
            let mut codec =
                nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission)
                    .map_err(err)?;
            let result = limited.with_depth_at_least(7, |b| {
                sentence::document_guests::collect(&input, &surface, &r, &mut codec, b)
            });
            if short {
                assert!(result.is_err());
                assert_eq!(
                    limited.poll(),
                    Err(match resource {
                        0 => StopReason::WorkLimit,
                        1 => StopReason::AllocationLimit,
                        _ => StopReason::DepthLimit,
                    })
                );
            } else {
                assert_eq!(result.map_err(err)?.documents(), actual.documents());
            }
            assert_eq!(limited.current_depth(), 0);
        }
    }
    Ok(())
}

#[test]
fn retained_selection_rejects_invalid_later_owner_and_hidden_tables() -> Result<(), String> {
    let (r, surface, input) = fixture()?;
    for case in 0..5 {
        let mut invalid = input.clone();
        match case {
            0 => invalid
                .value
                .embeds
                .provenances
                .push(invalid.value.embeds.provenances[0].clone()),
            1 => invalid.value.embeds.entries[0].syntax.root = NodeRef(99),
            2 => {
                invalid.value.nodes = vec![Kind::Break];
                invalid.value.root = Root::Inline(InlineRef(0));
                invalid.locations.truncate(1);
                invalid.value.embeds.entries.clear();
            }
            3 => invalid.value.embeds.entries[0].syntax.category.clear(),
            _ => invalid.value.embeds.provenances[0]
                .origins
                .push(Origin::Composite(vec![OriginId(u64::MAX)])),
        }
        let store = nepl3_core::source::SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = nepl3_wire::foundation::FoundationCodec::new(&r, &store, &mut admission)
            .map_err(err)?;
        assert!(
            sentence::document_guests::collect(&invalid, &surface, &r, &mut codec, &mut b())
                .is_err()
        );
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert!(
        input
            .validate_retaining_foreign_bundles(&r, &mut cancelled, &mut SourceAdmission::default())
            .is_err()
    );
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}
