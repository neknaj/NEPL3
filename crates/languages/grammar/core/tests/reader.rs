use nepl3_core::{budget::*, schema::*, source::*, value::*};
use nepl3_grammar_core::{
    compile::{self, CompileError, ReaderContext},
    model::*,
};
use nepl3_reader::plan::ReaderExpr;
type TestResult = Result<(), String>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn fixture() -> Result<(Document, SchemaRegistry, SchemaRef), String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()),
        nepl3_reader::schema::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let reference = descriptor
            .reference(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
    }
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("reader")?
        .clone();
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("grammar-test".into()),
        0,
        "memory:grammar-test".into(),
        b"language G 1 Root cons reader r repeat 1 2 scalar range \"a\" \"z\" nil".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let span = |text: &str| -> Result<Span, String> {
        let start = source.text().find(text).ok_or("fixture token")?;
        source
            .span(start as u64, (start + text.len()) as u64)
            .map_err(|e| format!("{e:?}"))
    };
    let cover = |start: &str, end: &str| -> Result<Span, String> {
        let start = source.text().find(start).ok_or("fixture start")?;
        let end = source.text().find(end).ok_or("fixture end")? + end.len();
        source
            .span(start as u64, end as u64)
            .map_err(|e| format!("{e:?}"))
    };
    let nodes = vec![
        Node {
            span: cover("language", "nil")?,
            kind: NodeKind::Language {
                name: Located {
                    value: "G".into(),
                    span: span("G")?,
                },
                revision: Located {
                    value: Integer::from(1u64),
                    span: span("1")?,
                },
                root: Located {
                    value: "Root".into(),
                    span: span("Root")?,
                },
                declarations: NodeList {
                    items: vec![NodeId(1)],
                    span: cover("cons", "nil")?,
                },
            },
        },
        Node {
            span: cover("reader", "\"z\"")?,
            kind: NodeKind::Reader {
                name: Located {
                    value: "r".into(),
                    span: source.span(30, 31).map_err(|e| format!("{e:?}"))?,
                },
                expression: NodeId(2),
            },
        },
        Node {
            span: cover("repeat", "\"z\"")?,
            kind: NodeKind::Repeat {
                min: Located {
                    value: Integer::from(1u64),
                    span: source.span(39, 40).map_err(|e| format!("{e:?}"))?,
                },
                max: Located {
                    value: Integer::from(2u64),
                    span: span("2")?,
                },
                body: NodeId(3),
            },
        },
        Node {
            span: cover("scalar", "\"z\"")?,
            kind: NodeKind::Scalar { class: NodeId(4) },
        },
        Node {
            span: cover("range", "\"z\"")?,
            kind: NodeKind::Range {
                lo: Located {
                    value: "a".into(),
                    span: span("\"a\"")?,
                },
                hi: Located {
                    value: "z".into(),
                    span: span("\"z\"")?,
                },
            },
        },
    ];
    Ok((
        Document {
            sources: vec![source],
            nodes,
            root: NodeId(0),
        },
        registry,
        schema,
    ))
}
fn run(
    document: &Document,
    registry: &SchemaRegistry,
    schema: &SchemaRef,
) -> Result<nepl3_reader::plan::ReaderPlan, CompileError> {
    let mut b = budget();
    let checked = document.validate(&mut b, &mut SourceAdmission::default())?;
    compile::reader::compile(
        &checked,
        &ReaderContext {
            schema,
            state_type: &TypeDescriptor::Unit,
            imports: &[],
            classes: &[],
            views: &[],
            registry,
        },
        &mut b,
    )
}
#[test]
fn reader_compiler_lowers_real_plan_and_rejects_natural_range_repeat_and_reference_errors()
-> TestResult {
    let (doc, registry, schema) = fixture()?;
    let NodeKind::Reader { name, .. } = &doc.nodes[1].kind else {
        return Err("reader fixture".into());
    };
    assert_eq!(
        doc.sources[0]
            .slice(&name.span)
            .map_err(|e| format!("{e:?}"))?,
        name.value
    );
    let NodeKind::Repeat { min, max, .. } = &doc.nodes[2].kind else {
        return Err("repeat fixture".into());
    };
    assert_eq!(
        doc.sources[0]
            .slice(&min.span)
            .map_err(|e| format!("{e:?}"))?,
        "1"
    );
    assert_eq!(min.value, Integer::from(1u64));
    assert_eq!(
        doc.sources[0]
            .slice(&max.span)
            .map_err(|e| format!("{e:?}"))?,
        "2"
    );
    assert_eq!(max.value, Integer::from(2u64));
    let plan = run(&doc, &registry, &schema).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        plan.expressions[0],
        ReaderExpr::Repeat { min: 1, max: 2, .. }
    ));
    assert_eq!(
        plan.rules[0].output,
        TypeDescriptor::List(Box::new(TypeDescriptor::Text))
    );
    plan.check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut bad = doc.clone();
    if let NodeKind::Repeat { min, .. } = &mut bad.nodes[2].kind {
        min.value = Integer::from_canonical(false, &[1, 0, 0, 0, 0, 0, 0, 0, 0])
            .map_err(|e| format!("{e:?}"))?;
    }
    assert!(matches!(
        run(&bad, &registry, &schema),
        Err(CompileError::NaturalOverflow)
    ));
    let mut bad = doc.clone();
    if let NodeKind::Repeat { min, .. } = &mut bad.nodes[2].kind {
        min.value = Integer::from(3u64);
    }
    assert!(matches!(
        run(&bad, &registry, &schema),
        Err(CompileError::InvalidRepeat)
    ));
    for wrong in ["", "ab", "🙂🙂"] {
        let mut bad = doc.clone();
        if let NodeKind::Range { lo, .. } = &mut bad.nodes[4].kind {
            lo.value = wrong.into();
        }
        assert!(matches!(
            run(&bad, &registry, &schema),
            Err(CompileError::InvalidRange)
        ));
    }
    let mut bad = doc.clone();
    let literal = Located {
        value: "missing".into(),
        span: bad.nodes[2].span.clone(),
    };
    bad.nodes[2].kind = NodeKind::Ref {
        name: literal.clone(),
    };
    bad.nodes.truncate(3);
    assert!(matches!(
        run(&bad, &registry, &schema),
        Err(CompileError::MissingRule)
    ));
    bad.nodes[2].kind = NodeKind::Call { provider: literal };
    assert!(matches!(
        run(&bad, &registry, &schema),
        Err(CompileError::MissingProvider)
    ));
    Ok(())
}

