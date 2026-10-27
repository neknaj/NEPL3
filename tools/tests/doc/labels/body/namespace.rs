use super::*;
use nepl3_doc_core::labels::namespace as ns;

fn named(
    c: &Compiled,
    name: &str,
    source: &str,
    category: Category,
) -> Result<DocumentSyntax, String> {
    let kind = match category {
        Category::Article => "Article",
        Category::Body => "Body",
        Category::Sentence => "Sentence",
        Category::Inline => "Inline",
        _ => return Err("test input category".into()),
    };
    nepl3_tools::doc::source::with_named_input(
        true,
        c,
        source,
        name,
        kind,
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            nepl3_suite::adapters::sentence::literal::lower(
                &checked,
                &c.doc.package.schema,
                category,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)
        },
    )
}

#[test]
fn body_namespace_requires_explicit_selection_and_keeps_each_owner() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        "# 日本語🙂\r\nbody cons section 本体 \"Body\" body cons paragraph cons sentence cons ref 外 text \"outside\" nil nil nil nil",
        |doc, r, b, a| {
            let other = named(
                &c,
                "other",
                r#"article en "Article" body cons paragraph cons sentence cons anchor 外 text "definition" cons ref 本体 text "back" nil nil nil"#,
                Category::Article,
            )?;
            let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
            assert!(matches!(
                ns::inspect(doc, r, b, a),
                Err(ns::Error::Root(Category::Body))
            ));
            assert!(matches!(
                labels::check_body(doc, r, b, a),
                Err(LabelError::Unresolved { .. })
            ));
            assert!(
                matches!(ns::resolve_body(&body, &[], b), Err(ns::Error::Unresolved { reference }) if reference.member == ns::MemberId(0))
            );
            let other = ns::inspect(&other, r, b, a).map_err(err)?;
            let fragments = [&other];
            let proof = ns::resolve_body(&body, &fragments, b).map_err(err)?;
            assert_eq!(
                proof
                    .definitions()
                    .iter()
                    .map(|s| (s.member.0, s.site.name))
                    .collect::<Vec<_>>(),
                [(0, "本体"), (1, "外")]
            );
            assert_eq!(
                proof
                    .references()
                    .iter()
                    .map(|s| (s.reference.member.0, s.target.0))
                    .collect::<Vec<_>>(),
                [(0, 1), (1, 0)]
            );
            assert!(core::ptr::eq(
                proof.document(ns::MemberId(0)).ok_or("body owner")?,
                doc
            ));
            assert!(core::ptr::eq(
                proof.document(ns::MemberId(1)).ok_or("fragment owner")?,
                other.document()
            ));
            assert!(proof.document(ns::MemberId(u64::MAX)).is_none());
            assert_eq!(proof.documents().count(), 2);
            let selected = proof.references()[0]
                .reference
                .site
                .selection
                .ok_or("selection")?;
            assert_eq!(
                doc.sources
                    .iter()
                    .find(|s| s.identity() == selected.snapshot_ref())
                    .ok_or("source")?
                    .slice(selected)
                    .map_err(err)?,
                "外"
            );
            Ok(())
        },
    )
}

#[test]
fn body_namespace_duplicate_and_repeated_fragment_occurrences_are_explicit() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        r#"body cons section root "Root" body nil nil"#,
        |doc, r, b, a| {
            let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
            let duplicate = named(
                &c,
                "duplicate",
                r#"anchor root text "again""#,
                Category::Inline,
            )?;
            let duplicate = ns::inspect(&duplicate, r, b, a).map_err(err)?;
            assert!(
                matches!(ns::resolve_body(&body,&[&duplicate],b),Err(ns::Error::Duplicate { definition, previous }) if definition.member==ns::MemberId(1) && previous.member==ns::MemberId(0))
            );
            let reference = named(&c, "reference", r#"ref root text "read""#, Category::Inline)?;
            let reference = ns::inspect(&reference, r, b, a).map_err(err)?;
            let fragments = [&reference, &reference];
            let checked = ns::resolve_body(&body, &fragments, b).map_err(err)?;
            assert_eq!(
                checked
                    .references()
                    .iter()
                    .map(|s| s.reference.member.0)
                    .collect::<Vec<_>>(),
                [1, 2]
            );
            assert!(checked.references().iter().all(|s| s.target.0 == 0));
            Ok(())
        },
    )
}

