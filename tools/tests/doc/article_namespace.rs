use super::*;
use nepl3_doc_core::labels::namespace::{self, MemberId};
use nepl3_doc_html::{ParallelMode, RenderOptions, namespace as html_namespace};
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode, validate};
use nepl3_sentence_core::lower::ForeignInlineForm;
use nepl3_suite::adapters::{
    document::sentence,
    sentence::{document_guests, html},
};

#[derive(Debug)]
struct Failure(String);
impl From<nepl3_core::budget::StopReason> for Failure {
    fn from(reason: nepl3_core::budget::StopReason) -> Self {
        Self(err(reason))
    }
}
fn failure(error: impl core::fmt::Debug) -> Failure {
    Failure(err(error))
}

#[test]
fn article_section_resolves_sentence_guest_reference_after_first_receive() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body
        cons paragraph cons sentence sentence cons doc ref target text "go" nil nil
        cons section target sentence "Target" body nil
        nil"#;
    with_input(&compiled, source, "Article", |tree, profile, b, a| {
        let registry = profile.registry();
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
        let doc = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        let raw = portable::to_value(&doc, registry, &mut codec, &mut budget()).map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let received = portable::from_value(
            &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
            registry,
            &mut receiver,
            &mut budget(),
        )
        .map_err(err)?;
        retention::assert_doc_retention(&doc, &received)?;
        let surface = registry
            .selected("nepl3.syntax.sentence", 1)
            .ok_or("Sentence surface")?;
        let forms = [ForeignInlineForm {
            kind: "Form:DocumentInline",
            guest_schema: &compiled.doc.package.schema,
            guest_category: "Inline",
        }];
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let mut expected = None;
        for document in [&doc, &received] {
            let selected = nepl3_suite::adapters::document::sentences::collect(
                document,
                surface,
                &forms,
                registry,
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert!(core::ptr::eq(selected.document(), document));
            assert_eq!(selected.occurrences().len(), 3);
            let mut sentences = Vec::new();
            let mut selections = Vec::new();
            for (index, slot) in document.value.embeds.iter().enumerate() {
                assert_eq!(slot.kind, EmbedKind::Sentence);
                let input = selected
                    .sentence(EmbedRef(index as u64))
                    .ok_or("selected Sentence")?;
                let selection = document_guests::collect(
                    input,
                    &compiled.doc.package.schema,
                    registry,
                    &mut receiver,
                    &mut budget(),
                )
                .map_err(err)?;
                sentences.push(input);
                selections.push(selection);
            }
            assert_eq!(sentences.len(), 3);
            let guests: Vec<_> = selections.iter().flat_map(|s| s.documents()).collect();
            assert_eq!(guests.len(), 1);
            assert!(matches!(&guests[0].value.nodes[0].kind,
                DocKind::Reference { target, .. } if target == "target"));
            let mut members = Vec::new();
            for member in core::iter::once(document).chain(guests.iter().copied()) {
                members.push(
                    namespace::inspect(
                        member,
                        registry,
                        &mut budget(),
                        &mut SourceAdmission::default(),
                    )
                    .map_err(err)?,
                );
            }
            let refs: Vec<_> = members.iter().collect();
            let namespace = namespace::resolve(&refs, &mut budget()).map_err(err)?;
            assert_eq!(namespace.definitions().len(), 1);
            assert_eq!(namespace.references().len(), 1);
            assert_eq!(namespace.definitions()[0].member, MemberId(0));
            assert_eq!(namespace.references()[0].reference.member, MemberId(1));
            for (site, owner, spelling, prefix) in [
                (
                    namespace.definitions()[0].site,
                    document,
                    "section target",
                    "section ",
                ),
                (
                    namespace.references()[0].reference.site,
                    guests[0],
                    "ref target",
                    "ref ",
                ),
            ] {
                let span = site.selection.ok_or("name selection")?;
                let start = (source.find(spelling).ok_or("fixture name")? + prefix.len()) as u64;
                assert_eq!((span.start(), span.end()), (start, start + 6));
                let snapshot = owner
                    .sources
                    .iter()
                    .find(|s| s.text() == source)
                    .ok_or("member's original source")?;
                assert_eq!(span.snapshot_ref(), snapshot.identity());
            }
            // A Sentence's Doc reference cannot resolve without its Article.
            assert!(matches!(
                namespace::resolve(&[&members[1]], &mut budget()),
                Err(namespace::Error::Unresolved { .. })
            ));
            assert!(matches!(
                namespace::resolve(&[&members[0], &members[0], &members[1]], &mut budget()),
                Err(namespace::Error::Duplicate { .. })
            ));
            let prepared = html_namespace::prepare_with_foreign(
                &namespace,
                &options,
                registry,
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            let mut occurrences = Vec::new();
            let rendered = html_namespace::render_part_with_foreign(
                &prepared,
                MemberId(0),
                &mut |_, embed, b| -> Result<_, Failure> {
                    let input = &sentences[embed.0 as usize];
                    let selection = &selections[embed.0 as usize];
                    let part = html::render_part_with_foreign(
                        input,
                        registry,
                        &mut |_, guest, b| -> Result<_, Failure> {
                            assert_eq!(selection.occurrences().len(), 1);
                            assert_eq!(selection.occurrences()[0].embed, guest);
                            let part = html_namespace::render_part_with_foreign(
                                &prepared,
                                MemberId(1),
                                &mut |label, _, b| -> Result<_, Failure> {
                                    let label = sentence::lower(
                                        label,
                                        surface,
                                        &[],
                                        registry,
                                        &mut receiver,
                                        b,
                                    )
                                    .map_err(failure)?;
                                    html::render(
                                        &label,
                                        registry,
                                        b,
                                        &mut SourceAdmission::default(),
                                    )
                                    .map(|v| v.into_markup())
                                    .map_err(failure)
                                },
                                b,
                            )
                            .map_err(failure)?;
                            Ok(part.part.into_parts().2)
                        },
                        b,
                        &mut SourceAdmission::default(),
                    )
                    .map_err(failure)?;
                    let (_, markup, origins, foreign) = part.into_parts();
                    occurrences.push((embed, origins, foreign));
                    Ok(markup)
                },
                &mut budget(),
            )
            .map_err(|e| match e {
                html_namespace::ForeignPartError::Render(
                    nepl3_doc_html::ForeignRenderError::Foreign(Failure(message)),
                ) => message,
                error => err(error),
            })?;
            let placements = rendered.foreign;
            let (_, _, markup, _) = rendered.part.into_parts();
            let checked = validate(&markup.fragment, markup.slot, &markup.policy, &mut budget())
                .map_err(err)?;
            let output = nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?;
            let mut ids = Vec::new();
            let mut targets = Vec::new();
            for node in &markup.fragment.nodes {
                if let HtmlNode::Element { attributes, .. } = node {
                    for attr in attributes {
                        match attr {
                            HtmlAttribute::Id { value } => ids.push(value),
                            HtmlAttribute::Href {
                                value: HtmlHref::Fragment { id },
                            } => targets.push(id),
                            _ => (),
                        }
                    }
                }
            }
            assert_eq!(ids.len(), 1);
            assert_eq!(targets, ids);
            assert_eq!(placements.len(), 3);
            assert_eq!(occurrences.len(), 3);
            let mut local_text = Vec::new();
            let mut guest_text = Vec::new();
            for (placement, (embed, origins, foreign)) in placements.iter().zip(&occurrences) {
                assert_eq!(placement.embed, *embed);
                for origin in origins {
                    let final_element = placement.first_element + origin.element;
                    assert!(markup.fragment.nodes.get(final_element as usize).is_some());
                    assert!(
                        sentences[embed.0 as usize]
                            .locations
                            .get(origin.node as usize)
                            .is_some()
                    );
                    if let HtmlNode::Text { text } = &markup.fragment.nodes[final_element as usize]
                    {
                        match &sentences[embed.0 as usize].value.nodes[origin.node as usize] {
                            nepl3_sentence_core::model::Kind::Text { text: original } => {
                                assert_eq!(text, original);
                                local_text.push(text.as_str());
                            }
                            nepl3_sentence_core::model::Kind::ForeignInline { .. } => {
                                assert!(foreign.iter().any(|p| origin.element >= p.first_element
                                    && origin.element < p.first_element + p.elements));
                                guest_text.push(text.as_str());
                            }
                            _ => return Err("unexpected text owner".into()),
                        }
                    }
                }
            }
            assert_eq!(local_text, ["Title", "Target"]);
            assert_eq!(guest_text, ["go"]);
            if let Some(previous) = &expected {
                assert_eq!(previous, &output);
            }
            expected = Some(output);
        }
        Ok(())
    })
}
