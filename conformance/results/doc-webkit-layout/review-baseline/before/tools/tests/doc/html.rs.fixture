use super::*;
use nepl3_doc_core::{check::Category, lower};
use nepl3_doc_html::{ParallelMode, RenderOptions, prepare_local, render};

fn html(source: &str, options: RenderOptions) -> Result<String, String> {
    let compiled = compiled()?;
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(|e| format!("bundle validation: {e:?}; usage={:?}", b.usage()))?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let mut operation = budget();
        let prepared = prepare_local(
            &doc,
            &options,
            profile.registry(),
            &mut codec,
            &mut operation,
        )
        .map_err(err)?;
        let fragment = render(&prepared, &mut operation).map_err(err)?;
        let m = &fragment.markup;
        let checked = nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, &mut operation)
            .map_err(err)?;
        nepl3_markup::html::serialize(&checked, &mut operation).map_err(err)
    })
}
#[test]
fn annotated_linear_combination_document_reaches_real_html() -> Result<(), String> {
    let out = html(
        include_str!("../../../examples/document/linear-combination.nepld"),
        RenderOptions {
            parallel: ParallelMode::Rows,
        },
    )?;
    assert_eq!(out.matches("<section ").count(), 4);
    assert_eq!(out.matches("<table").count(), 1);
    assert_eq!(out.matches("class=\"nepl-parallel").count(), 10);
    assert!(out.contains("linear combination"));
    assert!(out.contains("class=\"nepl-ruby\""));
    assert!(out.contains("class=\"nepl-anno\""));
    assert!(!out.contains("<script"));
    Ok(())
}
#[test]
fn article_html_preserves_nested_paragraph_order_without_nested_p() -> Result<(), String> {
    let result = html(
        r#"article ja "例" body cons paragraph cons "外A" cons paragraph cons "内" nil cons "外B" nil nil"#,
        RenderOptions {
            parallel: ParallelMode::Rows,
        },
    )?;
    assert_eq!(
        result,
        "<article class=\"nepl-doc\" lang=\"ja\"><h1><span>例</span></h1><div class=\"nepl-paragraph\"><p><span>外A</span></p><div><div class=\"nepl-paragraph\"><p><span>内</span></p></div></div><p><span>外B</span></p></div></article>"
    );
    Ok(())
}

#[test]
fn explicit_break_renders_br_without_reinterpreting_text_line_feeds() -> Result<(), String> {
    let out = html(
        include_str!("../../../examples/document/line-break.nepld"),
        RenderOptions {
            parallel: ParallelMode::Rows,
        },
    )?;
    assert_eq!(out.matches("<br>").count(), 2);
    assert!(out.contains("This sentence continues<br>on the next line."));
    let out = html(
        r#"article en "Title" body cons paragraph cons sentence cons text "a\nb" cons break cons text "c" nil nil nil"#,
        RenderOptions {
            parallel: ParallelMode::Rows,
        },
    )?;
    assert!(out.contains("a\nb<br>c"));
    assert_eq!(out.matches("<br>").count(), 1);
    Ok(())
}
#[test]
fn article_html_preserves_ruby_notes_and_forward_label_reference() -> Result<(), String> {
    let result = html(
        r#"article ja "注記" body cons paragraph cons sentence cons ref 終 text "先へ" cons anno ruby text "字" text "じ" cons text "character" nil nil nil cons section 終 "末尾" body nil nil"#,
        RenderOptions {
            parallel: ParallelMode::Rows,
        },
    )?;
    assert!(result.contains("<a href=\"#n-e7b582\">先へ</a>"));
    assert!(result.contains("<section id=\"n-e7b582\"><h2><span>末尾</span></h2></section>"));
    assert!(result.contains("<span class=\"nepl-anno\"><span class=\"nepl-base\"><span class=\"nepl-ruby\"><span class=\"nepl-base\">字</span><span class=\"nepl-reading\">じ</span></span></span><span class=\"nepl-notes\"><span class=\"nepl-note\">character</span></span></span>"));
    Ok(())
}
#[test]
fn parallel_html_selects_only_explicit_language_or_fallback() -> Result<(), String> {
    let source = r#"article ja "対応" body cons paragraph cons parallel cons variant ja "日本語" cons variant en "English" nil nil nil"#;
    let options = |language: &str, fallbacks: Vec<String>| RenderOptions {
        parallel: ParallelMode::Single {
            language: language.into(),
            fallbacks,
        },
    };
    let selected = html(source, options("EN", vec![]))?;
    assert!(selected.contains("<span lang=\"en\"><span>English</span></span>"));
    assert!(!selected.contains("日本語"));
    assert!(html(source, options("fr", vec![])).is_err());
    assert_eq!(selected, html(source, options("fr", vec!["en".into()]))?);
    let all = html(
        source,
        RenderOptions {
            parallel: ParallelMode::Columns,
        },
    )?;
    assert!(all.contains("日本語") && all.contains("English"));
    Ok(())
}