#[test]
fn body_namespace_interleaved_order_validates_every_selected_site() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        r#"body cons section first "First" body cons paragraph cons sentence cons ref outside text "other" nil nil nil nil"#,
        |doc, r, b, a| {
            let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
            let other = named(
                &c,
                "interleaved",
                r#"concat cons anchor outside text "there" cons ref first text "back" nil"#,
                Category::Inline,
            )?;
            let other = ns::inspect(&other, r, b, a).map_err(err)?;
            let fragments = [&other];
            let defs = [
                ns::DefinitionIndex {
                    member: ns::MemberId(1),
                    index: 0,
                },
                ns::DefinitionIndex {
                    member: ns::MemberId(0),
                    index: 0,
                },
            ];
            let refs = [
                ns::ReferenceIndex {
                    member: ns::MemberId(1),
                    index: 0,
                },
                ns::ReferenceIndex {
                    member: ns::MemberId(0),
                    index: 0,
                },
            ];
            let checked =
                ns::resolve_body_interleaved(&body, &fragments, &defs, &refs, b).map_err(err)?;
            assert_eq!(
                checked
                    .definitions()
                    .iter()
                    .map(|s| s.site.name)
                    .collect::<Vec<_>>(),
                ["outside", "first"]
            );
            assert_eq!(
                checked
                    .references()
                    .iter()
                    .map(|s| (s.reference.member.0, s.target.0))
                    .collect::<Vec<_>>(),
                [(1, 1), (0, 0)]
            );
            assert!(matches!(
                ns::resolve_body_interleaved(&body, &fragments, &defs, &refs[..1], b),
                Err(ns::Error::Order(ns::OrderError::Missing { .. }))
            ));
            let mut measured = budget();
            ns::resolve_body_interleaved(&body, &fragments, &defs, &refs, &mut measured)
                .map_err(err)?;
            for short in [false, true] {
                let mut limits = budget().limits();
                limits.work = measured.usage().work + 17 - u64::from(short);
                let mut limited = Budget::new(limits);
                limited.charge(Resource::Work, 17).map_err(err)?;
                let result =
                    ns::resolve_body_interleaved(&body, &fragments, &defs, &refs, &mut limited);
                if short {
                    assert!(matches!(
                        result,
                        Err(ns::Error::Stopped(StopReason::WorkLimit))
                    ));
                    assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
                } else {
                    result.map_err(err)?;
                    assert_eq!(limited.usage().work, measured.usage().work + 17);
                }
            }
            for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
                let mut limits = budget().limits();
                if reason == StopReason::WorkLimit {
                    limits.work = 0;
                } else {
                    limits.allocation_units = 0;
                }
                let mut limited = Budget::new(limits);
                assert!(
                    matches!(ns::resolve_body_interleaved(&body, &fragments, &defs, &refs, &mut limited), Err(ns::Error::Stopped(actual)) if actual == reason)
                );
                assert_eq!(limited.poll(), Err(reason));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                ns::resolve_body_interleaved(&body, &fragments, &defs, &refs, &mut cancelled),
                Err(ns::Error::Stopped(StopReason::Cancelled))
            ));
            let invalid = [ns::ReferenceIndex {
                member: ns::MemberId(99),
                index: 0,
            }];
            assert!(matches!(
                ns::resolve_body_interleaved(&body, &fragments, &defs, &invalid, b),
                Err(ns::Error::Order(ns::OrderError::Member { .. }))
            ));
            Ok(())
        },
    )
}

