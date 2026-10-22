use super::*;
use nepl3_doc_core::{check::Category, model::*};
use nepl3_doc_html::{ParallelMode, RenderOptions};
use nepl3_markup::html::*;
use nepl3_tools::doc::annotations::{
    self, SentenceAnnotationRenderer,
    article::{self, render},
};

#[test]
fn typed_article_host_resolves_and_renders_real_sentence_doc_guests() -> Result<(), String> {
    let compiled = nepl3_tools::doc::source::compiled_with_sentence_forms(&[
        nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "document",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "Article host test",
        },
    ])?;
    let source = r#"article en Sentence "Title [漢/かん]" body cons paragraph
cons sentence cons ref inner text "root ref" cons anchor outer text "root def" nil
cons Sentence sentence cons document anchor inner text "guest def" cons document ref other text "forward" nil
cons Sentence sentence cons document anchor other text "other" cons document ref outer text "root" nil
nil nil"#;
    for native in [false, true] {
        for nested_math in [false, true] {
            let source = if nested_math {
                source.replace("document anchor inner text \"guest def\"", "document anchor inner text \"guest def\" cons document math Math label x Sentence \"annotation\"")
            } else {
                source.into()
            };
            with_input_route(
                native,
                &compiled,
                &source,
                "Article",
                |tree, profile, b, a| {
                    let registry = profile.registry();
                    let checked = tree
                        .tree()
                        .bundle
                        .validate_with_sources(registry, b, a)
                        .map_err(err)?;
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let root = nepl3_suite::adapters::sentence::literal::lower(
                        &checked,
                        &compiled.doc.package.schema,
                        Category::Article,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    let mut host = SentenceAnnotationRenderer {
                        registry,
                        surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                        math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                        doc_surface: Some(&compiled.doc.package.schema),
                        codec: &mut codec,
                    };
                    let selected = article::collect_structural(&root, &mut host, b).map_err(err)?;
                    for parallel in [
                        ParallelMode::Rows,
                        ParallelMode::Columns,
                        ParallelMode::Single {
                            language: "en".into(),
                            fallbacks: vec![],
                        },
                    ] {
                        let output = render::render(
                            &selected,
                            &RenderOptions { parallel },
                            &mut host,
                            &mut |_, _, _, _| Err::<HtmlRequest, _>("unexpected outer guest"),
                            b,
                        )
                        .map_err(err)?;
                        assert_eq!(output.sentences.len(), 3);
                        assert_eq!(output.article.foreign.len(), 3);
                        assert_eq!(output.resolution.definitions().len(), 3);
                        let markup = &output.article.fragment.markup;
                        let text = serialize(
                            &validate(&markup.fragment, markup.slot, &markup.policy, b)
                                .map_err(err)?,
                            b,
                        )
                        .map_err(err)?;
                        assert!(
                            text.contains("guest def")
                                && text.contains("forward")
                                && text.contains("かん")
                        );
                        let total = markup.fragment.nodes.len() as u64;
                        let mut docs = 0;
                        let mut math = 0;
                        for record in &output.sentences {
                            let owner = &selected.owners()[record.owner.0];
                            assert_eq!(
                                output.article.foreign[record.placement].embed,
                                owner.embed()
                            );
                            for origin in &record.origins {
                                assert!(origin.element < total);
                                assert!(origin.node < (owner.syntax().value.nodes.len() as u64));
                            }
                            for foreign in &record.foreign {
                                let annotations::ForeignRecord::Document(doc) = foreign else {
                                    return Err("expected Doc record".into());
                                };
                                docs += 1;
                                for origin in &doc.origins {
                                    assert!(origin.element < total);
                                    assert!(origin.node < (doc.document.value.nodes.len() as u64));
                                    if let DocKind::Text { text } =
                                        &doc.document.value.nodes[origin.node as usize].kind
                                    {
                                        assert!(
                                            matches!(&markup.fragment.nodes[origin.element as usize], HtmlNode::Text { text: actual } if actual == text)
                                        );
                                    }
                                }
                                for nested in &doc.foreign {
                                    math += 1;
                                    for element in &nested.output.node_roots {
                                        assert!(*element < total);
                                    }
                                    for note in &nested.output.annotations {
                                        for origin in &note.origins {
                                            assert!(origin.element < total);
                                        }
                                    }
                                }
                            }
                        }
                        assert_eq!(docs, if nested_math { 5 } else { 4 });
                        assert_eq!(math, usize::from(nested_math));
                    }
                    let mut cancelled = budget();
                    cancelled.cancel();
                    assert!(matches!(
                        render::render(
                            &selected,
                            &RenderOptions {
                                parallel: ParallelMode::Rows
                            },
                            &mut host,
                            &mut |_, _, _, _| Err::<HtmlRequest, _>("unexpected"),
                            &mut cancelled
                        ),
                        Err(render::Error::Stopped(StopReason::Cancelled))
                    ));
                    Ok(())
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn article_host_preserves_failure_owners_and_scoped_operation_stops() -> Result<(), String> {
    let compiled = nepl3_tools::doc::source::compiled_with_sentence_forms(&[
        nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "document",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "Article error test",
        },
    ])?;
    for (scenario, source) in [
        r#"article en Sentence sentence cons document link external "javascript:alert(1)" text "bad" nil body nil"#,
        r#"article en Sentence "title" body cons code Math frac 1 2 cons image asset "missing" none Sentence "alt" none nil"#,
        r#"article en Sentence "title" body cons paragraph cons Sentence "body" nil cons code Math frac 1 2 nil"#,
    ].into_iter().enumerate() {
        with_input_route(true,&compiled,source,"Article",|tree,profile,b,a| {
            let registry=profile.registry();
            let checked=tree.tree().bundle.validate_with_sources(registry,b,a).map_err(err)?;
            let store=SourceStore::default(); let mut admission=SourceAdmission::default();
            let mut codec=FoundationCodec::new(registry,&store,&mut admission).map_err(err)?;
            let root=nepl3_suite::adapters::sentence::literal::lower(&checked,&compiled.doc.package.schema,Category::Article,registry,b,&mut codec).map_err(err)?;
            let mut host=SentenceAnnotationRenderer{registry,surface:&compiled.others.get(3).ok_or("Sentence")?.schema,math_surface:Some(&compiled.others.first().ok_or("Math")?.schema),doc_surface:Some(&compiled.doc.package.schema),codec:&mut codec};
            let selected=article::collect_structural(&root,&mut host,b).map_err(err)?;
            let mut calls=0;
            let result=render::render(&selected,&RenderOptions{parallel:ParallelMode::Rows},&mut host,&mut |_,_,_,_| {calls+=1;Err::<HtmlRequest,_>("late failure")},b);
            match (scenario,result) {
                (0,Err(render::Error::Document{member:Some(article::namespace::MemberOrigin::SentenceInline{sentence,inline_occurrence,..}),error})) => {
                    assert_eq!(sentence,article::namespace::SentenceOccurrenceId(0));
                    assert_eq!(inline_occurrence,0);
                    assert!(matches!(*error,annotations::document::Error::InvalidExternalUri{..}));
                    assert_eq!(calls,0);
                }
                (1,Err(render::Error::Document{member:Some(article::namespace::MemberOrigin::Article),error})) => {
                    assert!(matches!(*error,annotations::document::Error::NeedsResolution(_)));
                    assert_eq!(calls,0,"asset rejection precedes even earlier Code callbacks");
                }
                (2,Err(render::Error::Adapter(message))) => {assert_eq!(message,"late failure");assert_eq!(calls,1);}
                (_,Err(error)) => return Err(err(error)),
                (_,Ok(_)) => return Err("invalid operation succeeded".into()),
            }
            let adopted:Vec<_>=(0..selected.sentences().len()).map(article::namespace::SentenceOccurrenceId).collect();
            let result=article::namespace::resolve_with(&selected,&adopted,&mut host,b,|_,_,_,_|Err::<(),_>("callback failed"));
            assert!(matches!(result,Err(article::namespace::OperationError::Operation("callback failed"))));
            let mut cancelled=budget();
            let result=article::namespace::resolve_with(&selected,&adopted,&mut host,&mut cancelled,|_,_,_,b|{b.cancel();Err::<(),_>("masked")});
            assert!(matches!(result,Err(article::namespace::OperationError::Resolve(article::namespace::Error::Stopped(StopReason::Cancelled)))));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn article_host_keeps_parallel_table_shared_owners_and_non_sentence_positions() -> Result<(), String>
{
    let compiled = compiled()?;
    let source = r#"article en Sentence "Title" body
cons paragraph cons parallel cons variant en Sentence "English" cons variant ja Sentence "Japanese" nil
cons code Math frac 1 2 cons Sentence "Tail" nil
cons table cons left cons right nil some row cons Sentence "H1" cons Sentence "H2" nil
cons row cons Sentence "R1" cons Sentence "R2" nil nil
cons section s Sentence "Section" body cons paragraph cons Sentence "Section body" nil nil
nil"#;
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            source,
            "Article",
            |tree, profile, b, a| {
                let registry = profile.registry();
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(registry, b, a)
                    .map_err(err)?;
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let mut root = nepl3_suite::adapters::sentence::literal::lower(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    registry,
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                // Repeat the same declaration-free owner, not a second lowered copy.
                let items = root
                    .value
                    .nodes
                    .iter_mut()
                    .find_map(|node| match &mut node.kind {
                        DocKind::Paragraph { items } if items.len() == 3 => Some(items),
                        _ => None,
                    })
                    .ok_or("paragraph")?;
                items.push(*items.last().ok_or("tail")?);
                let mut host = SentenceAnnotationRenderer {
                    registry,
                    surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                    math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let selected = article::collect_structural(&root, &mut host, b).map_err(err)?;
                assert_eq!(selected.sentences().len(), 11);
                assert_eq!(selected.owners().len(), 10);
                let code = || HtmlRequest {
                    fragment: HtmlFragment {
                        root: 0,
                        nodes: vec![HtmlNode::Text {
                            text: "CODE".into(),
                        }],
                    },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                };
                for parallel in [ParallelMode::Rows, ParallelMode::Columns] {
                    let mut calls = 0;
                    let output = render::render(
                        &selected,
                        &RenderOptions { parallel },
                        &mut host,
                        &mut |view, _, _, _| {
                            assert_eq!(view.kind, EmbedKind::Code);
                            calls += 1;
                            Ok::<_, String>(code())
                        },
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(calls, 1);
                    assert_eq!(output.sentences.len(), 11);
                    assert_eq!(output.article.foreign.len(), 12);
                    assert_eq!(output.sentences[3].owner, output.sentences[4].owner);
                    assert_eq!(output.sentences[3].placement, 4);
                    assert_eq!(output.sentences[4].placement, 5);
                    let markup = &output.article.fragment.markup;
                    let text = serialize(
                        &validate(&markup.fragment, markup.slot, &markup.policy, b).map_err(err)?,
                        b,
                    )
                    .map_err(err)?;
                    let mut after = 0;
                    for item in [
                        "Title",
                        "English",
                        "Japanese",
                        "CODE",
                        "Tail",
                        "Tail",
                        "H1",
                        "H2",
                        "R1",
                        "R2",
                        "Section",
                        "Section body",
                    ] {
                        after += text[after..]
                            .find(item)
                            .ok_or_else(|| format!("order: {item}"))?
                            + item.len();
                    }
                    for record in &output.sentences {
                        for origin in &record.origins {
                            if let nepl3_sentence_core::model::Kind::Text { text } =
                                &selected.owners()[record.owner.0].syntax().value.nodes
                                    [origin.node as usize]
                            {
                                // The root Span also has a source cause; only text
                                // outputs are compared to source text here.
                                if let HtmlNode::Text { text: actual } =
                                    &markup.fragment.nodes[origin.element as usize]
                                {
                                    assert_eq!(actual, text);
                                }
                            }
                        }
                    }
                }
                // This valid 128-level guest fits alone, but not below the
                // Article's Code insertion ancestors. Input validation fits
                // the same limit, so the failure occurs after the callback.
                let mut nodes = Vec::new();
                for index in 0..127 {
                    nodes.push(HtmlNode::Element {
                        tag: HtmlTag::Span,
                        attributes: vec![],
                        children: vec![index + 1],
                    });
                }
                nodes.push(HtmlNode::Text {
                    text: "deep".into(),
                });
                let deep = HtmlRequest {
                    fragment: HtmlFragment { root: 0, nodes },
                    slot: HtmlSlot::Phrasing,
                    policy: HtmlPolicy { classes: vec![] },
                };
                let mut limits = budget().limits();
                limits.depth = 128;
                check_part(
                    &deep.fragment,
                    deep.slot,
                    &deep.policy,
                    &mut Budget::new(limits),
                )
                .map_err(err)?;
                let mut operation = Budget::new(limits);
                let mut calls = 0;
                let result = render::render(
                    &selected,
                    &RenderOptions {
                        parallel: ParallelMode::Rows,
                    },
                    &mut host,
                    &mut |_, _, _, _| {
                        calls += 1;
                        Ok::<_, String>(deep.clone())
                    },
                    &mut operation,
                );
                assert_eq!(calls, 1);
                assert_eq!(operation.poll(), Err(StopReason::DepthLimit));
                assert!(matches!(
                    result,
                    Err(render::Error::Stopped(StopReason::DepthLimit))
                ));
                Ok(())
            },
        )?;
    }
    Ok(())
}