#[test]
fn table_list_and_raw_code_keep_order_shape_and_original_characters() -> Result<(), String> {
    let source = r#"article ja "構造" body
 cons table cons left cons right nil
   some row cons "A" cons "B" nil
   cons row cons "1" cons "2" nil
   cons row cons "3" cons "4" nil nil
 cons list ordered 7
   cons item checked body cons paragraph cons "項目" nil nil
   cons item unchecked body cons list unordered cons item none body nil nil nil nil
 cons rawcode some "Rust" "\n\r\n<T> &  x"
 cons table nil none nil
 nil"#;
    let out = html(
        source,
        RenderOptions {
            parallel: ParallelMode::Rows,
        },
    )?;
    assert!(out.contains("<table><thead><tr><th class=\"nepl-align-left\" scope=\"col\"><span>A</span></th><th class=\"nepl-align-right\" scope=\"col\"><span>B</span></th></tr></thead><tbody><tr><td class=\"nepl-align-left\"><span>1</span></td><td class=\"nepl-align-right\"><span>2</span></td></tr><tr><td class=\"nepl-align-left\"><span>3</span></td><td class=\"nepl-align-right\"><span>4</span></td></tr></tbody></table>"));
    assert!(out.contains("<ol start=\"7\"><li><span aria-label=\"checked\" class=\"nepl-checkbox\" role=\"img\">☑ </span>"));
    assert!(out.contains("<ul><li></li></ul>"));
    assert!(out.contains("<figure><figcaption>Rust</figcaption><pre>\n<code>\n&#xD;\n&lt;T&gt; &amp;  x</code></pre></figure><table></table>"));
    Ok(())
}

#[test]
fn local_render_does_not_hide_unresolved_external_inputs_or_invalid_options() {
    for source in [
        r#"article ja "外部" body cons paragraph cons sentence cons link external "https://example.org" text "URL" nil nil nil"#,
        r#"article ja "画像" body cons image asset "logo" none "代替" none nil"#,
        r#"article ja "数式" body cons display Math frac 1 0 nil"#,
        r#"article ja "コード" body cons code Math frac 1 0 nil"#,
    ] {
        let error = html(
            source,
            RenderOptions {
                parallel: ParallelMode::Rows,
            },
        )
        .err();
        assert!(
            error.is_some_and(|e| e.starts_with("NeedsResolution(")),
            "{source}"
        );
    }
    let source = r#"article ja "文" body nil"#;
    for (language, fallbacks) in [("bad_tag", vec![]), ("en", vec!["EN".into()])] {
        assert_eq!(
            html(
                source,
                RenderOptions {
                    parallel: ParallelMode::Single {
                        language: language.into(),
                        fallbacks
                    }
                }
            ),
            Err("Language".into())
        );
    }
    assert!(
        html(
            r#"article ja "列" body cons list ordered 2147483648 nil nil"#,
            RenderOptions {
                parallel: ParallelMode::Rows
            }
        )
        .is_err()
    );
}

