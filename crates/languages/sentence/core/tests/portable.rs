use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, TypeDescriptor},
    source::{SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_sentence_core::{check, model::*, portable};
use nepl3_wire::foundation::FoundationCodec;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        nodes: 1_000_000,
        depth: 100_000,
        allocation_units: 500_000_000,
        output_bytes: 100_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()),
        nepl3_sentence_core::schema::descriptor(&mut budget()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
            .map_err(err)?;
    }
    r.finalize(&mut budget()).map_err(err)?;
    Ok(r)
}

#[test]
fn typed_foreign_value_roundtrip_preserves_identity_and_requires_variant() -> Result<(), String> {
    use nepl3_core::value::{Record, TypedValue};
    let r = registry()?;
    let foundation = r
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let value = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::ForeignInline {
            syntax: EmbedRef(0),
        }],
        embeds: vec![InlineContent::Value {
            value: TypedValue::Record(Record {
                schema: foundation,
                kind: "NodeRef".into(),
                fields: vec![NdfValue::U64(17)],
            }),
        }],
    };
    let raw = encode(&value, &r)?;
    let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    assert_eq!(decode(&received, &r, &mut budget()).map_err(err)?, value);
    let NdfValue::Record(record) = &raw else {
        return Err("sentence record".into());
    };
    let NdfValue::List(embeds) = &record.fields[2] else {
        return Err("embeds".into());
    };
    let NdfValue::Variant(content) = &embeds[0] else {
        return Err("content variant".into());
    };
    assert_eq!(content.type_name, "InlineContent");
    assert_eq!(content.variant, "Value");
    assert_eq!(
        content.schema,
        *r.selected("nepl3.sentence", 1).ok_or("sentence")?
    );
    let NdfValue::Record(guest) = &content.fields[0] else {
        return Err("guest record".into());
    };
    assert_eq!(guest.kind, "NodeRef");
    assert_eq!(guest.fields, [NdfValue::U64(17)]);
    for replacement in [content.fields[0].clone(), NdfValue::Text("untyped".into())] {
        let mut forged = record.clone();
        forged.fields[2] = NdfValue::List(vec![replacement]);
        assert!(decode(&NdfValue::Record(forged), &r, &mut budget()).is_err());
    }
    let mut forged = value.clone();
    let InlineContent::Value {
        value: TypedValue::Record(guest),
    } = &mut forged.embeds[0]
    else {
        return Err("typed guest".into());
    };
    guest.schema.digest = nepl3_core::source::Digest::of(b"wrong guest schema");
    assert!(encode(&forged, &r).is_err());

    // Reconstruct the prior descriptor contract: embeds were an untagged
    // ForeignClosure list. Its actual descriptor digest must remain rejected.
    use nepl3_core::schema::{TypeRef, TypeShape};
    let mut old = nepl3_sentence_core::schema::descriptor(&mut budget()).map_err(err)?;
    old.types.retain(|ty| ty.name != "InlineContent");
    let TypeShape::Record { fields } = &mut old
        .types
        .iter_mut()
        .find(|ty| ty.name == "SentenceValue")
        .ok_or("SentenceValue")?
        .shape
    else {
        return Err("record shape".into());
    };
    fields
        .iter_mut()
        .find(|field| field.name == "embeds")
        .ok_or("embeds field")?
        .ty = TypeDescriptor::List(Box::new(TypeDescriptor::Named(TypeRef {
        package: "nepl3.foundation".into(),
        revision: 1,
        name: "ForeignClosure".into(),
    })));
    let mut prior = record.clone();
    prior.schema = old.reference(&mut budget()).map_err(err)?;
    assert_ne!(prior.schema, record.schema);
    assert!(decode(&NdfValue::Record(prior), &r, &mut budget()).is_err());
    Ok(())
}
fn sentence() -> SentenceValue {
    SentenceValue {
        root: Root::Sentence(SentenceRef(0)),
        nodes: vec![
            Kind::Sentence {
                inlines: (1..=9).map(InlineRef).collect(),
            },
            Kind::Text {
                text: "漢\n𝄞é\"\\".into(),
            },
            Kind::Code {
                text: "<script>& literal".into(),
            },
            Kind::Concat {
                inlines: vec![InlineRef(1), InlineRef(2)],
            },
            Kind::Ruby {
                base: InlineRef(1),
                reading: InlineRef(2),
            },
            Kind::InlineAnno {
                base: InlineRef(4),
                notes: vec![InlineRef(2), InlineRef(1)],
            },
            Kind::Emphasis {
                inline: InlineRef(3),
            },
            Kind::Strong {
                inline: InlineRef(6),
            },
            Kind::Break,
            Kind::ExternalLink {
                uri: "https://example.invalid/漢".into(),
                label: InlineRef(1),
            },
        ],
        embeds: vec![],
    }
}
fn encode(v: &SentenceValue, r: &SchemaRegistry) -> Result<NdfValue, String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
    portable::to_value(v, r, &mut c, &mut budget()).map_err(err)
}
fn decode(
    v: &NdfValue,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<SentenceValue, portable::Error<nepl3_wire::WireError>> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c =
        FoundationCodec::new(r, &empty, &mut admission).map_err(portable::Error::Foundation)?;
    portable::from_value(v, r, &mut c, b)
}
fn node_mut(v: &mut NdfValue, id: usize) -> Result<&mut nepl3_core::value::Variant, String> {
    let NdfValue::Record(record) = v else {
        return Err("value record".into());
    };
    let Some(NdfValue::List(nodes)) = record.fields.get_mut(1) else {
        return Err("nodes".into());
    };
    let Some(NdfValue::Variant(node)) = nodes.get_mut(id) else {
        return Err("node".into());
    };
    Ok(node)
}
fn set_index(v: &mut NdfValue, index: u64) -> Result<(), String> {
    let NdfValue::Record(r) = v else {
        return Err("reference record".into());
    };
    let [field] = r.fields.as_mut_slice() else {
        return Err("reference field".into());
    };
    *field = NdfValue::U64(index);
    Ok(())
}

