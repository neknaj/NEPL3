#![cfg(feature = "doc-sentence")]
use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::*,
    source::{SourceAdmission, SourceStore},
    syntax::*,
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::{DocContent, DocEmbed, EmbedKind};
use nepl3_sentence_core::{
    model::{InlineRef, Kind, Root, SentenceRef, SentenceValue},
    syntax::{NodeLocation, SentenceSyntax},
};
use nepl3_suite::adapters::document::sentence::{self, Error};
use nepl3_wire::foundation::FoundationCodec;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        nodes: 100_000,
        depth: 1000,
        source_bytes: 100_000,
        output_bytes: 100_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

fn input() -> Result<(SchemaRegistry, SchemaRef, DocEmbed), String> {
    // Independent, source-less syntax fixture. It does not use the Doc bridge,
    // Grammar compiler or a print/parse roundtrip to define the expected value.
    let surface = SchemaDescriptor {
        package: "test.doc-sentence".into(),
        revision: 1,
        operations: vec![],
        types: vec![
            NamedType {
                name: "Form:Break".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "Form:Sentence".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "inlines".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "NodeRef".into(),
                        }),
                    }],
                },
            },
            NamedType {
                name: "List:Inline:Nil".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "List:Inline:Cons".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: ["head", "tail"]
                        .map(|name| FieldDescriptor {
                            name: name.into(),
                            ty: TypeDescriptor::Named(TypeRef {
                                package: "nepl3.foundation".into(),
                                revision: 1,
                                name: "NodeRef".into(),
                            }),
                        })
                        .into(),
                },
            },
        ],
    };
    let identity = surface.reference(&mut budget()).map_err(err)?;
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        nepl3_sentence_core::schema::descriptor(&mut budget()).map_err(err)?,
        surface,
    ] {
        registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let digest = codec
        .environment_digest(&environment, &mut budget())
        .map_err(err)?;
    let node = |kind: &str, fields| SyntaxNode {
        schema: identity.clone(),
        kind: kind.into(),
        fields,
        head: None,
        cover: None,
        token: None,
        origin: OriginId(0),
    };
    let embed = DocEmbed {
        kind: EmbedKind::Sentence,
        content: DocContent::Syntax {
            closure: Box::new(ForeignClosure {
                syntax: ForeignSyntax {
                    schema: identity.clone(),
                    category: "Sentence".into(),
                    root: NodeRef(4),
                    bundle: SyntaxBundle {
                        nodes: vec![
                            node("Form:Break", vec![]),
                            node("List:Inline:Nil", vec![]),
                            node(
                                "List:Inline:Cons",
                                vec![FieldValue::Child(NodeRef(0)), FieldValue::Child(NodeRef(1))],
                            ),
                            node(
                                "List:Inline:Cons",
                                vec![FieldValue::Child(NodeRef(0)), FieldValue::Child(NodeRef(2))],
                            ),
                            node("Form:Sentence", vec![FieldValue::Child(NodeRef(3))]),
                        ],
                        root: NodeRef(4),
                        origins: vec![Origin::Synthetic {
                            reason: "generated sentence fixture".into(),
                            anchor: None,
                        }],
                        sources: vec![],
                        tokens: vec![],
                        source_maps: vec![],
                        environments: vec![],
                    },
                    environment: EnvironmentRef { id: 1, digest },
                },
                owner_environment: EnvironmentEntry {
                    id: 1,
                    digest,
                    value: environment,
                },
                provenance: nepl3_core::syntax::OwnerProvenance::from_parts(vec![], vec![], vec![]),
            }),
        },
    };
    Ok((registry, identity, embed))
}

