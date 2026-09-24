use super::*;
use nepl3_doc_core::{
    labels::namespace,
    pages::{self, namespace as scopes},
    portable,
};

#[test]
fn article_sentence_doc_links_resolve_in_page_namespaces() -> Result<(), String> {
    let compiled = compiled()?;
    let sources = [
        r#"article en sentence sentence cons doc link page "second" some "target" text "Go" nil body nil"#,
        r#"article en sentence sentence cons doc anchor target text "Goal" nil body nil"#,
    ];
    for native in [false, true] {
        let mut pages = Vec::new();
        let mut fragments = Vec::new();
        for (name, source) in ["first", "second"].into_iter().zip(sources) {
            with_named_input(
                native,
                &compiled,
                source,
                name,
                "Article",
                |tree, profile, b, a| {
                    let registry = profile.registry();
                    let input = tree
                        .tree()
                        .bundle
                        .validate_with_sources(registry, b, a)
                        .map_err(err)?;
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let document = lower::document(
                        &input,
                        &compiled.doc.package.schema,
                        Category::Article,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    let nepl3_doc_core::model::DocRoot::Article(root) = document.value.root else {
                        return Err("Article".into());
                    };
                    let DocKind::Article { title, .. } = document.value.nodes[root.0 as usize].kind
                    else {
                        return Err("Article kind".into());
                    };
                    let DocKind::Sentence { syntax } = document.value.nodes[title.0 as usize].kind
                    else {
                        return Err("Sentence slot".into());
                    };
                    let sentence = nepl3_suite::adapters::document::sentence::lower(
                        &document.value.embeds[syntax.0 as usize],
                        &compiled.others[3].schema,
                        &[nepl3_sentence_core::lower::ForeignInlineForm {
                            kind: "Form:DocumentInline",
                            guest_schema: &compiled.doc.package.schema,
                            guest_category: "Inline",
                        }],
                        registry,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    let selection = nepl3_suite::adapters::sentence::document_guests::collect(
                        &sentence,
                        &compiled.doc.package.schema,
                        registry,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    let (documents, occurrences) = selection.into_parts();
                    assert_eq!(documents.len(), 1);
                    assert_eq!(occurrences.len(), 1);
                    assert_eq!(occurrences[0].document.index(), 0);
                    fragments.push(documents.into_iter().next().ok_or("Doc guest")?);
                    pages.push(pages::PageDocument {
                        registration: pages::PageRegistration {
                            id: name.into(),
                            source: format!("doc/{name}.nepld"),
                            route: format!("docs/{name}.html"),
                        },
                        document,
                    });
                    Ok(())
                },
            )?;
        }
        let set = pages::PageSet {
            pages,
            files: vec![],
        };
        let registry = &compiled.doc.registry;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let encoded_set = portable::pages::set_to_value(&set, registry, &mut codec, &mut budget())
            .map_err(err)?;
        let bytes = nepl3_wire::encode(&encoded_set, &mut budget()).map_err(err)?;
        let mut fresh_admission = SourceAdmission::default();
        let mut fresh =
            FoundationCodec::new(registry, &store, &mut fresh_admission).map_err(err)?;
        let received = portable::pages::set_from_value(
            &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
            registry,
            &mut fresh,
            &mut budget(),
        )
        .map_err(err)?;
        for (received, original) in received.pages.iter().zip(&set.pages) {
            assert_eq!(received.registration, original.registration);
            assert_eq!(
                received.document.value.nodes, original.document.value.nodes,
                "nodes"
            );
            assert_eq!(
                received.document.value.root, original.document.value.root,
                "root"
            );
            // Foundation transport canonicalizes guest graph coordinates and
            // source table order. Doc-owned coordinates and every snapshot's
            // complete bytes remain independently checked here.
            assert_eq!(
                received.document.sources.len(),
                original.document.sources.len()
            );
            for source in &original.document.sources {
                assert!(received.document.sources.contains(source));
            }
            assert_eq!(
                received.document.origins, original.document.origins,
                "origins"
            );
            assert_eq!(received.document.views, original.document.views, "views");
            assert_eq!(
                received.document.source_maps, original.document.source_maps,
                "maps"
            );
        }
        assert_eq!(
            portable::pages::set_to_value(&received, registry, &mut fresh, &mut budget())
                .map_err(err)?,
            encoded_set
        );
        let mut received_fragments = Vec::new();
        for fragment in &fragments {
            let value =
                portable::to_value(fragment, registry, &mut codec, &mut budget()).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            received_fragments.push(
                portable::from_value(
                    &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                    registry,
                    &mut fresh,
                    &mut budget(),
                )
                .map_err(err)?,
            );
        }
        for (original, received) in fragments.iter().zip(&received_fragments) {
            assert_eq!(original.value.nodes, received.value.nodes);
            assert_eq!(original.value.root, received.value.root);
            assert_eq!(
                portable::to_value(original, registry, &mut codec, &mut budget()).map_err(err)?,
                portable::to_value(received, registry, &mut fresh, &mut budget()).map_err(err)?
            );
        }
        let mut identities = Vec::new();
        for (set, fragments) in [(&set, &fragments), (&received, &received_fragments)] {
            let mut admission = SourceAdmission::default();
            let mut members = Vec::new();
            for (page, fragment) in set.pages.iter().zip(fragments) {
                members.push([
                    namespace::inspect(&page.document, registry, &mut budget(), &mut admission)
                        .map_err(err)?,
                    namespace::inspect(fragment, registry, &mut budget(), &mut admission)
                        .map_err(err)?,
                ]);
            }
            let refs: Vec<_> = members
                .iter()
                .map(|members| [&members[0], &members[1]])
                .collect();
            let checked = refs
                .iter()
                .map(|members| namespace::resolve(members, &mut budget()).map_err(err))
                .collect::<Result<Vec<_>, _>>()?;
            let refs: Vec<_> = checked.iter().collect();
            let resolved =
                scopes::resolve(set, &refs, registry, &mut codec, &mut budget()).map_err(err)?;
            let plan = &resolved.members()[1];
            assert_eq!(
                plan.owner(),
                scopes::Owner {
                    page: 0,
                    member: namespace::MemberId(1)
                }
            );
            let [link] = plan.links() else {
                return Err("one page link".into());
            };
            assert_eq!(link.target, pages::PageDestination::Page { index: 1 });
            assert_eq!(link.fragment.as_deref(), Some("target"));
            let owner = resolved.document(plan.owner()).ok_or("link owner")?;
            let span = owner.value.nodes[link.node as usize]
                .span
                .as_ref()
                .ok_or("link source")?;
            assert_eq!(
                span.snapshot_ref().digest,
                Digest::of(sources[0].as_bytes())
            );
            let start = sources[0].find("link page").ok_or("link start")? as u64;
            assert_eq!(span.start(), start);
            let snapshot = owner
                .sources
                .iter()
                .find(|snapshot| snapshot.text() == sources[0])
                .ok_or("original input snapshot")?;
            assert_eq!(span.snapshot_ref(), snapshot.identity());
            let operand = r#"link page "second" some "target" text "Go""#;
            assert_eq!(span.end(), start + operand.len() as u64);
            assert_eq!(
                snapshot
                    .slice_range(span.start(), span.end())
                    .map_err(err)?,
                operand
            );
            identities.push(resolved.identity());
        }
        assert_eq!(identities[0], identities[1]);
    }
    Ok(())
}