#[test]
fn first_cbor_receiver_retains_all_local_forms_order_unicode_and_sharing() -> Result<(), String> {
    let r = registry()?;
    let v = sentence();
    let raw = encode(&v, &r)?;
    let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let actual = decode(&received, &r, &mut budget()).map_err(err)?;
    assert_eq!(actual, v);
    assert_eq!(
        nepl3_wire::encode(&encode(&actual, &r)?, &mut budget()).map_err(err)?,
        bytes
    );
    // Wire field order is specified independently of the Rust model declaration.
    let mut inspected = raw;
    let ruby = node_mut(&mut inspected, 4)?;
    assert_eq!(ruby.variant, "Ruby");
    for (field, expected) in ruby.fields.iter().zip([1, 2]) {
        let NdfValue::Record(reference) = field else {
            return Err("InlineRef".into());
        };
        assert_eq!(reference.kind, "InlineRef");
        assert_eq!(reference.fields, vec![NdfValue::U64(expected)]);
    }
    let empty = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::Text {
            text: String::new(),
        }],
        embeds: vec![],
    };
    assert_eq!(
        decode(&encode(&empty, &r)?, &r, &mut budget()).map_err(err)?,
        empty
    );
    Ok(())
}

#[test]
fn schema_valid_wire_still_rejects_cycles_references_and_empty_annotation_parts()
-> Result<(), String> {
    let r = registry()?;
    let raw = encode(&sentence(), &r)?;
    let mut cycle = raw.clone();
    set_index(&mut node_mut(&mut cycle, 6)?.fields[0], 7)?;
    let mut missing = raw.clone();
    set_index(&mut node_mut(&mut missing, 6)?.fields[0], u64::MAX)?;
    let mut category = raw.clone();
    set_index(&mut node_mut(&mut category, 6)?.fields[0], 0)?;
    let mut empty = raw.clone();
    node_mut(&mut empty, 1)?.fields[0] = NdfValue::Text(String::new());
    let mut notes = raw.clone();
    node_mut(&mut notes, 5)?.fields[1] = NdfValue::List(vec![]);
    for bad in [cycle, missing, category, empty, notes] {
        r.validate(&TypeDescriptor::TypedValue, &bad, &mut budget())
            .map_err(err)?;
        let before = bad.clone();
        assert!(matches!(
            decode(&bad, &r, &mut budget()),
            Err(portable::Error::Structure(_))
        ));
        assert_eq!(bad, before);
    }
    let mut native = sentence();
    native.nodes.push(Kind::Break);
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    assert!(matches!(
        portable::to_value(&native, &r, &mut c, &mut budget()),
        Err(portable::Error::Structure(check::Error::Unreachable(10)))
    ));
    Ok(())
}