#[test]
fn body_namespace_inspection_rejects_wrong_roots_and_structure_before_names() -> Result<(), String>
{
    let c = compiled()?;
    with_body(&c, "body nil", |doc, r, b, a| {
        let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
        assert!(
            ns::resolve_body(&body, &[], b)
                .map_err(err)?
                .definitions()
                .is_empty()
        );
        for (category, source) in [
            (Category::Article, r#"article en "A" body nil"#),
            (Category::Sentence, "sentence nil"),
            (Category::Inline, r#"text "inline""#),
        ] {
            let other = named(&c, &format!("wrong-{category:?}"), source, category)?;
            assert!(
                matches!(ns::inspect_body(&other,r,b,a),Err(ns::Error::Root(actual)) if actual==category)
            );
        }
        let mut broken = doc.clone_with_budget(b).map_err(err)?;
        broken.value.root = DocRoot::Body(BodyRef(u64::MAX));
        assert!(matches!(
            ns::inspect_body(&broken, r, b, a),
            Err(ns::Error::Input(LabelError::Structure(_)))
        ));
        broken.value.root = DocRoot::Article(ArticleRef(u64::MAX));
        assert!(matches!(
            ns::inspect_body(&broken, r, b, a),
            Err(ns::Error::Input(LabelError::Structure(_)))
        ));
        let mut repeated = named(
            &c,
            "repeated-body",
            r#"body cons section one "One" body nil nil"#,
            Category::Body,
        )?;
        let DocRoot::Body(root) = repeated.value.root else {
            return Err("Body fixture".into());
        };
        let DocKind::Body { blocks } = &mut repeated.value.nodes[root.0 as usize].kind else {
            return Err("Body node".into());
        };
        blocks.push(*blocks.first().ok_or("section fixture")?);
        assert!(matches!(
            ns::inspect_body(&repeated, r, b, a),
            Err(ns::Error::Input(LabelError::DuplicateOccurrence { .. }))
        ));
        let duplicate = named(
            &c,
            "duplicate-body",
            r#"body cons section same "A" body nil cons section same "B" body nil nil"#,
            Category::Body,
        )?;
        assert!(matches!(
            ns::inspect_body(&duplicate, r, b, a),
            Err(ns::Error::Input(LabelError::Duplicate { .. }))
        ));
        Ok(())
    })
}

#[test]
fn body_namespace_preserves_cumulative_inspection_and_resolution_stops() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        r#"body cons section root "R" body cons paragraph cons sentence cons ref root text "self" nil nil nil nil"#,
        |doc, r, b, a| {
            let saved = doc.clone_with_budget(b).map_err(err)?;
            let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
            for phase in 0..2 {
                let mut measured = budget();
                if phase == 0 {
                    ns::inspect_body(doc, r, &mut measured, &mut SourceAdmission::default())
                        .map_err(err)?;
                } else {
                    ns::resolve_body(&body, &[], &mut measured).map_err(err)?;
                }
                for short in [false, true] {
                    let mut limits = budget().limits();
                    limits.work = measured.usage().work + 17 - u64::from(short);
                    let mut operation = Budget::new(limits);
                    operation.charge(Resource::Work, 17).map_err(err)?;
                    let result = if phase == 0 {
                        ns::inspect_body(doc, r, &mut operation, &mut SourceAdmission::default())
                            .map(|_| ())
                    } else {
                        ns::resolve_body(&body, &[], &mut operation).map(|_| ())
                    };
                    if short {
                        assert!(matches!(
                            result,
                            Err(ns::Error::Stopped(StopReason::WorkLimit))
                        ));
                        assert_eq!(operation.poll(), Err(StopReason::WorkLimit));
                    } else {
                        result.map_err(err)?;
                        assert_eq!(operation.usage().work, measured.usage().work + 17);
                    }
                }
            }
            for resource in 0..5 {
                let mut limits = budget().limits();
                let reason = match resource {
                    0 => {
                        limits.work = 0;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.allocation_units = 0;
                        StopReason::AllocationLimit
                    }
                    2 => {
                        limits.source_bytes = 0;
                        StopReason::SourceLimit
                    }
                    3 => {
                        limits.nodes = 0;
                        StopReason::NodeLimit
                    }
                    _ => {
                        limits.depth = 0;
                        StopReason::DepthLimit
                    }
                };
                let mut operation = Budget::new(limits);
                assert!(
                    matches!(ns::inspect_body(doc,r,&mut operation,&mut SourceAdmission::default()),Err(ns::Error::Stopped(actual)) if actual==reason)
                );
                assert_eq!(operation.poll(), Err(reason));
            }
            for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
                let mut limits = budget().limits();
                if reason == StopReason::WorkLimit {
                    limits.work = 0;
                } else {
                    limits.allocation_units = 0;
                }
                let mut operation = Budget::new(limits);
                assert!(
                    matches!(ns::resolve_body(&body,&[],&mut operation),Err(ns::Error::Stopped(actual)) if actual==reason)
                );
                assert_eq!(operation.poll(), Err(reason));
                assert_eq!(operation.usage().allocation_units, 0);
            }
            let mut depth_probe = budget();
            ns::inspect_body(doc, r, &mut depth_probe, &mut SourceAdmission::default())
                .map_err(err)?;
            for short in [false, true] {
                let mut limits = budget().limits();
                limits.depth = depth_probe.usage().depth + 7 - u64::from(short);
                let mut limited = Budget::new(limits);
                let result = limited.with_depth_at_least(7, |b| {
                    ns::inspect_body(doc, r, b, &mut SourceAdmission::default())
                });
                if short {
                    assert!(matches!(
                        result,
                        Err(ns::Error::Stopped(StopReason::DepthLimit))
                    ));
                } else {
                    result.map_err(err)?;
                }
                assert_eq!(limited.current_depth(), 0);
            }
            let mut operation = budget();
            operation
                .with_depth_at_least(7, |b| {
                    ns::inspect_body(doc, r, b, &mut SourceAdmission::default())
                })
                .map_err(err)?;
            assert_eq!(operation.current_depth(), 0);
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                ns::resolve_body(&body, &[], &mut cancelled),
                Err(ns::Error::Stopped(StopReason::Cancelled))
            ));
            assert!(matches!(
                ns::inspect_body(doc, r, &mut cancelled, a),
                Err(ns::Error::Stopped(StopReason::Cancelled))
            ));
            assert_doc_retention(doc, &saved)?;
            Ok(())
        },
    )
}