#[test]
fn real_source_html_crosses_first_receiver_cbor_with_positions_and_options() -> Result<(), String> {
    use nepl3_doc_html::{LocalHtmlRequest, portable};
    let mut compiled = compiled()?;
    for descriptor in [
        nepl3_markup::schema::descriptor(&mut budget()),
        nepl3_doc_html::schema::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        compiled
            .doc
            .registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    compiled.doc.registry.finalize(&mut budget()).map_err(err)?;
    let input = "article ja \"原稿🙂\"\r\nbody cons paragraph cons parallel cons variant ja \"[文/ぶん]\" cons variant en \"text\" nil nil nil";
    with_input(&compiled, input, "Article", |tree, profile, b, a| {
        let r = profile.registry();
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(r, b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admitted = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admitted).map_err(err)?;
        let request = LocalHtmlRequest {
            document: lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Article,
                r,
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?,
            options: RenderOptions {
                parallel: ParallelMode::Single {
                    language: "EN".into(),
                    fallbacks: vec!["ja".into()],
                },
            },
        };
        let prepared = prepare_local(
            &request.document,
            &request.options,
            r,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let native = render(&prepared, &mut budget()).map_err(err)?;
        let request_bytes = nepl3_wire::encode(
            &portable::request_to_value(&request, r, &mut codec, &mut budget()).map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let reply_bytes = nepl3_wire::encode(
            &portable::rendered_to_value(&native, &prepared, r, &mut codec, &mut budget())
                .map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let mut receiver_admission = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(r, &empty, &mut receiver_admission).map_err(err)?;
        let mut operation = budget();
        let received = portable::request_from_value(
            &nepl3_wire::decode(&request_bytes, &mut operation).map_err(err)?,
            r,
            &mut receiver,
            &mut operation,
        )
        .map_err(err)?;
        // These are real source positions, including CRLF and astral UTF-8,
        // not a source-less constructor transport test.
        assert_eq!(received.document.value.nodes, request.document.value.nodes);
        assert!(received.document.sources.iter().any(|s| s.text() == input));
        assert_eq!(received.document.origins, request.document.origins);
        assert_eq!(received.document.views, request.document.views);
        assert_eq!(received.document.source_maps, request.document.source_maps);
        assert!(
            received
                .document
                .value
                .nodes
                .iter()
                .any(|n| n.span.is_some())
        );
        let p = prepare_local(
            &received.document,
            &received.options,
            r,
            &mut receiver,
            &mut operation,
        )
        .map_err(err)?;
        let value = nepl3_wire::decode(&reply_bytes, &mut operation).map_err(err)?;
        assert_eq!(
            portable::rendered_from_value(&value, &p, r, &mut receiver, &mut operation)
                .map_err(err)?,
            native
        );
        // Valid type and valid HTML still do not authorize altered contents.
        let mut changed = native.clone();
        let text = changed
            .markup
            .fragment
            .nodes
            .iter_mut()
            .find_map(|n| match n {
                nepl3_markup::html::HtmlNode::Text { text } if text == "text" => Some(text),
                _ => None,
            })
            .ok_or("selected text")?;
        *text = "forged".into();
        assert!(
            portable::rendered_to_value(&changed, &p, r, &mut receiver, &mut budget()).is_err()
        );
        let mut changed = native.clone();
        changed.origins[0].node = u64::MAX;
        assert!(
            portable::rendered_to_value(&changed, &p, r, &mut receiver, &mut budget()).is_err()
        );
        Ok(())
    })
}

// Actual production output consumed by the separate browser layout probe.
// ASCII A/B have identical font metrics; B is always the annotated base.
#[test]
fn browser_layout_corpus_from_real_doc_source() -> Result<(), String> {
    for (name, source) in [
        (
            "ruby",
            r#"article en "Layout" body cons paragraph cons "A[B/read]C" nil nil"#,
        ),
        (
            "anno",
            r#"article en "Layout" body cons paragraph cons "A{B/note}C" nil nil"#,
        ),
        (
            "anno-ruby",
            r#"article en "Layout" body cons paragraph cons "A{[B/read]/note}C" nil nil"#,
        ),
        (
            "ruby-anno",
            r#"article en "Layout" body cons paragraph cons "A[{B/note}/read]C" nil nil"#,
        ),
        (
            "ruby-ruby",
            r#"article en "Layout" body cons paragraph cons "A[[B/read]/outer]C" nil nil"#,
        ),
        (
            "table-ruby",
            r#"article en "Layout" body cons table cons left nil none cons row cons "A[B/read]C" nil nil nil"#,
        ),
        (
            "list-ruby",
            r#"article en "Layout" body cons list unordered cons item none body cons paragraph cons "A[B/read]C" nil nil nil nil"#,
        ),
    ] {
        let out = html(
            source,
            RenderOptions {
                parallel: ParallelMode::Rows,
            },
        )?;
        assert!(out.contains("class=\"nepl-base\">B</span>"));
        let hex: String = out
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        println!("DOC_HTML_CASE {name} {hex}");
    }
    Ok(())
}
