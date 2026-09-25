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
    use nepl3_doc_core::model::{Alignment, DocKind, DocRoot};
    let compiled = compiled()?;
    let source = include_str!("../../../examples/document/linear-combination.nepld");
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &store, a).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        let kind = |id: u64| &document.value.nodes[id as usize].kind;
        let DocRoot::Article(root) = document.value.root else {
            return Err("article root".into());
        };
        let DocKind::Article { body, .. } = kind(root.0) else {
            return Err("article".into());
        };
        let DocKind::Body { blocks } = kind(body.0) else {
            return Err("article body".into());
        };
        assert_eq!(blocks.len(), 4);
        for (block, (expected_id, sentence_pairs, has_table)) in blocks.iter().zip([
            ("linear_combination", 2, false),
            ("span", 2, false),
            ("plane", 3, true),
            ("independence", 3, false),
        ]) {
            let DocKind::Section { id, body, .. } = kind(block.0) else {
                return Err("section".into());
            };
            assert_eq!(id, expected_id);
            let DocKind::Body { blocks } = kind(body.0) else {
                return Err("section body".into());
            };
            assert_eq!(blocks.len(), if has_table { 2 } else { 1 });
            let DocKind::Paragraph { items } = kind(blocks[0].0) else {
                return Err("paragraph".into());
            };
            assert_eq!(items.len(), sentence_pairs);
            for item in items {
                let DocKind::Parallel { variants } = kind(item.0) else {
                    return Err("parallel".into());
                };
                assert_eq!(variants.len(), 2);
                for (variant, expected) in variants.iter().zip(["ja", "en"]) {
                    let DocKind::Variant { language, sentence } = kind(variant.0) else {
                        return Err("variant".into());
                    };
                    assert_eq!(language, expected);
                    assert!(matches!(kind(sentence.0), DocKind::Sentence { .. }));
                }
            }
            if has_table {
                let DocKind::Table {
                    columns,
                    header,
                    rows,
                } = kind(blocks[1].0)
                else {
                    return Err("table".into());
                };
                assert_eq!(
                    columns,
                    &[Alignment::Left, Alignment::Left, Alignment::Right]
                );
                assert_eq!(rows.len(), 3);
                for row in
                    std::iter::once(header.ok_or("table header")?).chain(rows.iter().copied())
                {
                    let DocKind::Row { cells } = kind(row.0) else {
                        return Err("row".into());
                    };
                    assert_eq!(cells.len(), 3);
                    for cell in cells {
                        assert!(matches!(kind(cell.0), DocKind::Sentence { .. }));
                    }
                }
            }
        }
        Ok(())
    })?;
    let out = nepl3_tools::doc::export::generate(&compiled, source)?.html;
    assert_eq!(out.matches("<section ").count(), 4);
    assert_eq!(out.matches("<table").count(), 1);
    assert_eq!(out.matches("class=\"nepl-parallel").count(), 10);
    assert!(out.contains("linear combination"));
    assert!(out.contains("class=\"nepl-ruby\""));
    assert!(out.contains("class=\"nepl-anno\""));
    // Both language variants refer to the same declared section through Doc Inline.
    assert_eq!(
        out.matches("href=\"#n-6c696e6561725f636f6d62696e6174696f6e\"")
            .count(),
        2
    );
    assert!(out.contains("<section id=\"n-6c696e6561725f636f6d62696e6174696f6e\">"));
    assert!(!out.contains("<script"));
    Ok(())
}

