use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::*,
    source::SourceAdmission,
    syntax::*,
    value::SchemaRef,
};
use nepl3_sentence_core::{lower, model::*};
fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        depth: 100_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        source_bytes: 2_000_000,
        output_bytes: 2_000_000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn fixture() -> Result<(SchemaRegistry, SchemaRef, SyntaxBundle), String> {
    // Hand-authored source-less subset, separate from the production Grammar
    // test. Descriptor identities are computed, not fabricated digests.
    let descriptor = SchemaDescriptor {
        package: "test.sentence-surface".into(),
        revision: 1,
        operations: vec![],
        types: vec![
            NamedType {
                name: "Leaf:SentenceLiteral".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "Form:Break".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "Form:Emphasis".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "inline".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "NodeRef".into(),
                        }),
                    }],
                },
            },
        ],
    };
    let surface = descriptor.reference(&mut b()).map_err(err)?;
    let mut r = SchemaRegistry::default();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?;
    r.register(
        foundation.reference(&mut b()).map_err(err)?,
        foundation,
        &mut b(),
    )
    .map_err(err)?;
    r.register(surface.clone(), descriptor, &mut b())
        .map_err(err)?;
    let sentence = nepl3_sentence_core::schema::descriptor(&mut b()).map_err(err)?;
    r.register(
        sentence.reference(&mut b()).map_err(err)?,
        sentence,
        &mut b(),
    )
    .map_err(err)?;
    r.finalize(&mut b()).map_err(err)?;
    let mut nodes = vec![SyntaxNode {
        schema: surface.clone(),
        kind: "Form:Break".into(),
        fields: vec![],
        head: None,
        cover: None,
        token: None,
        origin: OriginId(0),
    }];
    for i in 0..1000 {
        nodes.push(SyntaxNode {
            schema: surface.clone(),
            kind: "Form:Emphasis".into(),
            fields: vec![FieldValue::Child(NodeRef(i))],
            head: None,
            cover: None,
            token: None,
            origin: OriginId(0),
        });
    }
    Ok((
        r,
        surface,
        SyntaxBundle {
            sources: vec![],
            nodes,
            origins: vec![Origin::Synthetic {
                reason: "test generated prefix".into(),
                anchor: None,
            }],
            root: NodeRef(1000),
            environments: vec![],
            tokens: vec![],
            source_maps: vec![],
        },
    ))
}
#[test]
fn generated_inline_projection_keeps_node_correspondence_and_revalidates_limits()
-> Result<(), String> {
    let (r, surface, bundle) = fixture()?;
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    let result = lower::prefix(
        &checked,
        &surface,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(result.value.root, Root::Inline(InlineRef(1000)));
    assert_eq!(result.value.nodes[0], Kind::Break);
    assert_eq!(
        result.value.nodes[1000],
        Kind::Emphasis {
            inline: InlineRef(999)
        }
    );
    assert_eq!(
        result.syntax_to_meaning,
        (0..=1000).map(Some).collect::<Vec<_>>()
    );
    assert!(
        bundle
            .nodes
            .iter()
            .all(|n| n.head.is_none() && n.cover.is_none())
    );
    let empty = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let syntax =
        lower::presentation::sentence(&checked, &surface, &r, &mut codec, &mut b()).map_err(err)?;
    assert_eq!(syntax.value, result.value);
    assert_eq!(syntax.origins, bundle.origins);
    assert!(
        syntax
            .locations
            .iter()
            .all(|l| l.origin == OriginId(0) && l.head.is_none() && l.cover.is_none())
    );
    let mut shallow = b().limits();
    shallow.depth = 10;
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut Budget::new(shallow),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Stopped(StopReason::DepthLimit))
    );
    let mut cancelled = b();
    cancelled.cancel();
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut cancelled,
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &SchemaRegistry::default(),
            &mut b(),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Syntax(SyntaxError::Schema(
            SchemaError::Unfinalized
        )))
    );
    Ok(())
}
#[test]
fn structurally_valid_bundle_is_not_a_proof_of_sentence_operands() -> Result<(), String> {
    let (r, surface, mut bundle) = fixture()?;
    // Foundation validates graph geometry, not the language constructor's
    // arity. Lowering must independently reject this missing inline operand.
    bundle.nodes[1000].fields.clear();
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Unsupported(NodeRef(1000)))
    );
    Ok(())
}

#[test]
fn literal_consumer_matches_owner_token_and_returns_typed_stops() -> Result<(), String> {
    use nepl3_core::{
        source::{SourceId, SourceSnapshot, SourceStore},
        value::KindRef,
        view::Token,
    };
    use nepl3_sentence_core::{literal, portable};
    let (r, surface, _) = fixture()?;
    let source = SourceSnapshot::new(
        SourceId("literal-owner".into()),
        1,
        "memory:literal-owner".into(),
        "\"[漢/かん]{語/note}\" ".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let read = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let literal::SentenceOutcome::Matched(read) = read.outcome else {
        return Err("literal match".into());
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let payload =
        portable::literal::to_value(&read.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    let mut bundle = SyntaxBundle {
        sources: vec![source.clone()],
        nodes: vec![SyntaxNode {
            schema: surface.clone(),
            kind: "Leaf:SentenceLiteral".into(),
            fields: vec![],
            head: Some(read.head.clone()),
            cover: Some(read.head.clone()),
            origin: OriginId(0),
            token: Some(TokenRef(0)),
        }],
        origins: vec![Origin::Direct(read.head.clone())],
        root: NodeRef(0),
        environments: vec![],
        source_maps: vec![],
        tokens: vec![Token {
            kind: KindRef {
                schema: surface.clone(),
                local_kind: 0, // first descriptor type in the explicit fixture
            },
            head: read.head.clone(),
            payload,
            views: read.syntax.views[0].view.clone(),
            leading_trivia: vec![],
        }],
    };
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::presentation::sentence(&checked, &surface, &r, &mut codec, &mut b()).map_err(err)?,
        read.syntax
    );
    for (limits, cancel, expected) in [
        (
            {
                let mut l = b().limits();
                l.work = 0;
                l
            },
            false,
            StopReason::WorkLimit,
        ),
        (
            {
                let mut l = b().limits();
                l.depth = 1;
                l
            },
            false,
            StopReason::DepthLimit,
        ),
        (b().limits(), true, StopReason::Cancelled),
    ] {
        let mut budget = Budget::new(limits);
        if cancel {
            budget.cancel();
        }
        assert_eq!(
            lower::literal::sentence(&checked, &surface, &r, &mut codec, &mut budget).err(),
            Some(lower::literal::Error::Stopped(expected))
        );
        assert_eq!(budget.current_depth(), 0);
    }
    let mut wrong = surface.clone();
    wrong.revision += 1;
    assert!(matches!(
        lower::literal::sentence(&checked, &wrong, &r, &mut codec, &mut b()),
        Err(lower::literal::Error::Syntax(lower::Error::Unsupported(_)))
    ));
    bundle.nodes[0].token = None;
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::literal::sentence(&checked, &surface, &r, &mut codec, &mut b()).err(),
        Some(lower::literal::Error::TokenMismatch(NodeRef(0)))
    );
    bundle.nodes[0].token = Some(TokenRef(0));
    bundle.nodes[0].cover = Some(source.span(0, source.text().len() as u64).map_err(err)?);
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::literal::sentence(&checked, &surface, &r, &mut codec, &mut b()).err(),
        Some(lower::literal::Error::TokenMismatch(NodeRef(0)))
    );
    Ok(())
}
