use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        SyntaxBundle, SyntaxNode,
    },
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    labels::namespace as names,
    model::*,
    pages::{self, namespace as pagespaces},
    portable,
};
use nepl3_wire::foundation::FoundationCodec;
#[path = "namespaces/portable.rs"]
mod packets;
#[path = "support/closure.rs"]
mod support;

fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 256,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}
fn err(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        r.register(
            descriptor.reference(&mut b()).map_err(err)?,
            descriptor,
            &mut b(),
        )
        .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn document(
    kinds: Vec<DocKind>,
    root: DocRoot,
    role: EmbedKind,
    r: &SchemaRegistry,
) -> Result<DocumentSyntax, String> {
    let mut closure = support::closure(r)?;
    closure.syntax.category = if role == EmbedKind::Sentence {
        "Sentence"
    } else {
        "Inline"
    }
    .into();
    Ok(DocumentSyntax {
        value: DocValue {
            root,
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    origin: None,
                    span: None,
                    locations: vec![],
                })
                .collect(),
            embeds: vec![DocEmbed {
                kind: role,
                content: DocContent::Syntax {
                    closure: Box::new(closure),
                },
            }],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    })
}
fn article(r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    document(
        vec![
            DocKind::Article {
                language: "ja".into(),
                title: SentenceRef(1),
                body: BodyRef(2),
            },
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Body { blocks: vec![] },
        ],
        DocRoot::Article(ArticleRef(0)),
        EmbedKind::Sentence,
        r,
    )
}
fn inline(kind: DocKind, r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    document(
        vec![kind],
        DocRoot::Inline(InlineRef(0)),
        EmbedKind::SentenceInline,
        r,
    )
}
fn set(r: &SchemaRegistry) -> Result<pages::PageSet, String> {
    Ok(pages::PageSet {
        pages: ["first", "second"]
            .into_iter()
            .map(|id| {
                Ok(pages::PageDocument {
                    registration: pages::PageRegistration {
                        id: id.into(),
                        source: format!("doc/{id}.nepld"),
                        route: format!("docs/{id}.html"),
                    },
                    document: article(r)?,
                })
            })
            .collect::<Result<_, String>>()?,
        files: vec![pages::PageFile {
            registration: pages::PageRegistration {
                id: "attachment".into(),
                source: "data/attachment.bin".into(),
                route: "files/attachment.bin".into(),
            },
            content: pages::FileBytes(vec![0, 255]),
        }],
    })
}