#[test]
fn doc_consumes_independent_sentence_with_sharing_and_generated_provenance() -> Result<(), String> {
    let (registry, surface, input) = input()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let output = sentence::lower(&input, &surface, &[], &registry, &mut codec, &mut budget())
        .map_err(err)?;
    assert_eq!(output.value.root, Root::Sentence(SentenceRef(1)));
    assert_eq!(
        output.value.nodes,
        vec![
            Kind::Break,
            Kind::Sentence {
                inlines: vec![InlineRef(0), InlineRef(0)]
            }
        ]
    );
    assert_eq!(
        output.origins,
        input
            .syntax()
            .ok_or("syntax fixture")?
            .syntax
            .bundle
            .origins
    );
    assert!(output.sources.is_empty());
    assert!(
        output
            .locations
            .iter()
            .all(|location| location.origin == OriginId(0)
                && location.head.is_none()
                && location.cover.is_none())
    );

    let mut wrong = input.clone();
    wrong.kind = EmbedKind::InlineMath;
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Role)
    ));
    wrong.kind = EmbedKind::SentenceInline;
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Category)
    ));
    let DocContent::Syntax { closure } = &mut wrong.content else {
        return Err("syntax fixture".into());
    };
    closure.syntax.category = "Inline".into();
    // Even a matching outer category cannot relabel a semantic Sentence root.
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Category)
    ));
    let mut other = surface.clone();
    other.digest.0[0] ^= 1;
    assert!(matches!(
        sentence::lower(&input, &other, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Selection)
    ));
    Ok(())
}

