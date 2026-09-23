use super::*;
use nepl3_core::{
    origin::{Origin, OriginId},
    value::TypedValue,
};
use nepl3_sentence_core::{
    model as sentence,
    syntax::{NodeLocation, SentenceSyntax},
};

#[test]
fn typed_sentence_value_prints_inside_a_source_less_list() -> Result<(), String> {
    let compiled = compiled()?;
    // Establish only the host Profile. All tested content is constructed below.
    with_input(&compiled, "body nil", "Body", |_, profile, _, _| {
        use sentence::{InlineRef as I, Kind};
        let content = SentenceSyntax {
            value: sentence::SentenceValue {
                root: sentence::Root::Sentence(sentence::SentenceRef(5)),
                nodes: vec![
                    Kind::Text { text: "字".into() },
                    Kind::Text { text: "じ".into() },
                    Kind::Ruby {
                        base: I(0),
                        reading: I(1),
                    },
                    Kind::Text {
                        text: "note".into(),
                    },
                    Kind::InlineAnno {
                        base: I(2),
                        notes: vec![I(3)],
                    },
                    Kind::Sentence {
                        inlines: vec![I(4)],
                    },
                ],
                embeds: vec![],
            },
            locations: vec![
                NodeLocation {
                    origin: OriginId(0),
                    head: None,
                    cover: None
                };
                6
            ],
            origins: vec![Origin::Synthetic {
                reason: "typed list fixture".into(),
                anchor: None,
            }],
            sources: vec![],
            views: vec![],
            source_maps: vec![],
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let raw = nepl3_sentence_core::portable::syntax::to_value(
            &content,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let NdfValue::Record(record) = &raw else {
            return Err("SentenceSyntax record".into());
        };
        let document = DocumentSyntax {
            value: DocValue {
                root: DocRoot::Block(BlockRef(4)),
                nodes: vec![
                    DocKind::Sentence {
                        syntax: EmbedRef(0),
                    },
                    DocKind::Paragraph {
                        items: vec![FlowRef(0)],
                    },
                    DocKind::Body {
                        blocks: vec![BlockRef(1)],
                    },
                    DocKind::ListItem {
                        checked: Some(false),
                        body: BodyRef(2),
                    },
                    DocKind::List {
                        kind: ListKind::Unordered,
                        items: vec![ListItemRef(3)],
                    },
                ]
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    span: None,
                    origin: None,
                })
                .collect(),
                embeds: vec![DocEmbed {
                    kind: EmbedKind::Sentence,
                    content: DocContent::Value {
                        value: TypedValue::Record(record.clone()),
                    },
                }],
            },
            sources: vec![],
            origins: vec![],
            views: vec![],
            source_maps: vec![],
        };
        let identity = print::identity(&document, profile.registry(), &mut codec, &mut budget())
            .map_err(err)?;
        assert_eq!(identity.guests.len(), 1);
        let printed =
            nepl3_sentence_core::print::prefix(&content.value, &mut budget()).map_err(err)?;
        for mode in [PrintMode::Prefix, PrintMode::Compact] {
            let request = PrintRequest {
                document: document.clone(),
                mode,
                bindings: vec![print::GuestBinding {
                    schema: document.value.embeds[0].schema().clone(),
                    category: "Sentence".into(),
                    language: GuestLanguage::Sentence,
                }],
                guests: vec![print::PrintedGuest {
                    document_digest: identity.document_digest,
                    embed: EmbedRef(0),
                    guest_digest: identity.guests[0].guest_digest,
                    text: printed.clone(),
                }],
            };
            let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let PrintOutcome::Complete { artifact } = reply.outcome else {
                return Err(format!("typed list: {reply:?}"));
            };
            assert_eq!(artifact.entry, PrintEntry::Block);
            with_input(&compiled, &artifact.text, "Block", |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let actual = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Block,
                    profile.registry(),
                    &mut budget(),
                    &mut codec,
                )
                .map_err(err)?;
                assert_eq!(actual.value.root, document.value.root);
                assert_eq!(
                    actual
                        .value
                        .nodes
                        .iter()
                        .map(|n| &n.kind)
                        .collect::<Vec<_>>(),
                    document
                        .value
                        .nodes
                        .iter()
                        .map(|n| &n.kind)
                        .collect::<Vec<_>>()
                );
                assert_eq!(actual.value.embeds.len(), 1);
                let embed = &actual.value.embeds[0];
                let lowered = nepl3_suite::adapters::document::sentence::lower(
                    embed,
                    embed.schema(),
                    &[],
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                // Fixed constructor references independently describe every Ruby/Anno edge.
                assert_eq!(lowered.value, content.value);
                assert!(!actual.sources.is_empty());
                Ok(())
            })?;
            assert_eq!(request.document, document);
            assert!(document.sources.is_empty());
            assert!(content.sources.is_empty());
            assert!(
                content
                    .locations
                    .iter()
                    .all(|location| location.head.is_none() && location.cover.is_none())
            );
        }
        Ok(())
    })
}
