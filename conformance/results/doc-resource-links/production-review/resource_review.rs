use nepl3_core::{
    budget::{Budget, Limits, Resource},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_doc_core::{model::*, pages::*, portable};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
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
fn doc(target: LinkTarget) -> DocumentSyntax {
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text {
            text: "文書".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(4)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(5)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(6), InlineRef(7)],
        },
        DocKind::Anchor {
            id: "導入".into(),
            label: InlineRef(2),
        },
        DocKind::Link {
            target,
            label: InlineRef(2),
        },
    ];
    DocumentSyntax {
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
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}
fn set() -> PageSet {
    PageSet {
        files: vec![],
        pages: vec![
            PageDocument {
                registration: PageRegistration {
                    id: "first".into(),
                    source: "doc/first.nepld".into(),
                    route: "docs/first/index.html".into(),
                },
                document: doc(LinkTarget::Page {
                    page: "second".into(),
                    fragment: Some("導入".into()),
                }),
            },
            PageDocument {
                registration: PageRegistration {
                    id: "second".into(),
                    source: "doc/next/second.nepld".into(),
                    route: "docs/second/index.html".into(),
                },
                document: doc(LinkTarget::Relative {
                    path: ".././first.nepld".into(),
                    fragment: Some("導入".into()),
                }),
            },
        ],
    }
}
fn run(set: &PageSet) -> Result<PageLinkPlan, String> {
    let r = registry()?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    Ok(resolve(set, &r, &mut c, &mut b())
        .map_err(err)?
        .plan()
        .clone())
}
#[test]
fn registered_file_bytes_are_resolved_and_bound_to_the_portable_plan() -> Result<(), String> {
    let r = registry()?;
    let mut input = set();
    input.pages[0].document.value.nodes[7].kind = DocKind::Link {
        target: LinkTarget::Relative {
            path: "../design/contracts.json".into(),
            fragment: None,
        },
        label: InlineRef(2),
    };
    input.files.push(PageFile {
        registration: PageRegistration {
            id: "contracts".into(),
            source: "design/contracts.json".into(),
            route: "data/contracts.json".into(),
        },
        content: FileBytes(vec![0, 255, 13, 10]),
    });
    let plan = run(&input)?;
    assert_eq!(plan.links[0].target, PageDestination::File { index: 0 });
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let encoded = portable::pages::set_to_value(&input, &r, &mut codec, &mut b()).map_err(err)?;
    let bytes = nepl3_wire::encode(&encoded, &mut b()).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let decoded =
        portable::pages::set_from_value(&received, &r, &mut codec, &mut b()).map_err(err)?;
    assert_eq!(decoded, input);
    assert_eq!(run(&decoded)?, plan);
    let encoded_plan =
        portable::pages::plan_to_value(&plan, &input, &r, &mut codec, &mut b()).map_err(err)?;
    input.files[0].content.0[1] = 254;
    assert_ne!(run(&input)?.identity, plan.identity);
    assert!(
        portable::pages::plan_from_value(&encoded_plan, &input, &r, &mut codec, &mut b()).is_err()
    );
    Ok(())
}
#[test]
fn file_registration_cannot_shadow_pages_or_claim_doc_anchors() -> Result<(), String> {
    let mut input = set();
    let file = PageFile {
        registration: PageRegistration {
            id: "resource".into(),
            source: "data/item".into(),
            route: "download/item".into(),
        },
        content: FileBytes(vec![]),
    };
    input.files.push(file.clone());
    for field in [PageField::Id, PageField::Source, PageField::Route] {
        input.files[0] = file.clone();
        match field {
            PageField::Id => {
                input.files[0].registration.id = input.pages[0].registration.id.clone()
            }
            PageField::Source => input.files[0].registration.source = "doc".into(),
            PageField::Route => input.files[0].registration.route = "docs/first".into(),
        }
        assert!(run(&input).is_err());
    }
    input.files[0] = file;
    for target in [
        LinkTarget::Page {
            page: "resource".into(),
            fragment: None,
        },
        LinkTarget::Relative {
            path: "../data/item".into(),
            fragment: Some("anchor".into()),
        },
        LinkTarget::Relative {
            path: "../data/unknown".into(),
            fragment: None,
        },
    ] {
        input.pages[0].document.value.nodes[7].kind = DocKind::Link {
            target,
            label: InlineRef(2),
        };
        assert!(run(&input).is_err());
    }
    Ok(())
}
#[test]
fn page_boundary_validation_still_precedes_earlier_page_label_failures() -> Result<(), String> {
    let r = registry()?;
    let mut input = set();
    input.pages[0].document.value.nodes[5].kind = DocKind::Sentence {
        inlines: vec![InlineRef(6), InlineRef(6), InlineRef(7)],
    };
    let original = input.pages[1].document.value.nodes[0].kind.clone();
    input.pages[1].document.value.nodes[0].kind = DocKind::Article {
        language: "ja".into(),
        title: SentenceRef(1),
        body: BodyRef(u64::MAX),
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    assert!(matches!(
        resolve(&input, &r, &mut codec, &mut b()),
        Err(PageError::Boundary(portable::PortableError::Structure(_)))
    ));
    input.pages[1].document.value.nodes[0].kind = original;
    assert!(matches!(
        resolve(&input, &r, &mut codec, &mut b()),
        Err(PageError::Input {
            page: 0,
            error: nepl3_doc_core::prepare::PreparationError::Label(_)
        })
    ));
    Ok(())
}
#[test]
fn pages_resolve_forward_and_relative_links_with_explicit_identity() -> Result<(), String> {
    let r = registry()?;
    let set = set();
    let original = set.clone();
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let p = resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        p.plan().links,
        vec![
            PageLink {
                page: 0,
                node: 7,
                target: PageDestination::Page { index: 1 },
                fragment: Some("導入".into())
            },
            PageLink {
                page: 1,
                node: 7,
                target: PageDestination::Page { index: 0 },
                fragment: Some("導入".into())
            }
        ]
    );
    assert!(p.plan().remaining.is_empty());
    let value = portable::pages::set_to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    let mut bytes = b"NEPL3.Doc.Pages.v1\0".to_vec();
    bytes.extend(nepl3_wire::encode(&value, &mut b()).map_err(err)?);
    assert_eq!(p.plan().identity, Digest::of(&bytes));
    for (index, page) in set.pages.iter().enumerate() {
        // Independently assemble the specified domain + canonical NDF bytes.
        // A digest of digests or a PageSet child at the wrong index must differ.
        let document = portable::to_value(&page.document, &r, &mut c, &mut b()).map_err(err)?;
        let mut expected = nepl3_doc_core::prepare::DOCUMENT_DOMAIN.to_vec();
        expected.extend(nepl3_wire::encode(&document, &mut b()).map_err(err)?);
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
    assert_ne!(run(&changed)?.identity, p.plan().identity);
    changed = set.clone();
    if let DocKind::Text { text } = &mut changed.pages[1].document.value.nodes[2].kind {
        *text = "changed".into();
    }
    assert_ne!(run(&changed)?.identity, p.plan().identity);
    Ok(())
}
#[test]
fn registry_collisions_and_unknown_destinations_are_rejected() -> Result<(), String> {
    for field in [PageField::Id, PageField::Source, PageField::Route] {
        let mut s = set();
        let first = s.pages[0].registration.clone();
        let second = &mut s.pages[1].registration;
        match field {
            PageField::Id => second.id = first.id,
            PageField::Source => second.source = first.source,
            PageField::Route => second.route = first.route,
        }
        assert!(matches!(run(&s), Err(e) if e.contains("Collision")));
    }
    for route in [
        "docs/first/index.html/child",
        "docs",
        "/root",
        "../escape",
        "a/%2e/b",
        "a//b",
    ] {
        let mut s = set();
        s.pages[1].registration.route = route.into();
        assert!(run(&s).is_err(), "{route}");
    }
    for target in [
        LinkTarget::Page {
            page: "absent".into(),
            fragment: None,
        },
        LinkTarget::Page {
            page: "second".into(),
            fragment: Some("missing".into()),
        },
        LinkTarget::Relative {
            path: "../../escape".into(),
            fragment: None,
        },
        LinkTarget::Relative {
            path: "next/%73econd.nepld".into(),
            fragment: None,
        },
    ] {
        let mut s = set();
        if let DocKind::Link { target: t, .. } = &mut s.pages[0].document.value.nodes[7].kind {
            *t = target;
        }
        assert!(run(&s).is_err());
    }
    assert!(
        run(&PageSet {
            pages: vec![],
            files: vec![]
        })
        .is_err()
    );
    Ok(())
}
#[test]
fn external_links_remain_requirements_not_successful_page_links() -> Result<(), String> {
    let mut s = set();
    if let DocKind::Link { target, .. } = &mut s.pages[0].document.value.nodes[7].kind {
        *target = LinkTarget::External {
            uri: "https://example.org/".into(),
        };
    }
    let p = run(&s)?;
    assert_eq!(p.links.len(), 1);
    assert_eq!(p.remaining.len(), 1);
    assert_eq!(p.remaining[0].page, 0);
    Ok(())
}
#[test]
fn first_cbor_receiver_recomputes_plan_and_rejects_forged_or_stale_links() -> Result<(), String> {
    let r = registry()?;
    let s = set();
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let plan = resolve(&s, &r, &mut c, &mut b())
        .map_err(err)?
        .plan()
        .clone();
    let packet = nepl3_wire::encode(
        &portable::pages::set_to_value(&s, &r, &mut c, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let mut p = portable::pages::plan_to_value(&plan, &s, &r, &mut c, &mut b()).map_err(err)?;
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
    let received_p = nepl3_wire::decode(&packet_p, &mut b()).map_err(err)?;
    assert_eq!(
        portable::pages::plan_from_value(&received_p, &received, &r, &mut fresh, &mut b())
            .map_err(err)?,
        plan
    );
    let NdfValue::Record(ref mut bad) = p else {
        return Err("expected plan record".into());
    };
    bad.fields[1] = NdfValue::List(vec![]);
    assert!(portable::pages::plan_from_value(&p, &received, &r, &mut fresh, &mut b()).is_err());
    let mut changed = received;
    changed.pages[1].registration.route = "moved.html".into();
    assert!(
        portable::pages::plan_from_value(&received_p, &changed, &r, &mut fresh, &mut b()).is_err()
    );
    Ok(())
}
#[test]
fn page_resolution_obeys_sticky_resource_limits() -> Result<(), String> {
    let r = registry()?;
    let s = set();
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
            assert!(matches!(
                resolve(&s, &r, &mut c, &mut budget),
                Err(PageError::Stopped(_))
            ));
            assert!(matches!(
                resolve(&s, &r, &mut c, &mut budget),
                Err(PageError::Stopped(_))
            ));
            assert_eq!(s, original);
        }
    }
    Ok(())
}

#[test]
fn independent_files_codec_order_and_forgery() -> Result<(),String> {
 let r=registry()?;let mut input=set();
 input.pages[0].document.value.nodes[7].kind=DocKind::Link{target:LinkTarget::Relative{path:"../data/blob".into(),fragment:None},label:InlineRef(2)};
 for (id,path,bytes) in [("f0","data/other",vec![]),("f1","data/blob",vec![0,255,13,10])] {
 input.files.push(PageFile{registration:PageRegistration{id:id.into(),source:path.into(),route:format!("download/{id}")},content:FileBytes(bytes)});
 }
 let plan=run(&input)?;assert_eq!(plan.links[0].target,PageDestination::File{index:1});
 let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
 let encoded=portable::pages::set_to_value(&input,&r,&mut c,&mut b()).map_err(err)?;
 let mut malformed=encoded.clone();
 if let NdfValue::Record(s)=&mut malformed {if let NdfValue::List(fs)=&mut s.fields[1] {if let NdfValue::Record(f)=&mut fs[1] {f.fields[1]=NdfValue::Text("not bytes".into());}}}
 assert!(portable::pages::set_from_value(&malformed,&r,&mut c,&mut b()).is_err());
 let mut oldshape=encoded.clone();if let NdfValue::Record(s)=&mut oldshape{s.fields.pop();}
 assert!(portable::pages::set_from_value(&oldshape,&r,&mut c,&mut b()).is_err());
 let mut forged=plan.clone();forged.links[0].target=PageDestination::File{index:u64::MAX};assert!(portable::pages::plan_to_value(&forged,&input,&r,&mut c,&mut b()).is_err());
 let mut reordered=input.clone();reordered.files.swap(0,1);assert_ne!(run(&reordered)?.identity,plan.identity);assert_eq!(run(&reordered)?.links[0].target,PageDestination::File{index:0});
 let mut moved=input.clone();moved.files[1].registration.route="different/file".into();assert_ne!(run(&moved)?.identity,plan.identity);
 let mut cap=b().limits();cap.allocation_units=1;let mut stopped=Budget::new(cap);assert!(portable::pages::set_to_value(&input,&r,&mut c,&mut stopped).is_err());assert!(stopped.poll().is_err());
 let mut cancelled=b();cancelled.cancel();assert!(resolve(&input,&r,&mut c,&mut cancelled).is_err());
 Ok(())
}
