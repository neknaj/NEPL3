use super::*;
use nepl3_core::budget::Limits;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        depth: 100,
        ..Limits::default()
    })
}

#[test]
fn native_structure_proof_reuses_validation_and_keeps_labels() -> Result<(), alloc::string::String>
{
    let err = |e| alloc::format!("{e:?}");
    let mut registry = SchemaRegistry::default();
    let descriptor = crate::schema::descriptor(&mut budget()).map_err(err)?;
    registry
        .register(
            descriptor.reference(&mut budget()).map_err(err)?,
            descriptor,
            &mut budget(),
        )
        .map_err(err)?;
    // Foundation types referenced by the Doc schema remain part of validation.
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?;
    registry
        .register(
            descriptor.reference(&mut budget()).map_err(err)?,
            descriptor,
            &mut budget(),
        )
        .map_err(err)?;
    registry.finalize(&mut budget()).map_err(err)?;
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(2),
        },
        DocKind::Sentence { inlines: vec![] },
        DocKind::Body {
            blocks: vec![BlockRef(3)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(4)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(5)],
        },
        DocKind::Anchor {
            id: "entry".into(),
            label: InlineRef(6),
        },
        DocKind::Text {
            text: "entry".into(),
        },
    ];
    let document = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Article(ArticleRef(0)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let mut admission = SourceAdmission::default();
    let mut full = budget();
    let ordinary = check(&document, &registry, &mut full, &mut admission)
        .map_err(|e| alloc::format!("{e:?}"))?;
    let proof = document
        .validate_structure(&registry, &mut budget(), &mut admission)
        .map_err(|e| alloc::format!("{e:?}"))?;
    let mut reused = budget();
    let actual = check_structure(&proof, &mut reused).map_err(|e| alloc::format!("{e:?}"))?;
    assert_eq!(actual.definitions(), ordinary.definitions());
    assert_eq!(actual.references(), ordinary.references());
    assert_eq!(actual.definitions()[0].name, "entry");
    assert!(core::ptr::eq(actual.document(), &document));
    assert!(reused.usage().work < full.usage().work);
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        check_structure(&proof, &mut stopped),
        Err(LabelError::Stopped(StopReason::WorkLimit))
    ));
    Ok(())
}
