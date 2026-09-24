use super::*;
use nepl3_doc_core::model::{BlockRef, DocRoot, DocumentSyntax, FlowRef};
use nepl3_suite::adapters::document::sentences as selection;

pub(super) fn verify_inline<C: FoundationValueCodec>(
    inline: &DocumentSyntax,
    article: &DocumentSyntax,
    compiled: &Compiled,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
) -> Result<(), String>
where
    C::Error: core::fmt::Debug,
{
    use nepl3_suite::adapters::document::sentence;
    let selected = selection::collect(
        inline,
        &compiled.others[3].schema,
        &[],
        registry,
        codec,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(selected.occurrences().len(), 1);
    let sentence_value = selected
        .sentence(selected.occurrences()[0].embed)
        .ok_or("Inline sentence")?;
    assert_eq!(
        selected.occurrences()[0].kind,
        nepl3_doc_core::model::EmbedKind::SentenceInline
    );
    assert!(matches!(
        sentence_value.value.root,
        nepl3_sentence_core::model::Root::Inline(_)
    ));
    let label = sentence::embed(sentence_value, registry, codec, &mut budget()).map_err(err)?;
    hidden_variant(article, &label, compiled, registry, codec)?;
    let mut wrong = article.clone();
    let slot = wrong
        .value
        .embeds
        .iter_mut()
        .find(|embed| embed.kind == nepl3_doc_core::model::EmbedKind::Sentence)
        .ok_or("Sentence slot")?;
    slot.content = label.content;
    let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    assert!(matches!(
        selection::collect(
            &wrong,
            &compiled.others[3].schema,
            &forms,
            registry,
            codec,
            &mut budget()
        ),
        Err(selection::Error::Sentence {
            error: sentence::Error::Category,
            ..
        })
    ));
    Ok(())
}

fn hidden_variant<C: FoundationValueCodec>(
    article: &DocumentSyntax,
    inline_label: &nepl3_doc_core::model::DocEmbed,
    compiled: &Compiled,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
) -> Result<(), String>
where
    C::Error: core::fmt::Debug,
{
    use nepl3_doc_core::model::{EmbedRef, SentenceRef, VariantRef};
    let mut document = article.clone();
    let DocRoot::Article(root) = document.value.root else {
        return Err("Article".into());
    };
    let DocKind::Article { body, .. } = document.value.nodes[root.0 as usize].kind else {
        return Err("Article kind".into());
    };
    let DocKind::Body { blocks } = &document.value.nodes[body.0 as usize].kind else {
        return Err("Body".into());
    };
    let paragraph = blocks[0];
    let DocKind::Paragraph { items } = &document.value.nodes[paragraph.0 as usize].kind else {
        return Err("Paragraph".into());
    };
    let visible = items[0].0;
    let DocKind::Sentence { syntax } = document.value.nodes[visible as usize].kind else {
        return Err("Sentence".into());
    };
    let hidden_embed = EmbedRef(document.value.embeds.len() as u64);
    document
        .value
        .embeds
        .push(document.value.embeds[syntax.0 as usize].clone());
    let hidden = append(
        &mut document,
        visible,
        DocKind::Sentence {
            syntax: hidden_embed,
        },
    );
    let english = append(
        &mut document,
        visible,
        DocKind::Variant {
            language: "en".into(),
            sentence: SentenceRef(visible),
        },
    );
    let japanese = append(
        &mut document,
        visible,
        DocKind::Variant {
            language: "ja".into(),
            sentence: SentenceRef(hidden),
        },
    );
    let parallel = append(
        &mut document,
        visible,
        DocKind::Parallel {
            variants: vec![VariantRef(english), VariantRef(japanese)],
        },
    );
    document.value.nodes[paragraph.0 as usize].kind = DocKind::Paragraph {
        items: vec![FlowRef(parallel)],
    };
    let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    let valid = selection::collect(
        &document,
        &compiled.others[3].schema,
        &forms,
        registry,
        codec,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(
        valid
            .occurrences()
            .iter()
            .map(|item| item.node)
            .collect::<Vec<_>>(),
        vec![
            match document.value.nodes[root.0 as usize].kind {
                DocKind::Article { title, .. } => title.0,
                _ => return Err("Article".into()),
            },
            visible,
            hidden
        ]
    );
    // Both language variants participate before a host chooses English output.
    // Keep the Doc role and NDF schema valid; only the guest root is incompatible.
    document.value.embeds[hidden_embed.0 as usize].content = inline_label.content.clone();
    assert!(
        matches!(selection::collect(&document, &compiled.others[3].schema, &forms, registry, codec, &mut budget()), Err(selection::Error::Sentence { embed, error: nepl3_suite::adapters::document::sentence::Error::Category }) if embed == hidden_embed)
    );
    Ok(())
}

fn append(document: &mut DocumentSyntax, template: u64, kind: DocKind) -> u64 {
    let mut node = document.value.nodes[template as usize].clone();
    node.kind = kind;
    let index = document.value.nodes.len() as u64;
    document.value.nodes.push(node);
    index
}

pub(super) fn verify<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    compiled: &Compiled,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
) -> Result<(), String>
where
    C::Error: core::fmt::Debug,
{
    let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    let mut shared = document.clone();
    let DocRoot::Article(root) = shared.value.root else {
        return Err("Article".into());
    };
    let DocKind::Article { body, .. } = shared.value.nodes[root.0 as usize].kind else {
        return Err("Article kind".into());
    };
    let DocKind::Body { blocks } = &shared.value.nodes[body.0 as usize].kind else {
        return Err("Body".into());
    };
    let paragraph = blocks[0];
    let mut nested = shared.value.nodes[paragraph.0 as usize].clone();
    nested.kind = DocKind::Paragraph {
        items: vec![FlowRef(paragraph.0)],
    };
    let nested_id = BlockRef(shared.value.nodes.len() as u64);
    shared.value.nodes.push(nested);
    let DocKind::Body { blocks } = &mut shared.value.nodes[body.0 as usize].kind else {
        return Err("Body".into());
    };
    blocks.push(nested_id);
    let result = selection::collect(
        &shared,
        &compiled.others[3].schema,
        &forms,
        registry,
        codec,
        &mut budget(),
    )
    .map_err(err)?;
    let occurrences = result.occurrences();
    assert_eq!(occurrences.len(), 3);
    assert!(result.sentence(occurrences[0].embed).is_some());
    assert!(core::ptr::eq(
        result
            .sentence(occurrences[1].embed)
            .ok_or("shared Sentence")?,
        result
            .sentence(occurrences[2].embed)
            .ok_or("shared Sentence")?
    ));
    assert_eq!(occurrences[1].embed, occurrences[2].embed);
    assert_eq!(occurrences[1].node, occurrences[2].node);
    assert_eq!(occurrences[1].depth, 4);
    assert_eq!(occurrences[2].depth, 5);
    deepest_lowering(&shared, &result, compiled, registry, codec)?;
    super::exact_resource_boundaries(|b| {
        selection::collect(
            &shared,
            &compiled.others[3].schema,
            &forms,
            registry,
            codec,
            b,
        )
        .is_ok()
    });
    let mut wrong = compiled.others[3].schema.clone();
    wrong.digest = Digest::of(b"unselected Sentence grammar");
    assert!(matches!(
        selection::collect(&shared, &wrong, &forms, registry, codec, &mut budget()),
        Err(selection::Error::Sentence {
            error: nepl3_suite::adapters::document::sentence::Error::Selection,
            ..
        })
    ));
    // Deep occurrence validation must include the caller's current depth.
    let mut measured = budget();
    selection::collect(
        &shared,
        &compiled.others[3].schema,
        &forms,
        registry,
        codec,
        &mut measured,
    )
    .map_err(err)?;
    let mut limits = measured.limits();
    limits.depth = measured.usage().depth;
    let mut limited = Budget::new(limits);
    let result = limited.with_depth_at_least::<_, selection::Error<C::Error>>(1, |b| {
        selection::collect(
            &shared,
            &compiled.others[3].schema,
            &forms,
            registry,
            codec,
            b,
        )
    });
    assert!(matches!(
        result,
        Err(selection::Error::Stopped(
            nepl3_core::budget::StopReason::DepthLimit
        ))
    ));
    Ok(())
}

fn deepest_lowering<C: FoundationValueCodec>(
    shared: &DocumentSyntax,
    selection: &selection::Selection<'_>,
    compiled: &Compiled,
    registry: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
) -> Result<(), String>
where
    C::Error: core::fmt::Debug,
{
    use nepl3_sentence_core::model::{InlineRef, Kind, Root};
    use nepl3_suite::adapters::document::sentence;
    let mut sentence = selection
        .sentence(selection.occurrences()[1].embed)
        .ok_or("body Sentence")?
        .clone();
    let Root::Sentence(root) = sentence.value.root else {
        return Err("Sentence root".into());
    };
    let Kind::Sentence { inlines } = &sentence.value.nodes[root.0 as usize] else {
        return Err("Sentence kind".into());
    };
    let mut child = inlines[0];
    let location = sentence.locations[child.0 as usize].clone();
    // This shared guest determines maximum depth, independently of the title.
    for _ in 0..32 {
        let parent = InlineRef(sentence.value.nodes.len() as u64);
        sentence.value.nodes.push(Kind::Concat {
            inlines: vec![child],
        });
        sentence.locations.push(location.clone());
        child = parent;
    }
    sentence.value.nodes[root.0 as usize] = Kind::Sentence {
        inlines: vec![child],
    };
    let guest = sentence::embed(&sentence, registry, codec, &mut budget()).map_err(err)?;
    let mut document = shared.clone();
    document.value.embeds[selection.occurrences()[1].embed.0 as usize] = guest;
    let mut local = budget();
    sentence::lower(
        &document.value.embeds[selection.occurrences()[1].embed.0 as usize],
        &compiled.others[3].schema,
        &[],
        registry,
        codec,
        &mut local,
    )
    .map_err(err)?;
    let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    let mut measured = budget();
    selection::collect(
        &document,
        &compiled.others[3].schema,
        &forms,
        registry,
        codec,
        &mut measured,
    )
    .map_err(err)?;
    // The Doc graph places this embed at depths four and five. Its one lowering
    // must use five, including the entire independent guest's measured depth.
    let expected = 5 + local.usage().depth;
    assert_eq!(measured.usage().depth, expected);
    let mut limits = measured.limits();
    limits.depth = expected - 1;
    assert!(matches!(
        selection::collect(
            &document,
            &compiled.others[3].schema,
            &forms,
            registry,
            codec,
            &mut Budget::new(limits)
        ),
        Err(selection::Error::Stopped(
            nepl3_core::budget::StopReason::DepthLimit
        ))
    ));
    Ok(())
}