#[test]
fn standalone_sentence_renders_annotation_without_fabricated_article() -> Result<(), String> {
    use nepl3_doc_html::{portable, prepare_local_sentence, render_sentence};
    let compiled = compiled_html()?;
    let source = r#"sentence cons anno ruby text "字" text "じ" cons text "character" nil cons break cons code "<&" nil"#;
    with_input_route(
        true,
        &compiled,
        source,
        "Sentence",
        |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let doc = lower::document(
                &syntax,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared = prepare_local_sentence(
                &doc,
                &options,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let rendered = render_sentence(&prepared, &mut budget()).map_err(err)?;
            assert_eq!(rendered.markup.slot, nepl3_markup::html::HtmlSlot::Phrasing);
            let checked = nepl3_markup::html::validate(
                &rendered.markup.fragment,
                rendered.markup.slot,
                &rendered.markup.policy,
                &mut budget(),
            )
            .map_err(err)?;
            let html = nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?;
            println!(
                "DOC_SENTENCE_HTML {}",
                html.as_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            // The specified shared inline tree preserves both annotation levels,
            // the explicit break, and code escaping without an Article/title.
            assert_eq!(
                html,
                "<span class=\"nepl-sentence\"><span><span class=\"nepl-anno\"><span class=\"nepl-base\"><span class=\"nepl-ruby\"><span class=\"nepl-base\">字</span><span class=\"nepl-reading\">じ</span></span></span><span class=\"nepl-notes\"><span class=\"nepl-note\">character</span></span></span><br><code>&lt;&amp;</code></span></span>"
            );
            let value = portable::rendered_sentence_to_value(
                &rendered,
                &prepared,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            assert_eq!(
                portable::rendered_sentence_from_value(
                    &received,
                    &prepared,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                )
                .map_err(err)?,
                rendered
            );
            let mut altered = rendered.clone();
            altered.document_digest = Digest::of(b"wrong input");
            assert!(matches!(
                portable::rendered_sentence_to_value(
                    &altered,
                    &prepared,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(portable::PortableError::Mismatch)
            ));
            for resource in 0..4 {
                let mut limits = budget().limits();
                match resource {
                    0 => limits.work = 0,
                    1 => limits.allocation_units = 0,
                    2 => limits.nodes = 0,
                    _ => limits.depth = 0,
                }
                assert!(matches!(
                    render_sentence(&prepared, &mut Budget::new(limits)),
                    Err(nepl3_doc_html::RenderError::Stopped(_))
                ));
            }
            Ok(())
        },
    )
}
#[test]
fn article_html_preserves_nested_paragraph_order_without_nested_p() -> Result<(), String> {
    let result = nepl3_tools::doc::export::generate(
        &compiled()?,
        r#"article ja sentence "例" body cons paragraph cons sentence "外A" cons paragraph cons sentence "内" nil cons sentence "外B" nil nil"#,
    )?.html;
    // The complete subtree fixes the order and paragraph boundaries independently
    // of the export shell. Doc owns the placement span; independent Sentence
    // owns the inner phrasing wrapper and its literal content.
    let sentence = |text: &str| {
        format!("<span><span class=\"nepl-sentence\"><span>{text}</span></span></span>")
    };
    assert!(result.contains(&format!(
        "<article class=\"nepl-doc\" lang=\"ja\"><h1>{}</h1><div class=\"nepl-paragraph\"><p>{}</p><div><div class=\"nepl-paragraph\"><p>{}</p></div></div><p>{}</p></div></article>",
        sentence("例"), sentence("外A"), sentence("内"), sentence("外B")
    )), "{result}");
    Ok(())
}

#[test]
fn explicit_break_renders_br_without_reinterpreting_text_line_feeds() -> Result<(), String> {
    let compiled = compiled()?;
    let out = nepl3_tools::doc::export::generate(
        &compiled,
        include_str!("../../../examples/document/line-break.nepld"),
    )?
    .html;
    assert_eq!(out.matches("<br>").count(), 2);
    assert!(out.contains("This sentence continues<br>on the next line."));
    let out = nepl3_tools::doc::export::generate(
        &compiled,
        r#"article en sentence "Title" body cons paragraph cons sentence sentence cons text "a\nb" cons break cons text "c" nil nil nil"#,
    )?.html;
    assert!(out.contains("a\nb<br>c"));
    assert_eq!(out.matches("<br>").count(), 1);
    Ok(())
}
#[test]
fn article_html_preserves_ruby_notes_and_forward_label_reference() -> Result<(), String> {
    let result = nepl3_tools::doc::export::generate(
        &compiled()?,
        r#"article ja sentence "注記" body cons paragraph cons sentence sentence cons doc ref 終 text "先へ" cons anno ruby text "字" text "じ" cons text "character" nil nil nil cons section 終 sentence "末尾" body nil nil"#,
    )?.html;
    // The reference remains a Doc Inline; its label is independent Sentence.
    assert!(result.contains("<a href=\"#n-e7b582\"><span class=\"nepl-sentence\">先へ</span></a>"));
    assert!(result.contains("<section id=\"n-e7b582\"><h2><span><span class=\"nepl-sentence\"><span>末尾</span></span></span></h2></section>"));
    assert!(result.contains("<span class=\"nepl-anno\"><span class=\"nepl-base\"><span class=\"nepl-ruby\"><span class=\"nepl-base\">字</span><span class=\"nepl-reading\">じ</span></span></span><span class=\"nepl-notes\"><span class=\"nepl-note\">character</span></span></span>"));
    Ok(())
}
#[test]
fn parallel_html_selects_only_explicit_language_or_fallback() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article ja sentence "対応" body cons paragraph cons parallel cons variant ja sentence "日本語" cons variant en sentence "English" nil nil nil"#;
    let options = |language: &str, fallbacks: Vec<String>| RenderOptions {
        parallel: ParallelMode::Single {
            language: language.into(),
            fallbacks,
        },
    };
    let generate =
        |options| nepl3_tools::doc::export::generate_with_options(&compiled, source, options);
    let selected = generate(options("EN", vec![]))?;
    assert!(selected.html.contains("<span lang=\"en\"><span><span class=\"nepl-sentence\"><span>English</span></span></span></span>"), "{}", selected.html);
    assert!(!selected.html.contains("日本語"));
    assert!(generate(options("fr", vec![])).is_err());
    let fallback = generate(options("fr", vec!["en".into()]))?;
    assert_eq!(selected.html, fallback.html);
    let manifest: serde_json::Value = serde_json::from_str(&fallback.manifest).map_err(err)?;
    assert_eq!(
        manifest["options"],
        serde_json::json!({"parallel":"Single","language":"fr","fallbacks":["en"]})
    );
    let all = generate(RenderOptions {
        parallel: ParallelMode::Columns,
    })?;
    assert!(all.html.contains("日本語") && all.html.contains("English"));
    let manifest: serde_json::Value = serde_json::from_str(&all.manifest).map_err(err)?;
    assert_eq!(
        manifest["options"],
        serde_json::json!({"parallel":"Columns"})
    );
    let rows = generate(RenderOptions {
        parallel: ParallelMode::Rows,
    })?;
    let default = nepl3_tools::doc::export::generate(&compiled, source)?;
    assert_eq!(rows.html, default.html);
    assert_eq!(rows.manifest, default.manifest);
    for invalid in [options("bad_tag", vec![]), options("en", vec!["EN".into()])] {
        assert!(generate(invalid).is_err());
    }
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
    let compiled = compiled_html()?;
    real_source_html_roundtrip(&compiled)
}
fn compiled_html() -> Result<Compiled, String> {
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
    Ok(compiled)
}
fn real_source_html_roundtrip(compiled: &Compiled) -> Result<(), String> {
    use nepl3_doc_html::{LocalHtmlRequest, portable};
    fn compose<C: nepl3_core::value_codec::FoundationValueCodec>(
        request: &LocalHtmlRequest,
        registry: &nepl3_core::schema::SchemaRegistry,
        codec: &mut C,
        operation: &mut nepl3_core::budget::Budget,
    ) -> Result<nepl3_doc_html::RenderedWithForeign, String>
    where
        C::Error: core::fmt::Debug,
    {
        let prepared = nepl3_doc_html::prepare_article_with_foreign(
            &request.document,
            &request.options,
            registry,
            codec,
            operation,
        )
        .map_err(err)?;
        nepl3_doc_html::render_article_with_foreign(
            &prepared,
            &mut |slot, _, b| {
                let sentence = nepl3_suite::adapters::document::sentence::lower(
                    slot,
                    slot.schema(),
                    &[],
                    registry,
                    codec,
                    b,
                )
                .map_err(err)?;
                nepl3_suite::adapters::sentence::html::render(
                    &sentence,
                    registry,
                    b,
                    &mut SourceAdmission::default(),
                )
                .map(|rendered| rendered.into_markup())
                .map_err(err)
            },
            operation,
        )
        .map_err(err)
    }
    let input = "article ja sentence \"原稿🙂\"\r\nbody cons paragraph cons parallel cons variant ja sentence \"[文/ぶん]\" cons variant en sentence \"text\" nil nil nil";
    with_input(compiled, input, "Article", |tree, profile, b, a| {
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
        let native = compose(&request, r, &mut codec, &mut budget())?;
        // Title and selected English variant are the two displayed guests.
        assert_eq!(native.foreign.len(), 2);
        let markup = &native.fragment.markup;
        let checked_html = nepl3_markup::html::validate(
            &markup.fragment,
            markup.slot,
            &markup.policy,
            &mut budget(),
        )
        .map_err(err)?;
        let output = nepl3_markup::html::serialize(&checked_html, &mut budget()).map_err(err)?;
        assert!(output.contains("原稿🙂") && output.contains("text"));
        assert!(!output.contains("ぶん"));
        let request_bytes = nepl3_wire::encode(
            &portable::request_to_value(&request, r, &mut codec, &mut budget()).map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let reply_bytes = nepl3_wire::encode(
            &portable::foreign::to_value(&native, r, &mut codec, &mut budget()).map_err(err)?,
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
        assert_eq!(received.options, request.options);
        assert_eq!(
            received.document.value.embeds,
            request.document.value.embeds
        );
        let expected = compose(&received, r, &mut receiver, &mut operation)?;
        let value = nepl3_wire::decode(&reply_bytes, &mut operation).map_err(err)?;
        assert_eq!(
            portable::foreign::from_value(&value, &expected, r, &mut receiver, &mut operation)
                .map_err(err)?,
            native
        );
        // Valid type and valid HTML still do not authorize altered contents.
        let mut changed = native.clone();
        let text = changed
            .fragment
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
        let forged =
            portable::foreign::to_value(&changed, r, &mut receiver, &mut budget()).map_err(err)?;
        assert!(matches!(
            portable::foreign::from_value(&forged, &expected, r, &mut receiver, &mut budget()),
            Err(portable::PortableError::Mismatch)
        ));
        let mut changed_options = native.clone();
        changed_options.fragment.options.parallel = ParallelMode::Rows;
        let mut changed_placement = native.clone();
        changed_placement.foreign[0].embed.0 = u64::MAX;
        for changed in [changed_options, changed_placement] {
            let forged = portable::foreign::to_value(&changed, r, &mut receiver, &mut budget())
                .map_err(err)?;
            assert!(matches!(
                portable::foreign::from_value(&forged, &expected, r, &mut receiver, &mut budget()),
                Err(portable::PortableError::Mismatch)
            ));
        }
        let mut changed = native.clone();
        changed.fragment.origins[0].node = u64::MAX;
        let forged =
            portable::foreign::to_value(&changed, r, &mut receiver, &mut budget()).map_err(err)?;
        assert!(matches!(
            portable::foreign::from_value(&forged, &expected, r, &mut receiver, &mut budget()),
            Err(portable::PortableError::Mismatch)
        ));
        Ok(())
    })
}

// Actual production output consumed by the separate browser layout probe.
// ASCII A/B have identical font metrics; B is always the annotated base.
#[test]
fn browser_layout_corpus_from_real_doc_source() -> Result<(), String> {
    let compiled = compiled()?;
    for (name, source) in [
        (
            "ruby",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A[B/read]C" nil nil"#,
        ),
        (
            "anno",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A{B/note}C" nil nil"#,
        ),
        (
            "anno-ruby",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A{[B/read]/note}C" nil nil"#,
        ),
        (
            "ruby-anno",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A[{B/note}/read]C" nil nil"#,
        ),
        (
            "ruby-ruby",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A[[B/read]/outer]C" nil nil"#,
        ),
        (
            "table-ruby",
            r#"article en sentence "Layout" body cons table cons left nil none cons row cons sentence "A[B/read]C" nil nil nil"#,
        ),
        (
            "list-ruby",
            r#"article en sentence "Layout" body cons list unordered cons item none body cons paragraph cons sentence "A[B/read]C" nil nil nil nil"#,
        ),
        (
            "ruby-multiline",
            r#"article en sentence "Layout" body cons paragraph cons sentence sentence cons text "A" cons ruby concat cons text "D" cons break cons text "B" nil text "read" cons text "C" nil nil nil"#,
        ),
        (
            "anno-multiline",
            r#"article en sentence "Layout" body cons paragraph cons sentence sentence cons text "A" cons anno concat cons text "B" cons break cons text "D" nil cons text "note" nil cons text "C" nil nil nil"#,
        ),
        (
            "reading-ruby",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A[B/[read/outer]]C" nil nil"#,
        ),
        (
            "notes-ruby",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A{B/[note/read]/second}C" nil nil"#,
        ),
        (
            "anno-anno",
            r#"article en sentence "Layout" body cons paragraph cons sentence "A{{B/inner}/outer}C" nil nil"#,
        ),
        (
            "line-reservation",
            r#"article en sentence "Layout" body cons paragraph cons sentence sentence cons text "Z" cons break cons text "A" cons anno ruby text "B" ruby text "read" text "outer" cons ruby text "note" text "reading" cons text "second" nil cons text "C" cons break cons text "Y" nil nil nil"#,
        ),
    ] {
        let out = nepl3_tools::doc::export::generate(&compiled, source)?.html;
        assert!(out.contains("nepl-base"));
        let hex: String = out
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        println!("DOC_HTML_CASE {name} {hex}");
    }
    Ok(())
}