#[test]
fn body_namespace_diagnostics_preserve_and_validate_source_owners() -> Result<(), String> {
    let c = compiled()?;
    with_body(
        &c,
        "# 日本語🙂\r\nbody cons section 重複 \"Root\" body cons paragraph cons sentence cons ref 不明 text \"missing\" nil nil nil nil",
        |doc, r, b, a| {
            let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
            let unresolved = ns::resolve_body(&body, &[], b)
                .err()
                .ok_or("unresolved expected")?;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
            let diagnostic = unresolved
                .diagnostic_body(&body, &[], r, &mut codec, b)
                .map_err(err)?;
            let selected = diagnostic.primary.as_ref().ok_or("primary")?;
            assert_eq!(
                doc.sources
                    .iter()
                    .find(|s| s.identity() == selected.snapshot_ref())
                    .ok_or("source")?
                    .slice(selected)
                    .map_err(err)?,
                "不明"
            );
            let other = named(
                &c,
                "diagnostic-other",
                r#"anchor 重複 text "other""#,
                Category::Inline,
            )?;
            let other = ns::inspect(&other, r, b, codec.source_admission()).map_err(err)?;
            let fragments = [&other];
            let duplicate = ns::resolve_body(&body, &fragments, b)
                .err()
                .ok_or("duplicate expected")?;
            let diagnostic = duplicate
                .diagnostic_body(&body, &fragments, r, &mut codec, b)
                .map_err(err)?;
            assert_eq!(diagnostic.related.len(), 1);
            assert_eq!(
                diagnostic.related[0].span.as_ref(),
                body.definitions()[0].selection
            );
            assert_eq!(
                diagnostic
                    .primary
                    .as_ref()
                    .ok_or("duplicate primary")?
                    .snapshot_ref(),
                other.definitions()[0]
                    .selection
                    .ok_or("other selection")?
                    .snapshot_ref()
            );
            let forged = ns::Error::Unresolved {
                reference: ns::NamespaceSite {
                    member: ns::MemberId(1),
                    site: body.references()[0],
                },
            };
            assert!(matches!(
                forged.diagnostic_body(&body, &fragments, r, &mut codec, b),
                Err(labels::LabelDiagnosticError::NotSemantic)
            ));
            let mut foreign_site = body.references()[0];
            foreign_site.selection = other.definitions()[0].selection;
            let forged_span = ns::Error::Unresolved {
                reference: ns::NamespaceSite {
                    member: ns::MemberId(0),
                    site: foreign_site,
                },
            };
            assert!(matches!(
                forged_span.diagnostic_body(&body, &fragments, r, &mut codec, b),
                Err(labels::LabelDiagnosticError::NotSemantic)
            ));
            let mut limits = budget().limits();
            limits.diagnostics = 0;
            let mut limited = Budget::new(limits);
            assert!(matches!(
                unresolved.diagnostic_body(&body, &[], r, &mut codec, &mut limited),
                Err(labels::LabelDiagnosticError::Stopped(
                    StopReason::DiagnosticLimit
                ))
            ));
            assert_eq!(limited.poll(), Err(StopReason::DiagnosticLimit));
            for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
                let mut limits = budget().limits();
                if reason == StopReason::WorkLimit {
                    limits.work = 0;
                } else {
                    limits.allocation_units = 0;
                }
                let mut limited = Budget::new(limits);
                assert!(
                    matches!(unresolved.diagnostic_body(&body, &[], r, &mut codec, &mut limited), Err(labels::LabelDiagnosticError::Stopped(actual)) if actual == reason)
                );
                assert_eq!(limited.poll(), Err(reason));
            }
            for nonsemantic in [
                ns::Error::Input(LabelError::ExpectedBody),
                ns::Error::Root(Category::Body),
                ns::Error::Order(ns::OrderError::Member {
                    kind: ns::SiteKind::Reference,
                    member: ns::MemberId(99),
                }),
            ] {
                let mut limits = budget().limits();
                limits.work = 0;
                limits.allocation_units = 0;
                let mut limited = Budget::new(limits);
                assert!(matches!(
                    nonsemantic.diagnostic_body(&body, &[], r, &mut codec, &mut limited),
                    Err(labels::LabelDiagnosticError::NotSemantic)
                ));
                assert_eq!(limited.usage().work, 0);
                assert_eq!(limited.usage().allocation_units, 0);
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                ns::Error::Stopped(StopReason::SourceLimit).diagnostic_body(
                    &body,
                    &[],
                    r,
                    &mut codec,
                    &mut cancelled
                ),
                Err(labels::LabelDiagnosticError::Stopped(StopReason::Cancelled))
            ));
            let mut stopped = budget();
            assert!(matches!(
                ns::Error::Stopped(StopReason::SourceLimit).diagnostic_body(
                    &body,
                    &[],
                    r,
                    &mut codec,
                    &mut stopped
                ),
                Err(labels::LabelDiagnosticError::Stopped(
                    StopReason::SourceLimit
                ))
            ));
            Ok(())
        },
    )
}

