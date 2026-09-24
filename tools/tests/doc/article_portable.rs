//! Receiver-owned reconstruction of complete Article output and guest placement.
use super::*;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
};
use nepl3_doc_html::{
    LocalHtmlRequest, ParallelMode, RenderOptions, RenderedWithForeign,
    portable::{self as html_wire, foreign},
    prepare_article_with_foreign, render_article_with_foreign,
};
use nepl3_tools::doc::source::Compiled;

fn compiled_output() -> Result<Compiled, String> {
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

fn request(compiled: &Compiled) -> Result<LocalHtmlRequest, String> {
    let source = r#"article ja sentence "Title" body cons paragraph cons sentence sentence cons ruby text "字" text "じ" cons text "🙂<&\r\n" nil nil nil"#;
    request_source(compiled, source)
}
fn request_source(compiled: &Compiled, source: &str) -> Result<LocalHtmlRequest, String> {
    with_input(compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let mut document = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        // Two display occurrences retain one semantic Sentence/closure owner.
        let paragraph = document
            .value
            .nodes
            .iter_mut()
            .find_map(|node| match &mut node.kind {
                DocKind::Paragraph { items } => Some(items),
                _ => None,
            })
            .ok_or("paragraph")?;
        let sentence = *paragraph.first().ok_or("sentence")?;
        paragraph.push(sentence);
        Ok(LocalHtmlRequest {
            document,
            options: RenderOptions {
                parallel: ParallelMode::Rows,
            },
        })
    })
}

fn render_request(
    request: &LocalHtmlRequest,
    registry: &SchemaRegistry,
) -> Result<RenderedWithForeign, String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let prepared = prepare_article_with_foreign(
        &request.document,
        &request.options,
        registry,
        &mut codec,
        &mut budget(),
    )
    .map_err(err)?;
    render_article_with_foreign(
        &prepared,
        &mut |slot, _, b| {
            let sentence = nepl3_suite::adapters::document::sentence::lower(
                slot,
                slot.schema(),
                &[],
                registry,
                &mut codec,
                b,
            )
            .map_err(err)?;
            nepl3_suite::adapters::sentence::html::render(
                &sentence,
                registry,
                b,
                &mut SourceAdmission::default(),
            )
            .map(|output| output.into_markup())
            .map_err(err)
        },
        &mut budget(),
    )
    .map_err(err)
}

#[test]
fn first_receiver_compares_article_output_and_each_shared_guest_occurrence() -> Result<(), String> {
    let compiled = compiled_output()?;
    let registry = &compiled.doc.registry;
    let request = request(&compiled)?;
    let original = render_request(&request, registry)?;
    assert_eq!(original.foreign.len(), 3);
    assert_eq!(original.foreign[1].embed, original.foreign[2].embed);
    assert_ne!(
        original.foreign[1].first_element,
        original.foreign[2].first_element
    );
    let body_owner = request
        .document
        .value
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            DocKind::Paragraph { items } => items.first().map(|node| node.0),
            _ => None,
        })
        .ok_or("body owner")?;
    for placement in &original.foreign[1..] {
        let start = placement.first_element as usize;
        let end = start + placement.elements as usize;
        let texts: Vec<_> = original.fragment.markup.fragment.nodes[start..end]
            .iter()
            .filter_map(|node| match node {
                nepl3_markup::html::HtmlNode::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["字", "じ", "🙂<&\r\n"]);
        for element in placement.first_element..placement.first_element + placement.elements {
            assert_eq!(
                original
                    .fragment
                    .origins
                    .iter()
                    .filter(|origin| origin.element == element)
                    .map(|origin| origin.node)
                    .collect::<Vec<_>>(),
                [body_owner]
            );
        }
    }
    let markup = &original.fragment.markup;
    let checked =
        nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, &mut budget())
            .map_err(err)?;
    let html = nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?;
    assert_eq!(html.matches("🙂&lt;&amp;&#xD;\n").count(), 2);
    assert_eq!(html.matches("nepl-ruby").count(), 2);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let request_bytes = nepl3_wire::encode(
        &html_wire::request_to_value(&request, registry, &mut codec, &mut budget()).map_err(err)?,
        &mut budget(),
    )
    .map_err(err)?;
    let reply_bytes = nepl3_wire::encode(
        &foreign::to_value(&original, registry, &mut codec, &mut budget()).map_err(err)?,
        &mut budget(),
    )
    .map_err(err)?;
    let mut fresh = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(registry, &empty, &mut fresh).map_err(err)?;
    let received = html_wire::request_from_value(
        &nepl3_wire::decode(&request_bytes, &mut budget()).map_err(err)?,
        registry,
        &mut receiver,
        &mut budget(),
    )
    .map_err(err)?;
    let expected = render_request(&received, registry)?;
    let value = nepl3_wire::decode(&reply_bytes, &mut budget()).map_err(err)?;
    assert_eq!(
        foreign::from_value(&value, &expected, registry, &mut receiver, &mut budget())
            .map_err(err)?,
        original
    );
    // All records remain schema-correct; these are derivation/ownership failures.
    for mutation in 0..9 {
        let mut forged = original.clone();
        match mutation {
            0 => forged.fragment.document_digest = Digest::of(b"other"),
            1 => forged.fragment.options.parallel = ParallelMode::Columns,
            2 => forged.fragment.origins.clear(),
            3 => forged.foreign.clear(),
            4 => forged.foreign[1].embed = EmbedRef(999),
            5 => forged.foreign[1].first_element += 1,
            6 => forged.foreign[1].elements -= 1,
            7 => forged.foreign.swap(1, 2),
            8 => forged.fragment.markup.policy.classes.push("forged".into()),
            _ => unreachable!("fixed mutations"),
        }
        let forged =
            foreign::to_value(&forged, registry, &mut receiver, &mut budget()).map_err(err)?;
        assert!(
            matches!(
                foreign::from_value(&forged, &expected, registry, &mut receiver, &mut budget()),
                Err(html_wire::PortableError::Mismatch)
            ),
            "mutation {mutation}"
        );
    }
    let mut columns = received.clone();
    columns.options.parallel = ParallelMode::Columns;
    let columns = render_request(&columns, registry)?;
    assert_eq!(columns.fragment.markup, expected.fragment.markup);
    assert!(matches!(
        foreign::from_value(&value, &columns, registry, &mut receiver, &mut budget()),
        Err(html_wire::PortableError::Mismatch)
    ));
    let changed = request_source(
        &compiled,
        r#"article ja sentence "Title" body cons paragraph cons sentence "Changed body" nil nil"#,
    )?;
    let changed = render_request(&changed, registry)?;
    assert!(matches!(
        foreign::from_value(&value, &changed, registry, &mut receiver, &mut budget()),
        Err(html_wire::PortableError::Mismatch)
    ));
    assert!(matches!(
        foreign::from_value(
            &nepl3_core::value::NdfValue::Unit,
            &expected,
            registry,
            &mut receiver,
            &mut budget()
        ),
        Err(html_wire::PortableError::Schema(_))
    ));
    Ok(())
}