#[test]
fn malformed_tags_field_counts_and_revision_are_not_accepted_as_sentence() -> Result<(), String> {
    let r = registry()?;
    let raw = encode(&sentence(), &r)?;
    let mut bad_tag = raw.clone();
    node_mut(&mut bad_tag, 4)?.variant = "Annotated".into();
    let mut bad_fields = raw.clone();
    node_mut(&mut bad_fields, 4)?.fields.pop();
    let mut bad_revision = raw.clone();
    node_mut(&mut bad_revision, 4)?.schema.revision += 1;
    let mut bad_kind = raw;
    let NdfValue::Record(ruby_ref) = &mut node_mut(&mut bad_kind, 4)?.fields[0] else {
        return Err("ref".into());
    };
    ruby_ref.kind = "SentenceRef".into();
    for bad in [bad_tag, bad_fields, bad_revision, bad_kind] {
        assert!(matches!(
            decode(&bad, &r, &mut budget()),
            Err(portable::Error::Schema(_))
        ));
    }
    Ok(())
}

#[test]
fn stopping_encode_and_decode_returns_no_partial_value_and_keeps_budget_stopped()
-> Result<(), String> {
    let r = registry()?;
    let v = sentence();
    let raw = encode(&v, &r)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let make_budget = || {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::NodeLimit => limits.nodes = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::DepthLimit => limits.depth = 0,
                _ => {}
            }
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.cancel();
            }
            b
        };
        let mut b = make_budget();
        assert_eq!(
            portable::to_value(&v, &r, &mut c, &mut b),
            Err(portable::Error::Stopped(reason))
        );
        assert_eq!(b.poll(), Err(reason));
        let mut b = make_budget();
        assert_eq!(
            decode(&raw, &r, &mut b),
            Err(portable::Error::Stopped(reason))
        );
        assert_eq!(b.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn altered_descriptor_with_same_package_revision_is_rejected() -> Result<(), String> {
    let mut r = SchemaRegistry::default();
    let mut sentence = nepl3_sentence_core::schema::descriptor(&mut budget()).map_err(err)?;
    sentence.types[0]
        .constraints
        .push("different-contract".into());
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
        sentence,
    ] {
        r.register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
            .map_err(err)?;
    }
    r.finalize(&mut budget()).map_err(err)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    assert_eq!(
        portable::to_value(&self::sentence(), &r, &mut c, &mut budget()),
        Err(portable::Error::SchemaIdentity)
    );
    Ok(())
}

