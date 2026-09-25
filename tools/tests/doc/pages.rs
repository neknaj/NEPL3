use super::*;
use nepl3_doc_core::{check::Category, lower, pages::*, portable};

#[test]
fn real_two_page_sources_resolve_without_ambient_documents() -> Result<(), String> {
    let mut compiled = compiled()?;
    for descriptor in [
        nepl3_markup::schema::descriptor(&mut budget()),
        nepl3_doc_html::schema::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        compiled
            .doc
            .registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    compiled.doc.registry.finalize(&mut budget()).map_err(err)?;
    let inputs = [
        (
            "intro",
            "doc/intro.nepld",
            "docs/intro/index.html",
            r#"article ja "入門"
body cons paragraph cons sentence cons anchor start text "入口"
cons link page "guide" some "usage" text "使い方" nil nil nil"#,
        ),
        (
            "guide",
            "doc/reference/guide.nepld",
            "docs/reference/guide/index.html",
            r#"article ja "使い方"
body cons section usage "利用方法" body cons paragraph cons sentence
cons link relative "../intro.nepld" some "start" text "入門へ戻る" nil nil nil nil"#,
        ),
    ];
    let mut pages = Vec::new();
    for (id, source, route, input) in inputs {
        let document = nepl3_tools::doc::source::with_named_input(
            true,
            &compiled,
            input,
            id,
            "Article",
            |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut budget(),
                    &mut c,
                )
                .map_err(err)
            },
        )?;
        assert_eq!(document.sources[0].identity().source.0, id);
        pages.push(PageDocument {
            registration: PageRegistration {
                id: id.into(),
                source: source.into(),
                route: route.into(),
            },
            document,
        });
    }
    let set = PageSet {
        pages,
        files: vec![],
    };
    let r = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    let plan = resolve(&set, r, &mut c, &mut budget())
        .map_err(err)?
        .plan()
        .clone();
    assert_eq!(
        plan.links
            .iter()
            .map(|l| (l.page, l.target, l.fragment.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (
                0,
                nepl3_doc_core::pages::PageDestination::Page { index: 1 },
                Some("usage")
            ),
            (
                1,
                nepl3_doc_core::pages::PageDestination::Page { index: 0 },
                Some("start")
            )
        ]
    );
    assert!(plan.remaining.is_empty());
    let request = nepl3_doc_html::pages::PagesHtmlRequest {
        set: set.clone(),
        options: nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        },
    };
    let rendered =
        nepl3_doc_html::pages::render_pages(&request, r, &mut c, &mut budget()).map_err(err)?;
    let mut html = Vec::new();
    for f in &rendered.fragments {
        let m = &f.markup;
        let checked = nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, &mut budget())
            .map_err(err)?;
        html.push(nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?);
    }
    assert!(html[0].contains("href=\"../reference/guide/index.html#n-7573616765\""));
    assert!(html[1].contains("href=\"../../intro/index.html#n-7374617274\""));
    assert!(html[0].contains("id=\"n-7374617274\""));
    assert!(html[1].contains("id=\"n-7573616765\""));
    let rendered_value = nepl3_doc_html::portable::pages::rendered_to_value(
        &rendered,
        &request,
        r,
        &mut c,
        &mut budget(),
    )
    .map_err(err)?;
    let rendered_bytes = nepl3_wire::encode(&rendered_value, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(
        &portable::pages::set_to_value(&set, r, &mut c, &mut budget()).map_err(err)?,
        &mut budget(),
    )
    .map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
    let received = portable::pages::set_from_value(
        &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
        r,
        &mut receiver,
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(
        &plan,
        resolve(&received, r, &mut receiver, &mut budget())
            .map_err(err)?
            .plan()
    );
    let received_request = nepl3_doc_html::pages::PagesHtmlRequest {
        set: received,
        options: request.options.clone(),
    };
    let rendered_received = nepl3_wire::decode(&rendered_bytes, &mut budget()).map_err(err)?;
    assert_eq!(
        rendered,
        nepl3_doc_html::portable::pages::rendered_from_value(
            &rendered_received,
            &received_request,
            r,
            &mut receiver,
            &mut budget()
        )
        .map_err(err)?
    );
    let mut changed = received_request;
    changed.set.pages[0].registration.route = "moved/index.html".into();
    assert!(
        nepl3_doc_html::portable::pages::rendered_from_value(
            &rendered_received,
            &changed,
            r,
            &mut receiver,
            &mut budget()
        )
        .is_err()
    );
    let mut missing = set.clone();
    missing.pages.pop();
    assert!(matches!(
        resolve(&missing, r, &mut c, &mut budget()),
        Err(PageError::MissingPage { .. })
    ));
    Ok(())
}

#[test]
fn page_output_rejects_an_anchor_hidden_by_language_selection() -> Result<(), String> {
    let compiled = compiled()?;
    let mut pages = Vec::new();
    for (id, input) in [
        (
            "from",
            r#"article en sentence "From" body cons paragraph cons sentence sentence cons doc link page "to" some "hidden" text "go" nil nil nil"#,
        ),
        (
            "to",
            r#"article ja sentence "To" body cons paragraph cons parallel cons variant ja sentence sentence cons doc anchor hidden text "対象" nil cons variant en sentence "Translation" nil nil nil"#,
        ),
    ] {
        let document = nepl3_tools::doc::source::with_named_input(
            true,
            &compiled,
            input,
            id,
            "Article",
            |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut budget(),
                    &mut c,
                )
                .map_err(err)
            },
        )?;
        pages.push(PageDocument {
            registration: PageRegistration {
                id: id.into(),
                source: format!("{id}.nepld"),
                route: format!("{id}/index.html"),
            },
            document,
        });
    }
    let mut request = nepl3_doc_html::pages::PagesHtmlRequest {
        set: PageSet {
            pages,
            files: vec![],
        },
        options: nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        },
    };
    let render = |request: &nepl3_doc_html::pages::PagesHtmlRequest, b: &mut Budget| {
        nepl3_tools::doc::export::pages::render_request(&compiled, request, b)
    };
    let rows = render(&request, &mut budget())?;
    assert!(rows.pages[0].contains("href=\"../to/index.html#n-68696464656e\""));
    assert!(rows.pages[1].contains("id=\"n-68696464656e\""));
    request.options.parallel = nepl3_doc_html::ParallelMode::Single {
        language: "en".into(),
        fallbacks: vec![],
    };
    let error = render(&request, &mut budget())
        .err()
        .ok_or("hidden target accepted")?;
    assert!(
        error.starts_with("MissingAnchor { page: 0, element:"),
        "{error}"
    );
    assert!(error.ends_with(", target: 1 }"), "{error}");
    request.options.parallel = nepl3_doc_html::ParallelMode::Single {
        language: "ja".into(),
        fallbacks: vec![],
    };
    let selected = render(&request, &mut budget())?;
    assert!(selected.pages[1].contains("id=\"n-68696464656e\""));
    assert!(!selected.pages[1].contains("Translation"));
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        render(&request, &mut cancelled).err(),
        Some("Cancelled".into())
    );
    Ok(())
}