#[test]
fn sentence_collection_reuses_owner_checks_without_skipping_guest_validation() -> Result<(), String>
{
    use nepl3_doc_core::model::{
        BlockRef, DocKind, DocNode, DocRoot, DocValue, DocumentSyntax, EmbedRef, FlowRef,
    };
    use nepl3_suite::adapters::document::sentences;
    let (registry, surface, mut input) = input()?;
    let DocContent::Syntax { closure } = &mut input.content else {
        return Err("syntax".into());
    };
    closure.provenance = OwnerProvenance::from_parts(
        vec![Origin::Synthetic {
            reason: "shared owner".repeat(1000),
            anchor: None,
        }],
        vec![],
        vec![],
    );
    let kinds = [
        DocKind::Sentence {
            syntax: EmbedRef(0),
        },
        DocKind::Sentence {
            syntax: EmbedRef(1),
        },
        DocKind::Paragraph {
            items: vec![FlowRef(0), FlowRef(1)],
        },
    ];
    let shared = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Block(BlockRef(2)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![input.clone(), input],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let mut independent = shared.clone();
    let DocContent::Syntax { closure } = &mut independent.value.embeds[1].content else {
        return Err("syntax".into());
    };
    closure.provenance =
        OwnerProvenance::from_parts(closure.provenance.origins().to_vec(), vec![], vec![]);
    let run = |doc: &DocumentSyntax, b: &mut Budget| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        sentences::collect(doc, &surface, &[], &registry, &mut codec, b)
            .map_err(err)
            .map(|v| v.into_parts())
    };
    let mut costs = Vec::new();
    let mut outputs = Vec::new();
    for doc in [&shared, &independent] {
        let mut shape = budget();
        doc.validate_structure(&registry, &mut shape, &mut SourceAdmission::default())
            .map_err(err)?;
        let mut total = budget();
        outputs.push(run(doc, &mut total)?);
        // After complete Doc validation, lowering uses the retained guest
        // proofs. Independent owner storage therefore adds no second owner
        // validation, while the common Doc check still validates both owners.
        costs.push(
            total
                .usage()
                .work
                .checked_sub(shape.usage().work)
                .ok_or("measurement")?,
        );
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(costs[0], costs[1]);
    for callback_error in [false, true] {
        let mut stopped = budget();
        let mut visited = 0;
        let result = shared.validate_structure_with_syntax(
            &registry,
            &mut stopped,
            &mut SourceAdmission::default(),
            |_, b| {
                visited += 1;
                if visited == 2 {
                    b.cancel();
                    if callback_error {
                        return Err(nepl3_doc_core::check::StructureError::ViewOwner);
                    }
                }
                Ok(())
            },
        );
        assert_eq!(visited, 2);
        assert!(matches!(
            result,
            Err(nepl3_doc_core::check::StructureError::Stopped(
                StopReason::Cancelled
            ))
        ));
        assert_eq!(stopped.current_depth(), 0);
    }
    let parts = &outputs[0].0;
    for (index, part) in parts.iter().enumerate() {
        let mut mixed = shared.clone();
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        mixed.value.embeds[index] = sentence::embed(
            part.as_ref().ok_or("sentence")?,
            &registry,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(run(&mixed, &mut budget())?, outputs[0]);
    }
    for part in parts.iter().flatten() {
        assert_eq!(
            part.value.nodes,
            vec![
                Kind::Break,
                Kind::Sentence {
                    inlines: vec![InlineRef(0), InlineRef(0)]
                }
            ]
        );
    }
    let mut bad = shared.clone();
    let DocContent::Syntax { closure } = &mut bad.value.embeds[1].content else {
        return Err("syntax".into());
    };
    // A valid common graph with the wrong selected category still reaches
    // the lowerer's category check for the second shared-owner slot.
    closure.syntax.category = "Inline".into();
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    assert!(matches!(
        sentences::collect(&bad, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(sentences::Error::Sentence {
            embed: EmbedRef(1),
            error: Error::Category
        })
    ));
    Ok(())
}

#[test]
fn inline_slot_accepts_inline_root_and_rejects_owner_mismatch() -> Result<(), String> {
    let (registry, surface, mut input) = input()?;
    input.kind = EmbedKind::SentenceInline;
    let DocContent::Syntax { closure } = &mut input.content else {
        return Err("syntax fixture".into());
    };
    closure.syntax.category = "Inline".into();
    closure.syntax.root = NodeRef(0);
    closure.syntax.bundle.root = NodeRef(0);
    closure.syntax.bundle.nodes.truncate(1);
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let output = sentence::lower(&input, &surface, &[], &registry, &mut codec, &mut budget())
        .map_err(err)?;
    assert_eq!(output.value.root, Root::Inline(InlineRef(0)));
    assert_eq!(output.value.nodes, vec![Kind::Break]);
    let DocContent::Syntax { closure } = &mut input.content else {
        return Err("syntax fixture".into());
    };
    closure.owner_environment.id += 1;
    assert!(matches!(
        sentence::lower(&input, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Closure(_))
    ));
    Ok(())
}

#[test]
fn sentence_consumer_revalidates_budget_boundaries() -> Result<(), String> {
    let (registry, surface, input) = input()?;
    let run = |b: &mut Budget| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        Ok::<_, String>(sentence::lower(
            &input,
            &surface,
            &[],
            &registry,
            &mut codec,
            b,
        ))
    };
    let mut full = budget();
    run(&mut full)?.map_err(err)?;
    let parent_depth = 7;
    let mut nested = budget();
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    nested
        .with_depth_at_least(parent_depth, |b| {
            sentence::lower(&input, &surface, &[], &registry, &mut codec, b)
        })
        .map_err(err)?;
    assert_eq!(nested.current_depth(), 0);
    assert_eq!(nested.usage().depth, full.usage().depth + parent_depth);
    let mut limits = budget().limits();
    limits.depth = nested.usage().depth - 1;
    let mut stopped = Budget::new(limits);
    assert!(matches!(
        stopped.with_depth_at_least(parent_depth, |b| {
            sentence::lower(&input, &surface, &[], &registry, &mut codec, b)
        }),
        Err(Error::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(stopped.current_depth(), 0);
    for (used, reason) in [
        (full.usage().work, StopReason::WorkLimit),
        (full.usage().allocation_units, StopReason::AllocationLimit),
        (full.usage().depth, StopReason::DepthLimit),
    ] {
        for (limit, success) in [
            (used, true),
            (used.checked_sub(1).ok_or("zero usage")?, false),
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!("fixed boundary cases"),
            }
            let result = run(&mut Budget::new(limits))?;
            if success {
                result.map_err(err)?;
            } else {
                assert!(matches!(result, Err(Error::Stopped(actual)) if actual == reason));
            }
        }
    }
    Ok(())
}

fn generated() -> SentenceSyntax {
    // Independently specified semantic value: both entries share the same Ruby.
    // No source is fabricated for a generated Text or its reading.
    let nodes = vec![
        Kind::Text {
            text: "漢字".into(),
        },
        Kind::Text {
            text: "かんじ".into(),
        },
        Kind::Ruby {
            base: InlineRef(0),
            reading: InlineRef(1),
        },
        Kind::Sentence {
            inlines: vec![InlineRef(2), InlineRef(2)],
        },
    ];
    SentenceSyntax {
        locations: vec![
            NodeLocation {
                origin: OriginId(0),
                head: None,
                cover: None
            };
            nodes.len()
        ],
        value: SentenceValue {
            root: Root::Sentence(SentenceRef(3)),
            nodes,
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![Origin::Synthetic {
            reason: "typed authoring fixture".into(),
            anchor: None,
        }],
        views: vec![],
        source_maps: vec![],
    }
}

fn generated_embed(
    registry: &SchemaRegistry,
    codec: &mut impl FoundationValueCodec<Error: core::fmt::Debug>,
) -> Result<DocEmbed, String> {
    let encoded = nepl3_sentence_core::portable::syntax::to_value(
        &generated(),
        registry,
        codec,
        &mut budget(),
    )
    .map_err(err)?;
    let nepl3_core::value::NdfValue::Record(value) = &encoded else {
        return Err("SentenceSyntax record".into());
    };
    Ok(DocEmbed {
        kind: EmbedKind::Sentence,
        content: DocContent::Value {
            value: nepl3_core::value::TypedValue::Record(value.clone()),
        },
    })
}

#[test]
fn typed_sentence_preserves_meaning_and_rejects_invalid_boundary() -> Result<(), String> {
    let (registry, surface, _) = input()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let input = generated_embed(&registry, &mut codec)?;
    let output = sentence::lower(&input, &surface, &[], &registry, &mut codec, &mut budget())
        .map_err(err)?;
    assert_eq!(output, generated());

    let mut wrong = input.clone();
    wrong.kind = EmbedKind::SentenceInline;
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Category)
    ));
    wrong.kind = EmbedKind::InlineMath;
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Role)
    ));

    let mut wrong = input.clone();
    let DocContent::Value {
        value: nepl3_core::value::TypedValue::Record(record),
    } = &mut wrong.content
    else {
        return Err("typed record fixture".into());
    };
    record.schema.digest.0[0] ^= 1;
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Value(_))
    ));

    let mut wrong = input;
    let DocContent::Value {
        value: nepl3_core::value::TypedValue::Record(record),
    } = &mut wrong.content
    else {
        return Err("typed record fixture".into());
    };
    // Field 1 is locations. Valid NDF shape alone does not establish association
    // of each semantic node with a local Origin and optional source positions.
    record.fields[1] = nepl3_core::value::NdfValue::List(vec![]);
    assert!(matches!(
        sentence::lower(&wrong, &surface, &[], &registry, &mut codec, &mut budget()),
        Err(Error::Value(_))
    ));
    Ok(())
}

#[test]
fn typed_sentence_obeys_work_allocation_and_owning_depth() -> Result<(), String> {
    let (registry, surface, _) = input()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let input = generated_embed(&registry, &mut codec)?;
    let mut run = |b: &mut Budget| {
        b.with_depth_at_least(7, |b| {
            sentence::lower(&input, &surface, &[], &registry, &mut codec, b)
        })
    };
    let mut full = budget();
    assert_eq!(run(&mut full).map_err(err)?, generated());
    assert_eq!(full.current_depth(), 0);
    for (usage, reason) in [
        (full.usage().work, StopReason::WorkLimit),
        (full.usage().allocation_units, StopReason::AllocationLimit),
        (full.usage().depth, StopReason::DepthLimit),
    ] {
        for (limit, success) in [
            (usage, true),
            (usage.checked_sub(1).ok_or("zero usage")?, false),
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!("fixed boundary cases"),
            }
            let mut b = Budget::new(limits);
            let actual = run(&mut b);
            if success {
                assert_eq!(actual.map_err(err)?, generated());
            } else {
                assert!(matches!(actual, Err(Error::Stopped(r)) if r == reason));
            }
            assert_eq!(b.current_depth(), 0);
        }
    }
    Ok(())
}