#[test]
fn foreign_cbor_closes_sources_and_rejects_missing_or_forged_owner_data() -> Result<(), String> {
    use nepl3_core::{
        origin::{Origin, OriginId},
        source::{SourceId, SourceSnapshot},
        syntax::*,
    };
    let r = registry()?;
    let schema = r
        .selected("nepl3.foundation", 1)
        .ok_or("foundation schema")?
        .clone();
    let source = SourceSnapshot::new(
        SourceId("sentence-guest".into()),
        7,
        "memory:sentence-guest".into(),
        "漢𝄞".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let owner_span = source.span(0, 3).map_err(err)?;
    let guest_span = source.span(3, 7).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest =
        nepl3_wire::environment::environment_digest(&environment, &schema, &r, &mut budget())
            .map_err(err)?;
    // This foundation NodeRef kind exercises syntax transport, not a language
    // form or a claim that any guest parser/typechecker ran.
    let closure = ForeignClosure {
        syntax: ForeignSyntax {
            schema: schema.clone(),
            category: "test-inline".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                sources: vec![source.clone()],
                nodes: vec![SyntaxNode {
                    schema,
                    kind: "NodeRef".into(),
                    fields: vec![],
                    head: Some(guest_span.clone()),
                    cover: Some(guest_span.clone()),
                    origin: OriginId(0),
                    token: None,
                }],
                origins: vec![Origin::Direct(guest_span)],
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 3, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 3,
            digest,
            value: environment,
        },
        provenance: nepl3_core::syntax::OwnerProvenance::from_parts(
            vec![Origin::Direct(owner_span)],
            vec![source.clone()],
            vec![],
        ),
    };
    let value = SentenceValue {
        root: Root::Sentence(SentenceRef(0)),
        nodes: vec![
            Kind::Sentence {
                inlines: vec![InlineRef(1), InlineRef(2)],
            },
            Kind::ForeignInline {
                syntax: EmbedRef(0),
            },
            Kind::ForeignInline {
                syntax: EmbedRef(0),
            },
        ],
        embeds: vec![closure.into()],
    };
    {
        use nepl3_sentence_core::text::{self, AnnotationPolicy::BaseOnly, Error};
        let proof = text::prepare(&value, &r, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert_eq!(
            proof.render(BaseOnly, &[], &mut budget()),
            Err(Error::Unresolved(EmbedRef(0)))
        );
        let supplied = proof
            .resolve(EmbedRef(0), "𝄞", &mut budget())
            .map_err(err)?;
        assert_eq!(
            proof
                .render(BaseOnly, &[supplied], &mut budget())
                .map_err(err)?,
            "𝄞𝄞"
        );
        let duplicate = [
            proof
                .resolve(EmbedRef(0), "a", &mut budget())
                .map_err(err)?,
            proof
                .resolve(EmbedRef(0), "b", &mut budget())
                .map_err(err)?,
        ];
        assert_eq!(
            proof.render(BaseOnly, &duplicate, &mut budget()),
            Err(Error::Duplicate(EmbedRef(0)))
        );
        assert!(matches!(
            proof.resolve(EmbedRef(1), "", &mut budget()),
            Err(Error::Embed(EmbedRef(1)))
        ));
        let other = value.clone();
        let other_proof = text::prepare(&other, &r, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let supplied = other_proof
            .resolve(EmbedRef(0), "wrong owner", &mut budget())
            .map_err(err)?;
        assert_eq!(
            proof.render(BaseOnly, &[supplied], &mut budget()),
            Err(Error::WrongScope)
        );
        let mut stopped = budget();
        stopped.cancel();
        assert!(matches!(
            proof.resolve(EmbedRef(0), "", &mut stopped),
            Err(Error::Stopped(StopReason::Cancelled))
        ));
    }
    let raw = encode(&value, &r)?;
    let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
    let decoded = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let mut b = budget();
    let actual = decode(&decoded, &r, &mut b).map_err(err)?;
    assert_eq!(actual, value);
    assert_eq!(b.usage().source_bytes, 7);
    assert_ne!(
        actual.embeds[0]
            .syntax()
            .ok_or("syntax content")?
            .provenance
            .origins(),
        actual.embeds[0]
            .syntax()
            .ok_or("syntax content")?
            .syntax
            .bundle
            .origins
    );
    let mut bad_native = value.clone();
    let InlineContent::Syntax { closure } = &mut bad_native.embeds[0] else {
        return Err("syntax content".into());
    };
    closure.provenance = nepl3_core::syntax::OwnerProvenance::from_parts(
        closure.provenance.origins().to_vec(),
        vec![],
        closure.provenance.source_maps().to_vec(),
    );
    assert!(encode(&bad_native, &r).is_err());
    {
        use nepl3_sentence_core::text::{self, AnnotationPolicy::*, Error};
        let mut annotated = value.clone();
        annotated.root = Root::Inline(InlineRef(2));
        annotated.nodes = vec![
            Kind::Text {
                text: "base".into(),
            },
            Kind::ForeignInline {
                syntax: EmbedRef(0),
            },
            Kind::InlineAnno {
                base: InlineRef(0),
                notes: vec![InlineRef(1)],
            },
        ];
        let prepared = text::prepare(
            &annotated,
            &r,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        assert_eq!(
            prepared.render(BaseOnly, &[], &mut budget()).map_err(err)?,
            "base"
        );
        assert_eq!(
            prepared.render(WithAllNotes, &[], &mut budget()),
            Err(Error::Unresolved(EmbedRef(0)))
        );
        annotated.nodes[2] = Kind::Ruby {
            base: InlineRef(0),
            reading: InlineRef(1),
        };
        let prepared = text::prepare(
            &annotated,
            &r,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        assert_eq!(
            prepared.render(BaseOnly, &[], &mut budget()).map_err(err)?,
            "base"
        );
        assert_eq!(
            prepared.render(WithReadings, &[], &mut budget()),
            Err(Error::Unresolved(EmbedRef(0)))
        );
        // Even a reading excluded from output must have a valid source closure.
        let InlineContent::Syntax { closure } = &mut annotated.embeds[0] else {
            return Err("syntax content".into());
        };
        closure.provenance = nepl3_core::syntax::OwnerProvenance::from_parts(
            closure.provenance.origins().to_vec(),
            vec![],
            closure.provenance.source_maps().to_vec(),
        );
        assert!(matches!(
            text::prepare(
                &annotated,
                &r,
                &mut budget(),
                &mut SourceAdmission::default()
            ),
            Err(Error::Shape(_))
        ));
    }

    for remove_source in [true, false] {
        let mut bad = raw.clone();
        let NdfValue::Record(record) = &mut bad else {
            return Err("record".into());
        };
        let NdfValue::List(embeds) = &mut record.fields[2] else {
            return Err("embeds".into());
        };
        let NdfValue::Variant(content) = &mut embeds[0] else {
            return Err("inline content".into());
        };
        let NdfValue::Record(closure) = &mut content.fields[0] else {
            return Err("closure".into());
        };
        if remove_source {
            closure.fields[3] = NdfValue::List(vec![]);
        } else {
            let NdfValue::Record(env) = &mut closure.fields[1] else {
                return Err("environment".into());
            };
            env.fields[1] = NdfValue::Bytes(vec![0; 32]);
        }
        r.validate(&TypeDescriptor::TypedValue, &bad, &mut budget())
            .map_err(err)?;
        // Even a receiver with ambient source bytes cannot repair the missing
        // owner closure or authorize a forged environment digest.
        let mut ambient = SourceStore::default();
        ambient.insert(source.clone()).map_err(err)?;
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &ambient, &mut a).map_err(err)?;
        assert!(matches!(
            portable::from_value(&bad, &r, &mut c, &mut budget()),
            Err(portable::Error::Foundation(_))
        ));
    }
    // Shared guest reached first on a short path, then through forty wrappers.
    // Each arena fits depth 60 in isolation; their composition does not.
    let mut nested = value.clone();
    nested.nodes = vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1), InlineRef(2)],
        },
        Kind::ForeignInline {
            syntax: EmbedRef(0),
        },
    ];
    for i in 2..42 {
        nested.nodes.push(Kind::Strong {
            inline: InlineRef(i + 1),
        });
    }
    nested.nodes.push(Kind::ForeignInline {
        syntax: EmbedRef(0),
    });
    let InlineContent::Syntax { closure } = &mut nested.embeds[0] else {
        return Err("syntax content".into());
    };
    let guest = &mut closure.syntax.bundle;
    let template = guest.nodes[0].clone();
    for i in 0..29 {
        guest.nodes[i].fields = vec![FieldValue::Child(NodeRef(i as u64 + 1))];
        guest.nodes.push(template.clone());
    }
    for node in &mut guest.nodes {
        node.head = None;
        node.cover = None;
    }
    let mut limits = budget().limits();
    limits.depth = 60;
    nested
        .validate_shape(&mut Budget::new(limits))
        .map_err(err)?;
    nested.embeds[0]
        .syntax()
        .ok_or("syntax content")?
        .validate(
            &r,
            &mut Budget::new(limits),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
    let nested_raw = encode(&nested, &r)?;
    r.validate(
        &TypeDescriptor::TypedValue,
        &nested_raw,
        &mut Budget::new(limits),
    )
    .map_err(err)?;
    let mut b = Budget::new(limits);
    assert_eq!(
        decode(&nested_raw, &r, &mut b),
        Err(portable::Error::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let mut b = Budget::new(limits);
    assert_eq!(
        portable::to_value(&nested, &r, &mut c, &mut b),
        Err(portable::Error::Stopped(StopReason::DepthLimit))
    );
    let mut b = budget();
    b.with_depth_at_least::<_, check::Error>(7, |b| {
        nested
            .validate_shape(b)?
            .validate_foreign(&r, b, &mut SourceAdmission::default())?;
        assert_eq!(b.current_depth(), 7);
        Ok(())
    })
    .map_err(err)?;
    assert!(b.usage().depth >= 7 + 42 + 30);
    assert_eq!(b.current_depth(), 0);

    let mut limits = budget().limits();
    limits.source_bytes = 0;
    let mut b = Budget::new(limits);
    assert_eq!(
        decode(&raw, &r, &mut b),
        Err(portable::Error::Stopped(StopReason::SourceLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::SourceLimit));
    Ok(())
}

#[test]
fn deep_flat_wire_arena_does_not_hide_semantic_depth() -> Result<(), String> {
    let r = registry()?;
    let mut nodes: Vec<_> = (1..=10_000)
        .map(|i| Kind::Strong {
            inline: InlineRef(i),
        })
        .collect();
    nodes.push(Kind::Text { text: "end".into() });
    let v = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes,
        embeds: vec![],
    };
    let raw = encode(&v, &r)?;
    let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
    let wire = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let mut limits = budget().limits();
    limits.depth = 100;
    let mut b = Budget::new(limits);
    assert_eq!(
        decode(&wire, &r, &mut b),
        Err(portable::Error::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(decode(&wire, &r, &mut budget()).map_err(err)?, v);
    Ok(())
}

#[test]
fn independently_authored_wire_preserves_ordered_annotation_fields() -> Result<(), String> {
    use nepl3_core::value::{Record, Variant};
    let r = registry()?;
    let schema = r.selected("nepl3.sentence", 1).ok_or("sentence schema")?;
    let reference = |kind: &str, index| {
        NdfValue::Record(Record {
            schema: schema.clone(),
            kind: kind.into(),
            fields: vec![NdfValue::U64(index)],
        })
    };
    let refs = |indices: &[u64]| {
        NdfValue::List(indices.iter().map(|i| reference("InlineRef", *i)).collect())
    };
    let variant = |ty: &str, case: &str, fields| {
        NdfValue::Variant(Variant {
            schema: schema.clone(),
            type_name: ty.into(),
            variant: case.into(),
            fields,
        })
    };
    // These ordered fields come from spec23/interfaces/sentence.json, not from
    // portable::to_value. Different base/reading and notes expose swapped fields.
    let nodes = vec![
        variant(
            "SentenceKind",
            "Sentence",
            vec![refs(&[1, 2, 3, 4, 5, 6, 7, 8, 9])],
        ),
        variant(
            "SentenceKind",
            "Text",
            vec![NdfValue::Text("漢\n𝄞é\"\\".into())],
        ),
        variant(
            "SentenceKind",
            "Code",
            vec![NdfValue::Text("<script>& literal".into())],
        ),
        variant("SentenceKind", "Concat", vec![refs(&[1, 2])]),
        variant(
            "SentenceKind",
            "Ruby",
            vec![reference("InlineRef", 1), reference("InlineRef", 2)],
        ),
        variant(
            "SentenceKind",
            "InlineAnno",
            vec![reference("InlineRef", 4), refs(&[2, 1])],
        ),
        variant("SentenceKind", "Emphasis", vec![reference("InlineRef", 3)]),
        variant("SentenceKind", "Strong", vec![reference("InlineRef", 6)]),
        variant("SentenceKind", "Break", vec![]),
        variant(
            "SentenceKind",
            "ExternalLink",
            vec![
                NdfValue::Text("https://example.invalid/漢".into()),
                reference("InlineRef", 1),
            ],
        ),
    ];
    let wire = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "SentenceValue".into(),
        fields: vec![
            variant(
                "SentenceRoot",
                "Sentence",
                vec![reference("SentenceRef", 0)],
            ),
            NdfValue::List(nodes),
            NdfValue::List(vec![]),
        ],
    });
    let bytes = nepl3_wire::encode(&wire, &mut budget()).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    assert_eq!(
        decode(&received, &r, &mut budget()).map_err(err)?,
        sentence()
    );
    assert_eq!(encode(&sentence(), &r)?, wire);
    Ok(())
}

#[test]
fn nonzero_work_exhaustion_during_boundary_processing_never_returns_partial_success()
-> Result<(), String> {
    let r = registry()?;
    let mut v = sentence();
    v.nodes[1] = Kind::Text {
        text: "long annotation".repeat(10_000),
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut sent = budget();
    let wire = portable::to_value(&v, &r, &mut c, &mut sent).map_err(err)?;
    let mut received = budget();
    assert_eq!(decode(&wire, &r, &mut received).map_err(err)?, v);
    for sending in [true, false] {
        let total = if sending {
            sent.usage().work
        } else {
            received.usage().work
        };
        for work in [total / 4, total / 2, total - 1] {
            let mut limits = budget().limits();
            limits.work = work;
            let mut b = Budget::new(limits);
            if sending {
                assert_eq!(
                    portable::to_value(&v, &r, &mut c, &mut b),
                    Err(portable::Error::Stopped(StopReason::WorkLimit))
                );
            } else {
                assert_eq!(
                    decode(&wire, &r, &mut b),
                    Err(portable::Error::Stopped(StopReason::WorkLimit))
                );
            }
            assert!(b.usage().work > 0);
            assert!(b.usage().allocation_units > 0);
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        }
    }
    Ok(())
}