#[test]
fn foreign_output_codec_obeys_exact_and_sticky_resource_limits() -> Result<(), String> {
    let compiled = compiled_output()?;
    let registry = &compiled.doc.registry;
    let expected = render_request(&request(&compiled)?, registry)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let mut encoding = budget();
    let value = foreign::to_value(&expected, registry, &mut codec, &mut encoding).map_err(err)?;
    let mut decoding = budget();
    foreign::from_value(&value, &expected, registry, &mut codec, &mut decoding).map_err(err)?;
    for decode in [false, true] {
        let used = if decode {
            decoding.usage()
        } else {
            encoding.usage()
        };
        assert!(used.depth > 0);
        for short in [false, true] {
            let mut limits = budget().limits();
            limits.depth = used.depth - u64::from(short);
            let mut limited = Budget::new(limits);
            let result = if decode {
                foreign::from_value(&value, &expected, registry, &mut codec, &mut limited)
                    .map(|_| ())
            } else {
                foreign::to_value(&expected, registry, &mut codec, &mut limited).map(|_| ())
            };
            if short {
                assert!(matches!(
                    result,
                    Err(html_wire::PortableError::Stopped(StopReason::DepthLimit))
                ));
                assert_eq!(limited.poll(), Err(StopReason::DepthLimit));
            } else {
                result.map_err(err)?;
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        let result = if decode {
            foreign::from_value(&value, &expected, registry, &mut codec, &mut cancelled).map(|_| ())
        } else {
            foreign::to_value(&expected, registry, &mut codec, &mut cancelled).map(|_| ())
        };
        assert!(matches!(
            result,
            Err(html_wire::PortableError::Stopped(StopReason::Cancelled))
        ));
        assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
        for (resource, count, reason) in [
            (Resource::Work, used.work, StopReason::WorkLimit),
            (
                Resource::AllocationUnits,
                used.allocation_units,
                StopReason::AllocationLimit,
            ),
            (Resource::Nodes, used.nodes, StopReason::NodeLimit),
        ] {
            assert!(count > 0);
            for short in [false, true] {
                let mut limits = budget().limits();
                let limit = count - u64::from(short);
                match resource {
                    Resource::Work => limits.work = limit,
                    Resource::AllocationUnits => limits.allocation_units = limit,
                    Resource::Nodes => limits.nodes = limit,
                    _ => unreachable!("fixed resources"),
                }
                let mut limited = Budget::new(limits);
                let result = if decode {
                    foreign::from_value(&value, &expected, registry, &mut codec, &mut limited)
                        .map(|_| ())
                } else {
                    foreign::to_value(&expected, registry, &mut codec, &mut limited).map(|_| ())
                };
                if short {
                    assert!(
                        matches!(result, Err(html_wire::PortableError::Stopped(actual)) if actual == reason)
                    );
                    assert_eq!(limited.poll(), Err(reason));
                    assert!(
                        matches!(foreign::from_value(&value, &expected, registry, &mut codec, &mut limited), Err(html_wire::PortableError::Stopped(actual)) if actual == reason)
                    );
                } else {
                    result.map_err(err)?;
                }
            }
        }
    }
    Ok(())
}
