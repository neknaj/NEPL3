use super::*;
use nepl3_doc_core::{check::Category, lower, pages::*, portable};

fn composed(
    compiled: &Compiled,
    request: &nepl3_doc_html::pages::PagesHtmlRequest,
) -> Result<
    (
        nepl3_core::source::Digest,
        Vec<nepl3_doc_html::RenderedWithForeign>,
    ),
    String,
> {
    use nepl3_doc_core::{labels::namespace as labels, pages::namespace as scopes};
    use nepl3_doc_html::pages::namespace as html;
    use nepl3_tools::doc::export::pages::{composition, discovery};
    let registry = &compiled.doc.registry;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut b = budget();
    let forms = [nepl3_sentence_core::lower::ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    let mut found = Vec::new();
    for page in &request.set.pages {
        found.push(
            discovery::collect(
                &page.document,
                registry
                    .selected("nepl3.syntax.sentence", 1)
                    .ok_or("Sentence")?,
                &compiled.doc.package.schema,
                &forms,
                registry,
                &mut codec,
                &mut b,
            )
            .map_err(err)?,
        );
    }
    let mut render_admission = SourceAdmission::default();
    let mut plans = Vec::new();
    for page in &found {
        plans.push(
            discovery::namespace::inspect(page, registry, &mut b, &mut render_admission)
                .map_err(err)?,
        );
    }
    let mut members = Vec::new();
    for plan in &plans {
        members.push(plan.member_refs(&mut b).map_err(err)?);
    }
    let mut namespaces = Vec::new();
    for members in &members {
        namespaces.push(labels::resolve(members, &mut b).map_err(err)?);
    }
    let refs: Vec<_> = namespaces.iter().collect();
    let checked =
        scopes::resolve(&request.set, &refs, registry, &mut codec, &mut b).map_err(err)?;
    // Independent expectations: the two authored links retain page ownership,
    // direction and fragment after recursive guest discovery and wire receipt.
    assert_eq!(
        checked
            .members()
            .iter()
            .flat_map(|m| m.links())
            .map(|l| (l.page, l.target, l.fragment.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (0, PageDestination::Page { index: 1 }, Some("usage")),
            (1, PageDestination::Page { index: 0 }, Some("start")),
        ]
    );
    let prepared = html::prepare(&checked, &request.options, &mut b).map_err(err)?;
    let mut outputs = Vec::new();
    for (page, plan) in plans.iter().enumerate() {
        outputs.push(
            composition::render(
                plan.selection(),
                &prepared,
                page as u64,
                registry,
                &mut |_, _, _, _, _| Err(nepl3_doc_html::RenderError::InternalShape),
                &mut |_, _, _, _| Err(nepl3_doc_html::RenderError::InternalShape),
                &mut b,
                &mut render_admission,
            )
            .map_err(err)?,
        );
    }
    let roots: Vec<_> = outputs
        .iter()
        .map(|o| {
            o.members()
                .first()
                .ok_or("root")
                .map(|m| m.document().output())
        })
        .collect::<Result<_, _>>()?;
    let markup: Vec<_> = roots.iter().map(|o| &o.fragment.markup).collect();
    let final_output = html::output::check(&prepared, &markup, &mut b).map_err(err)?;
    Ok((
        final_output.namespace_identity(),
        roots.into_iter().cloned().collect(),
    ))
}

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
            r#"article ja sentence "入門"
body cons paragraph cons sentence sentence cons doc anchor start text "入口"
cons doc link page "guide" some "usage" text "使い方" nil nil nil"#,
        ),
        (
            "guide",
            "doc/reference/guide.nepld",
            "docs/reference/guide/index.html",
            r#"article ja sentence "使い方"
body cons section usage sentence "利用方法" body cons paragraph cons sentence sentence
cons doc link relative "../intro.nepld" some "start" text "入門へ戻る" nil nil nil nil"#,
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
    let request = nepl3_doc_html::pages::PagesHtmlRequest {
        set: set.clone(),
        options: nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        },
    };
    let (identity, rendered) = composed(&compiled, &request)?;
    let mut html = Vec::new();
    for f in &rendered {
        let m = &f.fragment.markup;
        let checked = nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, &mut budget())
            .map_err(err)?;
        html.push(nepl3_markup::html::serialize(&checked, &mut budget()).map_err(err)?);
    }
    assert!(html[0].contains("href=\"../reference/guide/index.html#n-7573616765\""));
    assert!(html[1].contains("href=\"../../intro/index.html#n-7374617274\""));
    assert!(html[0].contains("id=\"n-7374617274\""));
    assert!(html[1].contains("id=\"n-7573616765\""));
    let exported =
        nepl3_tools::doc::export::pages::render_request(&compiled, &request, &mut budget())?;
    assert_eq!(exported.identity, identity);
    assert_eq!(exported.pages.len(), html.len());
    for (page, fragment) in exported.pages.iter().zip(&html) {
        assert!(page.contains(fragment));
    }
    let mut rendered_bytes = Vec::new();
    for output in &rendered {
        let value = nepl3_doc_html::portable::foreign::to_value(output, r, &mut c, &mut budget())
            .map_err(err)?;
        rendered_bytes.push(nepl3_wire::encode(&value, &mut budget()).map_err(err)?);
    }
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
    assert_eq!(received.pages.len(), set.pages.len());
    for (received, original) in received.pages.iter().zip(&set.pages) {
        assert_eq!(received.registration, original.registration);
        super::retention::assert_doc_retention(&original.document, &received.document)?;
    }
    let received_request = nepl3_doc_html::pages::PagesHtmlRequest {
        set: received,
        options: request.options.clone(),
    };
    let (received_identity, expected) = composed(&compiled, &received_request)?;
    assert_eq!(identity, received_identity);
    assert_eq!(rendered.len(), expected.len());
    for ((bytes, expected), native) in rendered_bytes.iter().zip(&expected).zip(&rendered) {
        let value = nepl3_wire::decode(bytes, &mut budget()).map_err(err)?;
        assert_eq!(
            &nepl3_doc_html::portable::foreign::from_value(
                &value,
                expected,
                r,
                &mut receiver,
                &mut budget()
            )
            .map_err(err)?,
            native
        );
    }
    let mut changed = received_request;
    changed.set.pages[0].registration.route = "moved/index.html".into();
    let (changed_identity, changed_outputs) = composed(&compiled, &changed)?;
    assert_ne!(identity, changed_identity);
    for (bytes, changed_output) in rendered_bytes.iter().zip(&changed_outputs) {
        let value = nepl3_wire::decode(bytes, &mut budget()).map_err(err)?;
        assert!(matches!(
            nepl3_doc_html::portable::foreign::from_value(
                &value,
                changed_output,
                r,
                &mut receiver,
                &mut budget()
            ),
            Err(nepl3_doc_html::portable::PortableError::Mismatch)
        ));
    }
    let mut missing = request;
    missing.set.pages.pop();
    let error = composed(&compiled, &missing)
        .err()
        .ok_or("missing page accepted")?;
    assert!(error.contains("MissingPage"), "{error}");
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
