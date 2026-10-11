use super::*;

#[test]
fn ordinary_export_renders_safe_urls_and_local_references() -> Result<(), String> {
    let c = compiled()?;
    let input = r#"article ja "参照" body cons paragraph cons sentence cons ref destination text "節へ" cons text " / " cons link external "https://example.org/docs?q=one&lang=ja#part" ruby text "資料" text "しりょう" cons text " / " cons link external "mailto:author@example.org" text "連絡" nil nil cons section destination "節" body cons paragraph cons "本文" nil nil nil"#;
    for css in [export::CssMode::External, export::CssMode::Inline] {
        let output =
            export::generate_with_renderer(&c, input, css, export::MathRenderer::MathMLOnly)?;
        assert!(
            output
                .html
                .contains("href=\"https://example.org/docs?q=one&amp;lang=ja#part\"")
        );
        assert!(output.html.contains("href=\"mailto:author@example.org\""));
        assert!(output.html.contains("href=\"#n-64657374696e6174696f6e\""));
        assert!(output.html.contains("id=\"n-64657374696e6174696f6e\""));
        assert!(output.html.contains("しりょう"));
        assert!(!output.html.contains("<script"));
        use nepl3_doc_core::model::{DocKind, LinkTarget};
        use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode, HtmlTag};
        let mut verified = 0;
        for (element, node) in output.rendered.markup.fragment.nodes.iter().enumerate() {
            let HtmlNode::Element {
                tag: HtmlTag::A,
                attributes,
                children,
            } = node
            else {
                continue;
            };
            for attr in attributes {
                let HtmlAttribute::Href {
                    value: HtmlHref::External { uri },
                } = attr
                else {
                    continue;
                };
                let origin = output
                    .rendered
                    .origins
                    .iter()
                    .find(|o| o.element == element as u64)
                    .ok_or("link origin missing")?;
                let DocKind::Link {
                    target: LinkTarget::External { uri: source_uri },
                    label,
                } = &output.document.value.nodes[origin.node as usize].kind
                else {
                    return Err("wrong link origin".into());
                };
                assert_eq!(uri, source_uri);
                assert!(
                    children.iter().any(|child| output
                        .rendered
                        .origins
                        .iter()
                        .any(|o| o.element == *child && o.node == label.0)),
                    "label origin missing"
                );
                verified += 1;
            }
        }
        assert_eq!(verified, 2);
    }
    Ok(())
}

#[test]
fn ordinary_export_rejects_unsafe_or_unresolved_references() -> Result<(), String> {
    let c = compiled()?;
    for uri in [
        "javascript:alert(1)",
        "data:text/html,x",
        "//example.org/",
        "https://user@example.org/",
        "https://example.org/%",
        "https://example.org/a b",
        "HTTPS://example.org/",
    ] {
        let input = format!(
            r#"article en "Links" body cons paragraph cons sentence cons link external "{uri}" text "label" nil nil nil"#
        );
        let error = export::generate(&c, &input)
            .err()
            .ok_or("unsafe URL exported")?;
        assert!(error.contains("InvalidExternalUri"), "{uri}: {error}");
    }
    for target in [r#"page "missing" none"#, r#"relative "other.nepld" none"#] {
        let input = format!(
            r#"article en "Links" body cons paragraph cons sentence cons link {target} text "label" nil nil nil"#
        );
        let error = export::generate(&c, &input)
            .err()
            .ok_or("unresolved URL exported")?;
        assert!(error.contains("NeedsResolution"), "{target}: {error}");
    }
    assert!(export::generate(&c, r#"article en "Links" body cons paragraph cons sentence cons ref missing text "label" nil nil nil"#).is_err());
    Ok(())
}

#[test]
fn urls_keep_rich_labels_in_titles_and_tables_and_reject_nested_links() -> Result<(), String> {
    let c = compiled()?;
    let input = r#"article en sentence cons link external "http://example.org/title" strong text "Title" nil body cons table cons left nil none cons row cons sentence cons link external "https://example.org/table" anno text "Cell" cons text "note" nil nil nil nil nil"#;
    let output = export::generate(&c, input)?;
    assert!(output.html.contains("href=\"http://example.org/title\""));
    assert!(output.html.contains("href=\"https://example.org/table\""));
    assert!(output.html.contains("<strong"));
    assert!(output.html.contains("note"));
    for input in [
        r#"article en "Nested" body cons paragraph cons sentence cons link external "https://example.org/outer" link external "https://example.org/inner" text "nested" nil nil nil"#,
        r#"article en "Missing" body cons paragraph cons sentence cons link external "https://example.org/" text "link" cons image asset "missing" none "missing" nil nil nil"#,
    ] {
        assert!(export::generate(&c, input).is_err());
    }
    Ok(())
}