#[test]
fn selected_members_bind_link_owners_order_and_identity() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let anchor = inline(
        DocKind::Anchor {
            id: "target".into(),
            label: EmbedRef(0),
        },
        &r,
    )?;
    let link = inline(
        DocKind::Link {
            target: LinkTarget::Page {
                page: "second".into(),
                fragment: Some("target".into()),
            },
            label: EmbedRef(0),
        },
        &r,
    )?;
    let mut admission = SourceAdmission::default();
    let roots = set
        .pages
        .iter()
        .map(|page| names::inspect(&page.document, &r, &mut b(), &mut admission).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let a = names::inspect(&anchor, &r, &mut b(), &mut admission).map_err(err)?;
    let l = names::inspect(&link, &r, &mut b(), &mut admission).map_err(err)?;
    // The same decoded guest has two distinct display occurrences.
    let first = [&roots[0], &l, &l];
    let second = [&roots[1], &a];
    let first = names::resolve(&first, &mut b()).map_err(err)?;
    let second = names::resolve(&second, &mut b()).map_err(err)?;
    let namespaces = [&first, &second];
    let sources = SourceStore::default();
    let mut codec = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let mut measured = b();
    let checked =
        pagespaces::resolve(&set, &namespaces, &r, &mut codec, &mut measured).map_err(err)?;
    assert_eq!(checked.members().len(), 5);
    for member in [1, 2] {
        let plan = &checked.members()[member];
        assert_eq!(
            plan.owner(),
            pagespaces::Owner {
                page: 0,
                member: names::MemberId(member as u64)
            }
        );
        assert!(core::ptr::eq(
            checked.document(plan.owner()).ok_or("owner")?,
            &link
        ));
        assert_eq!(
            plan.links(),
            &[pages::PageLink {
                page: 0,
                node: 0,
                target: pages::PageDestination::Page { index: 1 },
                fragment: Some("target".into())
            }]
        );
        assert_eq!(
            plan.remaining().len(),
            1,
            "Sentence label stays an explicit requirement"
        );
    }
    assert!(
        checked
            .document(pagespaces::Owner {
                page: 2,
                member: names::MemberId(0)
            })
            .is_none()
    );
    // Independent oracle for the documented domain and ordered digest input.
    let value = portable::pages::set_to_value(&set, &r, &mut codec, &mut b()).map_err(err)?;
    let mut scopes = Vec::new();
    for namespace in &namespaces {
        let mut members = Vec::new();
        for document in namespace.documents() {
            let value = portable::to_value(document, &r, &mut codec, &mut b()).map_err(err)?;
            let mut bytes = nepl3_doc_core::prepare::DOCUMENT_DOMAIN.to_vec();
            bytes.extend(nepl3_wire::encode(&value, &mut b()).map_err(err)?);
            members.push(NdfValue::Bytes(Digest::of(&bytes).0.to_vec()));
        }
        scopes.push(NdfValue::List(members));
    }
    let mut bytes = pagespaces::DOMAIN.to_vec();
    let NdfValue::Record(set_value) = &value else {
        return Err("PageSet".into());
    };
    let [NdfValue::List(pages), files] = set_value.fields.as_slice() else {
        return Err("pages".into());
    };
    let mut registrations = Vec::new();
    for page in pages {
        let NdfValue::Record(page) = page else {
            return Err("PageDocument".into());
        };
        let [registration, _] = page.fields.as_slice() else {
            return Err("page fields".into());
        };
        registrations.push(registration.clone());
    }
    bytes.extend(
        nepl3_wire::encode(
            &NdfValue::List(vec![
                NdfValue::List(registrations),
                files.clone(),
                NdfValue::List(scopes),
            ]),
            &mut b(),
        )
        .map_err(err)?,
    );
    assert_eq!(checked.identity(), Digest::of(&bytes));
    let short = [&roots[0], &l];
    let short = names::resolve(&short, &mut b()).map_err(err)?;
    assert_ne!(
        pagespaces::resolve(&set, &[&short, &second], &r, &mut codec, &mut b())
            .map_err(err)?
            .identity(),
        checked.identity()
    );
    assert!(matches!(
        pagespaces::resolve(&set, &[&second, &first], &r, &mut codec, &mut b()),
        Err(pagespaces::Error::Selection)
    ));
    assert!(matches!(
        pagespaces::resolve(&set, &[&first], &r, &mut codec, &mut b()),
        Err(pagespaces::Error::Selection)
    ));
    let absent = [&roots[1]];
    let absent = names::resolve(&absent, &mut b()).map_err(err)?;
    assert!(matches!(
        pagespaces::resolve(&set, &[&first, &absent], &r, &mut codec, &mut b()),
        Err(pagespaces::Error::Link {
            owner: pagespaces::Owner {
                page: 0,
                member: names::MemberId(1)
            },
            error: pages::PageError::MissingFragment {
                page: 0,
                node: 0,
                target: 1
            }
        })
    ));
    let usage = measured.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, usage.work),
        (StopReason::AllocationLimit, usage.allocation_units),
        (StopReason::NodeLimit, usage.nodes),
        (StopReason::DepthLimit, usage.depth),
    ] {
        for limit in [amount - 1, amount] {
            let mut limits = b().limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::NodeLimit => limits.nodes = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!(),
            }
            let mut limited = Budget::new(limits);
            let result = pagespaces::resolve(&set, &namespaces, &r, &mut codec, &mut limited);
            if limit == amount {
                assert_eq!(result.map_err(err)?.identity(), checked.identity());
            } else {
                assert!(
                    matches!(result, Err(pagespaces::Error::Stopped(actual)) if actual == reason)
                );
                assert_eq!(limited.poll(), Err(reason));
            }
        }
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        pagespaces::resolve(&set, &namespaces, &r, &mut codec, &mut cancelled),
        Err(pagespaces::Error::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn namespace_identity_binds_attachment_content_and_registration() -> Result<(), String> {
    fn identity(set: &pages::PageSet, r: &SchemaRegistry) -> Result<Digest, String> {
        let mut admission = SourceAdmission::default();
        let roots = set
            .pages
            .iter()
            .map(|page| names::inspect(&page.document, r, &mut b(), &mut admission).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let first = [&roots[0]];
        let second = [&roots[1]];
        let first = names::resolve(&first, &mut b()).map_err(err)?;
        let second = names::resolve(&second, &mut b()).map_err(err)?;
        let sources = SourceStore::default();
        let mut codec = FoundationCodec::new(r, &sources, &mut admission).map_err(err)?;
        Ok(
            pagespaces::resolve(set, &[&first, &second], r, &mut codec, &mut b())
                .map_err(err)?
                .identity(),
        )
    }
    let r = registry()?;
    let original = set(&r)?;
    let expected = identity(&original, &r)?;
    // Each independent change must invalidate the proof, including unchanged-size file bytes.
    for change in 0..4 {
        let mut changed = set(&r)?;
        match change {
            0 => changed.files[0].content.0[1] = 254,
            1 => changed.files[0].registration.route = "other/attachment.bin".into(),
            2 => changed.files[0].registration.source = "other/attachment.bin".into(),
            3 => changed.files[0].registration.id = "renamed".into(),
            _ => unreachable!(),
        }
        assert_ne!(identity(&changed, &r)?, expected);
    }
    Ok(())
}

#[test]
fn composed_links_keep_relative_file_and_rejection_rules() -> Result<(), String> {
    enum Expected {
        Page(u64),
        File,
        Relative,
        Missing,
        Fragment(u64),
        FileFragment,
    }
    let r = registry()?;
    let mut set = set(&r)?;
    set.files.clear();
    set.files.push(pages::PageFile {
        registration: pages::PageRegistration {
            id: "data".into(),
            source: "data/raw.bin".into(),
            route: "files/raw.bin".into(),
        },
        content: pages::FileBytes(vec![0, 255]),
    });
    let anchor = inline(
        DocKind::Anchor {
            id: "target".into(),
            label: EmbedRef(0),
        },
        &r,
    )?;
    let cases = [
        (
            LinkTarget::Page {
                page: "second".into(),
                fragment: Some("target".into()),
            },
            Expected::Page(1),
        ),
        (
            LinkTarget::Relative {
                path: ".././doc/second.nepld".into(),
                fragment: Some("target".into()),
            },
            Expected::Page(1),
        ),
        (
            LinkTarget::Relative {
                path: "".into(),
                fragment: Some("未登録".into()),
            },
            Expected::Fragment(0),
        ),
        (
            LinkTarget::Relative {
                path: "./second.nepld".into(),
                fragment: Some("target".into()),
            },
            Expected::Page(1),
        ),
        (
            LinkTarget::Relative {
                path: "".into(),
                fragment: Some("target".into()),
            },
            Expected::Page(0),
        ),
        (
            LinkTarget::Relative {
                path: "../data/raw.bin".into(),
                fragment: None,
            },
            Expected::File,
        ),
        (
            LinkTarget::Relative {
                path: "".into(),
                fragment: None,
            },
            Expected::Relative,
        ),
        (
            LinkTarget::Relative {
                path: "".into(),
                fragment: Some(String::new()),
            },
            Expected::Relative,
        ),
        (
            LinkTarget::Relative {
                path: "../../escape".into(),
                fragment: None,
            },
            Expected::Relative,
        ),
        (
            LinkTarget::Relative {
                path: "%73econd.nepld".into(),
                fragment: None,
            },
            Expected::Relative,
        ),
        (
            LinkTarget::Relative {
                path: "missing.nepld".into(),
                fragment: None,
            },
            Expected::Missing,
        ),
        (
            LinkTarget::Page {
                page: "data".into(),
                fragment: None,
            },
            Expected::Missing,
        ),
        (
            LinkTarget::Page {
                page: "second".into(),
                fragment: Some("absent".into()),
            },
            Expected::Fragment(1),
        ),
        (
            LinkTarget::Relative {
                path: "../data/raw.bin".into(),
                fragment: Some("target".into()),
            },
            Expected::FileFragment,
        ),
    ];
    for (target, expected) in cases {
        let link = inline(
            DocKind::Link {
                target,
                label: EmbedRef(0),
            },
            &r,
        )?;
        let mut admission = SourceAdmission::default();
        let roots = set
            .pages
            .iter()
            .map(|p| names::inspect(&p.document, &r, &mut b(), &mut admission).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let a = names::inspect(&anchor, &r, &mut b(), &mut admission).map_err(err)?;
        let l = names::inspect(&link, &r, &mut b(), &mut admission).map_err(err)?;
        let first = [&roots[0], &a, &l];
        let second = [&roots[1], &a];
        let first = names::resolve(&first, &mut b()).map_err(err)?;
        let second = names::resolve(&second, &mut b()).map_err(err)?;
        let namespaces = [&first, &second];
        let sources = SourceStore::default();
        let mut codec = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
        let result = pagespaces::resolve(&set, &namespaces, &r, &mut codec, &mut b());
        match expected {
            Expected::Page(index) => assert_eq!(
                result.map_err(err)?.members()[2].links()[0].target,
                pages::PageDestination::Page { index }
            ),
            Expected::File => assert_eq!(
                result.map_err(err)?.members()[2].links()[0].target,
                pages::PageDestination::File { index: 0 }
            ),
            expected => {
                let Err(pagespaces::Error::Link { owner, error }) = result else {
                    return Err("scoped link failure".into());
                };
                assert_eq!(
                    owner,
                    pagespaces::Owner {
                        page: 0,
                        member: names::MemberId(2)
                    }
                );
                match expected {
                    Expected::Relative => assert!(matches!(
                        error,
                        pages::PageError::InvalidRelative { page: 0, node: 0 }
                    )),
                    Expected::Missing => assert!(matches!(
                        error,
                        pages::PageError::MissingPage { page: 0, node: 0 }
                    )),
                    Expected::Fragment(expected) => assert!(matches!(
                        error,
                        pages::PageError::MissingFragment {
                            page: 0,
                            node: 0,
                            target
                        } if target == expected
                    )),
                    Expected::FileFragment => assert!(matches!(
                        error,
                        pages::PageError::FileFragment {
                            page: 0,
                            node: 0,
                            file: 0
                        }
                    )),
                    _ => unreachable!("success handled above"),
                }
            }
        }
    }
    Ok(())
}
