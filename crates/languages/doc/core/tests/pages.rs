//! Root-only PageSet contracts. Selected Inline link cases live in namespaces.rs.
//! Sentence URI behavior belongs to the Article integration tests.
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
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
use nepl3_doc_core::{model::*, pages::*, portable, prepare::DocRequirement};
use nepl3_wire::foundation::FoundationCodec;
#[path = "support/closure.rs"]
mod support;
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 1000,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn doc(r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    let mut closure = support::closure(r)?;
    closure.syntax.category = "Sentence".into();
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(2),
        },
        DocKind::Sentence {
            syntax: EmbedRef(0),
        },
        DocKind::Body {
            blocks: vec![BlockRef(3)],
        },
        DocKind::Section {
            id: "導入".into(),
            title: SentenceRef(1),
            body: BodyRef(4),
        },
        DocKind::Body {
            blocks: vec![BlockRef(5)],
        },
        DocKind::RawCode {
            language_hint: None,
            text: "文書".into(),
        },
    ];
    Ok(DocumentSyntax {
        value: DocValue {
            root: DocRoot::Article(ArticleRef(0)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![DocEmbed {
                kind: EmbedKind::Sentence,
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
fn set(r: &SchemaRegistry) -> Result<PageSet, String> {
    Ok(PageSet {
        pages: ["first", "second"]
            .into_iter()
            .map(|id| {
                Ok(PageDocument {
                    registration: PageRegistration {
                        id: id.into(),
                        source: format!("doc/{id}.nepld"),
                        route: format!("docs/{id}/index.html"),
                    },
                    document: doc(r)?,
                })
            })
            .collect::<Result<_, String>>()?,
        files: vec![],
    })
}
fn run(set: &PageSet, r: &SchemaRegistry) -> Result<PageLinkPlan, String> {
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    Ok(resolve(set, r, &mut c, &mut b()).map_err(err)?.into_plan())
}

#[test]
fn batch_encoding_preserves_checked_documents_and_stops() -> Result<(), String> {
    let r = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let example = doc(&r)?;
    let mut standalone_budget = b();
    let standalone =
        portable::to_value(&example, &r, &mut codec, &mut standalone_budget).map_err(err)?;
    let mut previous_work = None;
    for count in [4, 8, 16] {
        let input = PageSet {
            pages: (0..count)
                .map(|i| PageDocument {
                    registration: PageRegistration {
                        id: format!("page-{i:02}"),
                        source: format!("doc/page-{i:02}.nepld"),
                        route: format!("docs/page-{i:02}.html"),
                    },
                    document: example.clone(),
                })
                .collect(),
            files: vec![],
        };
        let mut budget = b();
        let value =
            portable::pages::set_to_value(&input, &r, &mut codec, &mut budget).map_err(err)?;
        let NdfValue::Record(root) = &value else {
            return Err("PageSet".into());
        };
        let NdfValue::List(pages) = &root.fields[0] else {
            return Err("pages".into());
        };
        for page in pages {
            let NdfValue::Record(page) = page else {
                return Err("page".into());
            };
            assert_eq!(page.fields[1], standalone);
        }
        if let Some(previous) = previous_work {
            assert!(budget.usage().work < previous * 3);
        }
        previous_work = Some(budget.usage().work);
        // Wrappers permit 20% above one checked standalone document per page;
        // repeating the DocumentSyntax schema walk exceeds this allowance.
        assert!(budget.usage().work < standalone_budget.usage().work * count as u64 * 6 / 5);
        let mut limits = b().limits();
        limits.work = budget.usage().work - 1;
        assert!(matches!(
            portable::pages::set_to_value(&input, &r, &mut codec, &mut Budget::new(limits)),
            Err(portable::PortableError::Stopped(StopReason::WorkLimit))
        ));
        let mut corrupt = value;
        let NdfValue::Record(root) = &mut corrupt else {
            return Err("PageSet".into());
        };
        let NdfValue::List(pages) = &mut root.fields[0] else {
            return Err("pages".into());
        };
        let NdfValue::Record(page) = &mut pages[0] else {
            return Err("page".into());
        };
        page.fields[1] = NdfValue::Unit;
        assert!(portable::pages::set_from_value(&corrupt, &r, &mut codec, &mut b()).is_err());
        let mut invalid = input;
        invalid.pages[0].document.value.root = DocRoot::Article(ArticleRef(u64::MAX));
        assert!(portable::pages::set_to_value(&invalid, &r, &mut codec, &mut b()).is_err());
    }
    Ok(())
}

#[test]
fn page_boundary_validation_still_precedes_earlier_page_label_failures() -> Result<(), String> {
    let r = registry()?;
    let mut input = set(&r)?;
    input.pages[0].document.value.nodes[2].kind = DocKind::Body {
        blocks: vec![BlockRef(3), BlockRef(3)],
    };
    let original = input.pages[1].document.value.nodes[0].kind.clone();
    input.pages[1].document.value.nodes[0].kind = DocKind::Article {
        language: "ja".into(),
        title: SentenceRef(1),
        body: BodyRef(u64::MAX),
    };
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    assert!(matches!(
        resolve(&input, &r, &mut c, &mut b()),
        Err(PageError::Boundary(portable::PortableError::Structure(_)))
    ));
    input.pages[1].document.value.nodes[0].kind = original;
    assert!(matches!(
        resolve(&input, &r, &mut c, &mut b()),
        Err(PageError::Input {
            page: 0,
            error: nepl3_doc_core::prepare::PreparationError::Label(_)
        })
    ));
    Ok(())
}

#[test]
fn root_plan_binds_documents_and_retains_sentence_requirements() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let original = set.clone();
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let p = resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert!(p.plan().links.is_empty());
    assert_eq!(p.plan().remaining.len(), 2);
    for (page, requirement) in p.plan().remaining.iter().enumerate() {
        assert_eq!(requirement.page, page as u64);
        assert!(matches!(
            requirement.requirement,
            DocRequirement::Foreign {
                embed: EmbedRef(0),
                kind: EmbedKind::Sentence,
                ..
            }
        ));
    }
    let value = portable::pages::set_to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    let mut bytes = b"NEPL3.Doc.Pages.v2\0".to_vec();
    bytes.extend(nepl3_wire::encode(&value, &mut b()).map_err(err)?);
    assert_eq!(p.plan().identity, Digest::of(&bytes));
    for (index, page) in set.pages.iter().enumerate() {
        let value = portable::to_value(&page.document, &r, &mut c, &mut b()).map_err(err)?;
        let mut expected = nepl3_doc_core::prepare::DOCUMENT_DOMAIN.to_vec();
        expected.extend(nepl3_wire::encode(&value, &mut b()).map_err(err)?);
        assert_eq!(p.document_digest(index as u64), Some(Digest::of(&expected)));
        assert_eq!(
            p.document_digest(index as u64),
            Some(
                nepl3_doc_core::prepare::inspect(&page.document, &r, &mut c, &mut b())
                    .map_err(err)?
                    .document_digest
            )
        );
    }
    assert_eq!(p.document_digest(set.pages.len() as u64), None);
    assert_eq!(p.document_digest(u64::MAX), None);
    assert_eq!(set, original);
    let mut changed = set.clone();
    changed.pages[1].registration.route = "other/index.html".into();
    assert_ne!(run(&changed, &r)?.identity, p.plan().identity);
    changed = set.clone();
    changed.pages[1].document.value.nodes[5].kind = DocKind::RawCode {
        language_hint: None,
        text: "changed".into(),
    };
    assert_ne!(run(&changed, &r)?.identity, p.plan().identity);
    Ok(())
}

#[test]
fn page_and_file_registrations_reject_collisions_and_path_overlap() -> Result<(), String> {
    let r = registry()?;
    for file in [false, true] {
        for field in [PageField::Id, PageField::Source, PageField::Route] {
            let mut s = set(&r)?;
            let first = s.pages[0].registration.clone();
            let second = if file {
                s.files.push(PageFile {
                    registration: PageRegistration {
                        id: "file".into(),
                        source: "data/file".into(),
                        route: "data/file".into(),
                    },
                    content: FileBytes(vec![]),
                });
                &mut s.files[0].registration
            } else {
                &mut s.pages[1].registration
            };
            match field {
                PageField::Id => second.id = first.id,
                PageField::Source => second.source = first.source,
                PageField::Route => second.route = first.route,
            }
            assert!(matches!(run(&s, &r), Err(e) if e.contains("Collision")));
        }
    }
    for route in [
        "docs/first/index.html/child",
        "docs",
        "/root",
        "../escape",
        "a/%2e/b",
        "a//b",
    ] {
        let mut s = set(&r)?;
        s.pages[1].registration.route = route.into();
        assert!(run(&s, &r).is_err(), "{route}");
    }
    assert!(
        run(
            &PageSet {
                pages: vec![],
                files: vec![]
            },
            &r
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn first_root_receiver_recomputes_plan_and_rejects_forged_or_stale_data() -> Result<(), String> {
    let r = registry()?;
    let mut s = set(&r)?;
    s.files.push(PageFile {
        registration: PageRegistration {
            id: "file".into(),
            source: "data/raw".into(),
            route: "files/raw".into(),
        },
        content: FileBytes(vec![0, 255, 13, 10]),
    });
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let plan = run(&s, &r)?;
    let packet = nepl3_wire::encode(
        &portable::pages::set_to_value(&s, &r, &mut c, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let p = portable::pages::plan_to_value(&plan, &s, &r, &mut c, &mut b()).map_err(err)?;
    let packet_p = nepl3_wire::encode(&p, &mut b()).map_err(err)?;
    let mut fresh_a = SourceAdmission::default();
    let mut fresh = FoundationCodec::new(&r, &empty, &mut fresh_a).map_err(err)?;
    let received = portable::pages::set_from_value(
        &nepl3_wire::decode(&packet, &mut b()).map_err(err)?,
        &r,
        &mut fresh,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(received, s);
    let received_p = nepl3_wire::decode(&packet_p, &mut b()).map_err(err)?;
    assert_eq!(
        portable::pages::plan_from_value(&received_p, &received, &r, &mut fresh, &mut b())
            .map_err(err)?,
        plan
    );
    let mut omitted = p;
    let NdfValue::Record(plan) = &mut omitted else {
        return Err("plan".into());
    };
    plan.fields[2] = NdfValue::List(vec![]);
    assert!(
        portable::pages::plan_from_value(&omitted, &received, &r, &mut fresh, &mut b()).is_err()
    );
    for route in [false, true] {
        let mut changed = received.clone();
        if route {
            changed.pages[1].registration.route = "moved.html".into();
        } else {
            changed.files[0].content.0[1] = 254;
        }
        assert!(
            portable::pages::plan_from_value(&received_p, &changed, &r, &mut fresh, &mut b())
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn page_resolution_obeys_sticky_resource_limits() -> Result<(), String> {
    let r = registry()?;
    let s = set(&r)?;
    let original = s.clone();
    let empty = SourceStore::default();
    for resource in [Resource::Work, Resource::AllocationUnits] {
        for cap in [0, 1, 100, 1000] {
            let mut limits = b().limits();
            match resource {
                Resource::Work => limits.work = cap,
                _ => limits.allocation_units = cap,
            }
            let mut budget = Budget::new(limits);
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
            for _ in 0..2 {
                assert!(matches!(
                    resolve(&s, &r, &mut c, &mut budget),
                    Err(PageError::Stopped(_))
                ));
            }
            assert_eq!(s, original);
        }
    }
    Ok(())
}
