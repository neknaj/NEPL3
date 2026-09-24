use super::*;
use nepl3_doc_core::model::{BlockRef, DocRoot, DocumentSyntax, FlowRef};
use nepl3_suite::adapters::document::sentence::selection;

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
    assert_eq!(selected.sentences().len(), 1);
    assert_eq!(selected.occurrences().len(), 1);
    assert_eq!(
        selected.occurrences()[0].owner.kind,
        nepl3_doc_core::model::EmbedKind::SentenceInline
    );
    assert!(matches!(
        selected.sentences()[0].value.root,
        nepl3_sentence_core::model::Root::Inline(_)
    ));
    let label =
        sentence::embed(&selected.sentences()[0], registry, codec, &mut budget()).map_err(err)?;
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
        Err(selection::SelectionError::Sentence {
            error: sentence::Error::Category,
            ..
        })
    ));
    Ok(())
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
    assert_eq!(result.sentences().len(), 2);
    let occurrences = result.occurrences();
    assert_eq!(occurrences.len(), 3);
    assert_eq!(occurrences[0].sentence.index(), 0);
    assert_eq!(occurrences[1].sentence, occurrences[2].sentence);
    assert_eq!(occurrences[1].owner.embed, occurrences[2].owner.embed);
    assert_eq!(occurrences[1].owner.node, occurrences[2].owner.node);
    assert_eq!(occurrences[1].owner.depth, 4);
    assert_eq!(occurrences[2].owner.depth, 5);
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
        Err(selection::SelectionError::Sentence {
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
    let result = limited.with_depth_at_least::<_, selection::SelectionError<C::Error>>(1, |b| {
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
        Err(selection::SelectionError::Stopped(
            nepl3_core::budget::StopReason::DepthLimit
        ))
    ));
    Ok(())
}