#[test]
fn body_namespace_does_not_import_foreign_definitions_or_conflicting_sources() -> Result<(), String>
{
    let c = compiled()?;
    with_body(
        &c,
        r#"body cons paragraph cons code Doc article en "Guest" body cons section hidden "Heading" body nil nil cons sentence cons ref hidden text "outside" nil nil nil"#,
        |doc, r, b, a| {
            let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
            assert!(body.definitions().is_empty());
            assert!(matches!(
                ns::resolve_body(&body, &[], b),
                Err(ns::Error::Unresolved { .. })
            ));
            let unselected = named(
                &c,
                "unselected",
                r#"anchor hidden text "separate""#,
                Category::Inline,
            )?;
            let selected = ns::inspect(&unselected, r, b, a).map_err(err)?;
            assert!(matches!(
                ns::resolve_body(&body, &[], b),
                Err(ns::Error::Unresolved { .. })
            ));
            ns::resolve_body(&body, &[&selected], b).map_err(err)?;
            let conflict = named(&c, "doc-input", "body nil", Category::Body)?;
            ns::inspect_body(&conflict, r, b, &mut SourceAdmission::default()).map_err(err)?;
            assert!(matches!(
                ns::inspect_body(&conflict, r, b, a),
                Err(ns::Error::Input(LabelError::Structure(_)))
            ));
            Ok(())
        },
    )
}

#[test]
fn body_namespace_does_not_admit_raw_body_to_legacy_preparation_or_pages() -> Result<(), String> {
    let c = compiled()?;
    with_body(&c, "body nil", |doc, r, b, a| {
        let body = ns::inspect_body(doc, r, b, a).map_err(err)?;
        ns::resolve_body(&body, &[], b).map_err(err)?;
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(r, &empty, a).map_err(err)?;
        assert!(matches!(
            nepl3_doc_core::prepare::inspect(doc, r, &mut codec, b),
            Err(nepl3_doc_core::prepare::PreparationError::Label(
                LabelError::ExpectedArticle
            ))
        ));
        let set = nepl3_doc_core::pages::PageSet {
            pages: vec![nepl3_doc_core::pages::PageDocument {
                registration: nepl3_doc_core::pages::PageRegistration {
                    id: "body".into(),
                    source: "body.nepld".into(),
                    route: "body.html".into(),
                },
                document: doc.clone_with_budget(b).map_err(err)?,
            }],
            files: vec![],
        };
        assert!(matches!(
            nepl3_doc_core::pages::resolve(&set, r, &mut codec, b),
            Err(nepl3_doc_core::pages::PageError::Input {
                page: 0,
                error: nepl3_doc_core::prepare::PreparationError::Label(
                    LabelError::ExpectedArticle
                ),
            })
        ));
        Ok(())
    })
}
