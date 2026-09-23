use super::*;
use nepl3_doc_core::print::{self, PrintMode, PrintOutcome, PrintRequest};

use entry::entry;
#[path = "print/entry.rs"]
mod entry;
use nepl3_doc_core::{check::Category, lower, model::*};

#[test]
fn typed_list_construction_preserves_order_and_inline_structure() -> Result<(), String> {
    let compiled = compiled()?;
    // This source only establishes the host Profile. The tested document is
    // constructed below without assembling or parsing a source template.
    with_input(&compiled, "sentence nil", "Sentence", |_, profile, _, _| {
        let kinds = vec![
            DocKind::Text { text: "字".into() },
            DocKind::Text { text: "じ".into() },
            DocKind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            DocKind::Text {
                text: "note".into(),
            },
            DocKind::Anno {
                base: InlineRef(2),
                notes: vec![InlineRef(3)],
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(4)],
            },
            DocKind::Paragraph {
                items: vec![FlowRef(5)],
            },
            DocKind::Body {
                blocks: vec![BlockRef(6)],
            },
            DocKind::ListItem {
                checked: None,
                body: BodyRef(7),
            },
            DocKind::Text {
                text: "second".into(),
            },
            DocKind::Link {
                target: LinkTarget::Relative {
                    path: "guide.md".into(),
                    fragment: Some("start".into()),
                },
                label: InlineRef(9),
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(10)],
            },
            DocKind::Paragraph {
                items: vec![FlowRef(11)],
            },
            DocKind::Body {
                blocks: vec![BlockRef(12)],
            },
            DocKind::ListItem {
                checked: Some(false),
                body: BodyRef(13),
            },
            DocKind::List {
                kind: ListKind::Unordered,
                items: vec![ListItemRef(8), ListItemRef(14)],
            },
        ];
        let document = DocumentSyntax {
            value: DocValue {
                root: DocRoot::Block(BlockRef(15)),
                nodes: kinds
                    .into_iter()
                    .map(|kind| DocNode {
                        kind,
                        locations: vec![],
                        origin: None,
                        span: None,
                    })
                    .collect(),
                embeds: vec![],
            },
            sources: vec![],
            origins: vec![],
            views: vec![],
            source_maps: vec![],
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        for mode in [PrintMode::Prefix, PrintMode::Compact] {
            let request = PrintRequest {
                document: document.clone(),
                mode,
                bindings: vec![],
                guests: vec![],
            };
            let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let PrintOutcome::Complete { artifact } = reply.outcome else {
                return Err(format!("typed list print: {reply:?}"));
            };
            let (surface, category) = entry(artifact.entry);
            with_input(&compiled, &artifact.text, surface, |tree, profile, b, a| {
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
                    category,
                    profile.registry(),
                    &mut budget(),
                    &mut codec,
                )
                .map_err(err)?;
                // The constructor fixture is already in postorder normal form.
                // Its fixed references are an independent oracle: computing
                // both sides through normalization could hide a shared defect.
                // Every edge, item order, annotation and link target is checked.
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
                assert!(!actual.sources.is_empty());
                Ok(())
            })?;
            // Printing does not invent source positions for typed constructors.
            assert_eq!(request.document, document);
            assert!(document.sources.is_empty());
            assert!(
                document
                    .value
                    .nodes
                    .iter()
                    .all(|n| n.span.is_none() && n.origin.is_none())
            );
        }
        Ok(())
    })
}
