use super::*;
use nepl3_doc_core::{
    labels::namespace,
    pages::{self, namespace as scopes},
    portable,
};

mod article;
mod composition;
mod discovery;
mod selection;

#[test]
fn article_sentence_doc_links_resolve_in_page_namespaces() -> Result<(), String> {
    let compiled = compiled()?;
    let sources = [
        r#"article en sentence sentence cons doc link page "second" some "target" text "Go" nil body cons paragraph cons sentence "First body" nil nil"#,
        r#"article en sentence sentence cons doc anchor target text "Goal" nil body cons paragraph cons sentence "Second body" nil nil"#,
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
                    selection::verify(&document, &compiled, registry, &mut codec)?;
                    let DocKind::Article { title, .. } = document.value.nodes[root.0 as usize].kind
                    else {
                        return Err("Article kind".into());
                    };
                    let DocKind::Sentence { syntax } = document.value.nodes[title.0 as usize].kind
                    else {
                        return Err("Sentence slot".into());
                    };
                    let selected = crate::doc::export::pages::discovery::collect(
                        &document,
                        &compiled.others[3].schema,
                        &compiled.doc.package.schema,
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
                    assert_eq!(selected.members().len(), 2);
                    let root = &selected.members()[0];
                    assert_eq!(root.occurrences().len(), 2);
                    assert_eq!(root.occurrences()[0].embed, syntax);
                    assert_eq!(root.occurrences()[0].node, title.0);
                    assert_eq!(root.sentences().iter().flatten().count(), 2);
                    assert_eq!(root.guests().len(), 1);
                    assert_eq!(root.guests()[0].document.index(), 1);
                    let fragment = selected.members()[1].document();
                    selection::verify_inline(fragment, &document, &compiled, registry, &mut codec)?;
                    fragments.push(fragment.clone());
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
            files: vec![pages::PageFile {
                registration: pages::PageRegistration {
                    id: "download".into(),
                    source: "download.txt".into(),
                    route: "docs/download.txt".into(),
                },
                content: pages::FileBytes(b"download".to_vec()),
            }],
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
            let mut discovered = Vec::new();
            for (page, fragment) in set.pages.iter().zip(fragments) {
                let found = crate::doc::export::pages::discovery::collect(
                    &page.document,
                    &compiled.others[3].schema,
                    &compiled.doc.package.schema,
                    &[nepl3_sentence_core::lower::ForeignInlineForm {
                        kind: "Form:DocumentInline",
                        guest_schema: &compiled.doc.package.schema,
                        guest_category: "Inline",
                    }],
                    registry,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(found.members()[1].document(), fragment);
                discovered.push(found);
            }
            let members = discovered
                .iter()
                .map(|found| {
                    crate::doc::export::pages::discovery::namespace::inspect(
                        found,
                        registry,
                        &mut budget(),
                        &mut admission,
                    )
                    .map_err(err)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let refs = members
                .iter()
                .map(|plan| plan.member_refs(&mut budget()).map_err(err))
                .collect::<Result<Vec<_>, _>>()?;
            let checked = refs
                .iter()
                .map(|members| namespace::resolve(members, &mut budget()).map_err(err))
                .collect::<Result<Vec<_>, _>>()?;
            let refs: Vec<_> = checked.iter().collect();
            let resolved =
                scopes::resolve(set, &refs, registry, &mut codec, &mut budget()).map_err(err)?;
            for member in resolved.members() {
                let document = resolved.document(member.owner()).ok_or("member owner")?;
                // Independently encode each exact owner, including non-root
                // members. Reusing the root at another member must change this
                // canonical byte digest, even when their source is shared.
                let value = portable::to_value(document, registry, &mut codec, &mut budget())
                    .map_err(err)?;
                let mut bytes = nepl3_doc_core::prepare::DOCUMENT_DOMAIN.to_vec();
                bytes.extend(nepl3_wire::encode(&value, &mut budget()).map_err(err)?);
                assert_eq!(
                    member.document_digest(),
                    nepl3_core::source::Digest::of(&bytes)
                );
            }
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
            let options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Rows,
            };
            let prepared =
                nepl3_doc_html::pages::namespace::prepare(&resolved, &options, &mut budget())
                    .map_err(err)?;
            article::verify(
                &prepared,
                &members,
                &compiled,
                registry,
                &mut codec,
                &mut budget(),
            )?;
            let mut measured = budget();
            let pending = nepl3_doc_html::pages::namespace::render_member(
                &prepared,
                plan.owner(),
                &mut |guest, _, b| {
                    super::namespace::sentence_label(guest, &compiled, registry, &mut codec, b)
                },
                &mut measured,
            )
            .map_err(err)?;
            assert_eq!(pending.owner(), plan.owner());
            assert_eq!(pending.namespace_identity(), resolved.identity());
            let output = pending.output();
            assert_eq!(output.fragment.document_digest, plan.document_digest());
            assert_eq!(output.foreign.len(), 1);
            use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};
            assert!(output.fragment.origins.iter().any(|origin| origin.node == link.node && matches!(&output.fragment.markup.fragment.nodes[origin.element as usize], HtmlNode::Element { attributes, .. } if attributes.contains(&HtmlAttribute::Href { value: HtmlHref::BetweenArtifacts { source: "docs/first.html".into(), target: "docs/second.html".into(), fragment: Some("n-746172676574".into()) } }))));
            let placement = &output.foreign[0];
            let range = placement.first_element as usize
                ..(placement.first_element + placement.elements) as usize;
            assert!(
                output.fragment.markup.fragment.nodes[range]
                    .iter()
                    .any(|node| matches!(node, HtmlNode::Text { text } if text == "Go"))
            );
            let mut calls = 0;
            assert!(matches!(
                nepl3_doc_html::pages::namespace::render_member(
                    &prepared,
                    scopes::Owner {
                        page: 0,
                        member: namespace::MemberId(99)
                    },
                    &mut |_, _, _| {
                        calls += 1;
                        Err::<nepl3_markup::html::HtmlRequest, ()>(())
                    },
                    &mut budget()
                ),
                Err(nepl3_doc_html::pages::namespace::MemberError::Selection(_))
            ));
            assert_eq!(calls, 0);
            use nepl3_core::budget::StopReason;
            use nepl3_doc_html::pages::namespace as html;
            for reason in [
                StopReason::WorkLimit,
                StopReason::AllocationLimit,
                StopReason::Cancelled,
            ] {
                let mut stopped = budget();
                stopped.stop(reason);
                assert!(
                    matches!(html::prepare(&resolved, &options, &mut stopped), Err(html::PreparationError::Stopped(actual)) if actual == reason)
                );
                assert!(
                    matches!(html::render_member(&prepared, plan.owner(), &mut |_, _, _| { calls += 1; Err::<nepl3_markup::html::HtmlRequest, ()>(()) }, &mut stopped), Err(html::MemberError::Stopped(actual)) if actual == reason)
                );
                assert_eq!(calls, 0);
                let mut stopped = budget();
                let result = html::render_member(
                    &prepared,
                    plan.owner(),
                    &mut |_, _, b| {
                        b.stop(reason);
                        Ok::<_, ()>(output.fragment.markup.clone())
                    },
                    &mut stopped,
                );
                assert!(
                    matches!(result, Err(html::MemberError::Stopped(actual)) if actual == reason)
                );
            }
            assert!(matches!(
                html::render_member(
                    &prepared,
                    plan.owner(),
                    &mut |_, _, _| {
                        calls += 1;
                        Err::<nepl3_markup::html::HtmlRequest, _>("adapter failure")
                    },
                    &mut budget()
                ),
                Err(html::MemberError::Render {
                    error: nepl3_doc_html::ForeignRenderError::Foreign("adapter failure"),
                    ..
                })
            ));
            assert_eq!(calls, 1);
            let target = html::render_member(
                &prepared,
                scopes::Owner {
                    page: 1,
                    member: namespace::MemberId(1),
                },
                &mut |guest, _, b| {
                    super::namespace::sentence_label(guest, &compiled, registry, &mut codec, b)
                },
                &mut budget(),
            )
            .map_err(err)?;
            // Exercise completed-output checks on the two independent Inline
            // outputs. Article insertion and its source map are separate tests.
            let requests = [
                output.fragment.markup.clone(),
                target.output().fragment.markup.clone(),
            ];
            let checked_output =
                html::output::check(&prepared, &requests, &mut budget()).map_err(err)?;
            assert_eq!(checked_output.pages().len(), 2);
            assert_eq!(checked_output.namespace_identity(), resolved.identity());
            exact_resource_boundaries(|b| html::prepare(&resolved, &options, b).is_ok());
            exact_resource_boundaries(|b| {
                html::render_member(
                    &prepared,
                    plan.owner(),
                    &mut |guest, _, b| {
                        super::namespace::sentence_label(guest, &compiled, registry, &mut codec, b)
                    },
                    b,
                )
                .is_ok()
            });
            exact_resource_boundaries(|b| html::output::check(&prepared, &requests, b).is_ok());
            assert!(matches!(
                html::output::check(&prepared, &requests[..1], &mut budget()),
                Err(html::output::Error::PageCount)
            ));
            for case in 0..6 {
                let mut modified = requests.clone();
                for node in &mut modified[0].fragment.nodes {
                    if let HtmlNode::Element { attributes, .. } = node {
                        for attribute in attributes {
                            if let HtmlAttribute::Href {
                                value:
                                    HtmlHref::BetweenArtifacts {
                                        source,
                                        target,
                                        fragment,
                                    },
                            } = attribute
                            {
                                match case {
                                    0 => *source = "docs/wrong.html".into(),
                                    1 => *target = "docs/missing.html".into(),
                                    2 => *fragment = Some("n-missing".into()),
                                    3 => {
                                        *attribute = HtmlAttribute::Href {
                                            value: HtmlHref::External {
                                                uri: "javascript:alert(1)".into(),
                                            },
                                        }
                                    }
                                    _ => {
                                        *target = "docs/download.txt".into();
                                        if case == 4 {
                                            *fragment = None;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if case == 0 {
                    // Safe custom Borrow implementations can switch arenas.
                    // Route checking must use the exact arena validated first.
                    struct Switching<'a> {
                        first: &'a nepl3_markup::html::HtmlRequest,
                        later: &'a nepl3_markup::html::HtmlRequest,
                        calls: core::cell::Cell<usize>,
                    }
                    impl core::borrow::Borrow<nepl3_markup::html::HtmlRequest> for Switching<'_> {
                        fn borrow(&self) -> &nepl3_markup::html::HtmlRequest {
                            let calls = self.calls.get();
                            self.calls.set(calls + 1);
                            if calls == 0 { self.first } else { self.later }
                        }
                    }
                    let switched = [0, 1].map(|index| Switching {
                        first: &modified[index],
                        later: &requests[index],
                        calls: core::cell::Cell::new(0),
                    });
                    assert!(matches!(
                        html::output::check(&prepared, &switched, &mut budget()),
                        Err(html::output::Error::SourceRoute { page: 0, .. })
                    ));
                    assert!(switched.iter().all(|request| request.calls.get() == 1));
                }
                let result = html::output::check(&prepared, &modified, &mut budget());
                match case {
                    0 => assert!(matches!(
                        result,
                        Err(html::output::Error::SourceRoute { page: 0, .. })
                    )),
                    1 => assert!(matches!(
                        result,
                        Err(html::output::Error::TargetRoute { page: 0, .. })
                    )),
                    2 => assert!(matches!(
                        result,
                        Err(html::output::Error::MissingAnchor {
                            page: 0,
                            target: 1,
                            ..
                        })
                    )),
                    3 => assert!(matches!(
                        result,
                        Err(html::output::Error::Html { page: 0, .. })
                    )),
                    4 => assert!(result.is_ok()),
                    _ => assert!(matches!(
                        result,
                        Err(html::output::Error::FileFragment { page: 0, .. })
                    )),
                }
            }
            identities.push(resolved.identity());
        }
        assert_eq!(identities[0], identities[1]);
    }
    Ok(())
}

fn exact_resource_boundaries(mut run: impl FnMut(&mut Budget) -> bool) {
    use nepl3_core::budget::StopReason;
    let mut measured = budget();
    assert!(run(&mut measured));
    let used = measured.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, used.work),
        (StopReason::AllocationLimit, used.allocation_units),
    ] {
        assert!(amount > 0);
        for limit in [amount - 1, amount] {
            let mut limits = measured.limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                _ => unreachable!("fixed cases"),
            }
            let mut limited = Budget::new(limits);
            assert_eq!(run(&mut limited), limit == amount);
            assert_eq!(
                limited.poll(),
                if limit == amount { Ok(()) } else { Err(reason) }
            );
        }
    }
}

#[test]
fn page_preparation_preserves_unresolved_asset_requirement() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons image asset "figure.svg" none sentence "Alternative" none nil"#;
    for native in [false, true] {
        with_named_input(
            native,
            &compiled,
            source,
            "asset",
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
                let set = pages::PageSet {
                    pages: vec![pages::PageDocument {
                        registration: pages::PageRegistration {
                            id: "asset".into(),
                            source: "asset.nepld".into(),
                            route: "asset.html".into(),
                        },
                        document,
                    }],
                    files: vec![],
                };
                let mut admission = SourceAdmission::default();
                let member =
                    namespace::inspect(&set.pages[0].document, registry, b, &mut admission)
                        .map_err(err)?;
                let members = [&member];
                let checked = namespace::resolve(&members, b).map_err(err)?;
                let namespaces = [&checked];
                let resolved =
                    scopes::resolve(&set, &namespaces, registry, &mut codec, b).map_err(err)?;
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Rows,
                };
                assert!(
                    matches!(nepl3_doc_html::pages::namespace::prepare(&resolved, &options, b), Err(nepl3_doc_html::pages::namespace::PreparationError::NeedsResolution { owner: scopes::Owner { page: 0, member: namespace::MemberId(0) }, requirement: nepl3_doc_core::prepare::DocRequirement::Asset { asset, .. } }) if asset.id == "figure.svg")
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}