#[test]
fn compiler_catalogs_reject_ambiguous_and_empty_names_and_ast_charges_nodes() -> TestResult {
    use compile::{Catalog, NamedClass, NamedView, ReaderImport};
    use nepl3_core::view::{FallbackRole, PresentationClass};
    let (doc, registry, schema) = fixture()?;
    let mut limited = budget().limits();
    limited.nodes = 0;
    assert!(matches!(
        doc.validate(&mut Budget::new(limited), &mut SourceAdmission::default()),
        Err(ModelError::Stopped(StopReason::NodeLimit))
    ));
    let mut b = budget();
    let checked = doc
        .validate(&mut b, &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(b.usage().nodes, doc.nodes.len() as u64);
    let class = NamedClass {
        name: "role".into(),
        class: PresentationClass {
            schema: schema.clone(),
            name: "role".into(),
            fallback: FallbackRole::Content,
        },
    };
    let view = NamedView {
        name: "view".into(),
        kind: KindRef {
            schema: schema.clone(),
            local_kind: 0,
        },
    };
    let import = ReaderImport {
        provider: "name".into(),
        signature: nepl3_reader::builtin::provider::signature(
            nepl3_reader::builtin::BuiltinReader::Name,
            &registry,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?,
    };
    for catalog in [Catalog::Imports, Catalog::Classes, Catalog::Views] {
        for empty in [false, true] {
            let mut imports = vec![];
            let mut classes = vec![];
            let mut views = vec![];
            match catalog {
                Catalog::Imports => {
                    let mut v = import.clone();
                    if empty {
                        v.provider.clear();
                    }
                    imports.push(v.clone());
                    if !empty {
                        imports.push(v);
                    }
                }
                Catalog::Classes => {
                    let mut v = class.clone();
                    if empty {
                        v.name.clear();
                    }
                    classes.push(v.clone());
                    if !empty {
                        v.class.fallback = FallbackRole::Marker;
                        classes.push(v);
                    }
                }
                Catalog::Views => {
                    let mut v = view.clone();
                    if empty {
                        v.name.clear();
                    }
                    views.push(v.clone());
                    if !empty {
                        views.push(v);
                    }
                }
            }
            let context = ReaderContext {
                schema: &schema,
                state_type: &TypeDescriptor::Unit,
                imports: &imports,
                classes: &classes,
                views: &views,
                registry: &registry,
            };
            let result = compile::reader::compile(&checked, &context, &mut budget());
            let expected = if empty {
                CompileError::InvalidCatalogName(catalog)
            } else {
                CompileError::DuplicateCatalogName(catalog)
            };
            assert_eq!(result.err(), Some(expected));
        }
    }
    Ok(())
}
