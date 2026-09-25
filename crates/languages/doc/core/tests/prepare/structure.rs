use super::*;
use nepl3_core::value_codec::CanonicalDigestInput;

fn inspect<'a>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    store: &SourceStore,
    budget: &mut Budget,
) -> Result<prepare::DocPreparationPlan, PreparationError<'a, nepl3_wire::WireError>> {
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, store, &mut admission)
        .map_err(|error| PreparationError::Boundary(portable::PortableError::Foundation(error)))?;
    match document.value.root {
        DocRoot::Sentence(_) => prepare::inspect_sentence(document, registry, &mut codec, budget),
        DocRoot::Inline(_) => prepare::inspect_inline(document, registry, &mut codec, budget),
        _ => prepare::inspect(document, registry, &mut codec, budget),
    }
}

#[test]
fn standalone_preparation_reuses_structure_and_preserves_complete_plans() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    for root in [
        DocRoot::Article(ArticleRef(0)),
        DocRoot::Sentence(SentenceRef(0)),
        DocRoot::Inline(InlineRef(0)),
    ] {
        let mut d = document(&r)?;
        d.value.root = root;
        match root {
            DocRoot::Article(_) => {}
            DocRoot::Sentence(_) => {
                d.value.nodes.truncate(1);
                d.value.nodes[0].kind = DocKind::Sentence {
                    syntax: EmbedRef(0),
                };
            }
            DocRoot::Inline(_) => {
                d.value.nodes.truncate(1);
                d.value.nodes[0].kind = DocKind::Link {
                    target: LinkTarget::Page {
                        page: "guide".into(),
                        fragment: None,
                    },
                    label: EmbedRef(0),
                };
                d.value.embeds[0].kind = EmbedKind::SentenceInline;
            }
            _ => return Err("fixture root".into()),
        }
        let DocContent::Syntax { closure } = &mut d.value.embeds[0].content else {
            return Err("fixture closure".into());
        };
        if matches!(root, DocRoot::Inline(_)) {
            closure.syntax.category = "Inline".into();
        }
        closure.provenance = nepl3_core::syntax::OwnerProvenance::from_parts(
            (0..64)
                .map(|index| Origin::Synthetic {
                    reason: format!("origin {index}: {}", "provenance".repeat(64)),
                    anchor: None,
                })
                .collect(),
            vec![],
            vec![],
        );
        let mut measured = b();
        let plan = inspect(&d, &r, &store, &mut measured).map_err(err)?;

        // The old public path checks labels, independently encodes the same
        // document, then batches identical document/guest digest requests.
        // Requirement construction is omitted, so this baseline is a lower
        // bound. The large provenance makes repeated structure work visible.
        let mut baseline = b();
        let mut admission = SourceAdmission::default();
        match root {
            DocRoot::Article(_) => {
                nepl3_doc_core::labels::check(&d, &r, &mut baseline, &mut admission)
                    .map_err(err)?;
            }
            DocRoot::Sentence(_) => {
                nepl3_doc_core::labels::check_sentence(&d, &r, &mut baseline, &mut admission)
                    .map_err(err)?;
            }
            DocRoot::Inline(_) => {
                nepl3_doc_core::labels::check_inline(&d, &r, &mut baseline, &mut admission)
                    .map_err(err)?;
            }
            _ => return Err("fixture root".into()),
        }
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let encoded = portable::to_value(&d, &r, &mut codec, &mut baseline).map_err(err)?;
        let NdfValue::Record(document) = &encoded else {
            return Err("DocumentSyntax record".into());
        };
        let NdfValue::Record(value) = &document.fields[0] else {
            return Err("DocValue record".into());
        };
        let NdfValue::List(embeds) = &value.fields[2] else {
            return Err("embed list".into());
        };
        let digests = codec
            .canonical_value_digests(
                &[
                    CanonicalDigestInput {
                        domain: prepare::DOCUMENT_DOMAIN,
                        value: &encoded,
                    },
                    CanonicalDigestInput {
                        domain: prepare::GUEST_DOMAIN,
                        value: &embeds[0],
                    },
                ],
                &mut baseline,
            )
            .map_err(err)?;
        assert_eq!(plan.document_digest, digests[0]);
        assert_eq!(
            plan.requirements.last(),
            Some(&DocRequirement::Foreign {
                embed: EmbedRef(0),
                kind: d.value.embeds[0].kind,
                guest_digest: digests[1],
            })
        );
        assert!(
            measured.usage().work < baseline.usage().work,
            "{root:?}: optimized={:?}, baseline={:?}",
            measured.usage(),
            baseline.usage()
        );
        assert!(measured.usage().allocation_units < baseline.usage().allocation_units);
        let exact = Limits {
            work: measured.usage().work,
            allocation_units: measured.usage().allocation_units,
            ..b().limits()
        };
        assert_eq!(
            inspect(&d, &r, &store, &mut Budget::new(exact)).map_err(err)?,
            plan
        );
        for (limits, reason) in [
            (
                Limits {
                    work: exact.work - 1,
                    ..exact
                },
                StopReason::WorkLimit,
            ),
            (
                Limits {
                    allocation_units: exact.allocation_units - 1,
                    ..exact
                },
                StopReason::AllocationLimit,
            ),
        ] {
            let mut limited = Budget::new(limits);
            assert_eq!(
                inspect(&d, &r, &store, &mut limited),
                Err(PreparationError::Stopped(reason))
            );
            assert_eq!(limited.poll(), Err(reason));
        }
        // No proof persists across calls: a replaced owner must be checked.
        let DocContent::Syntax { closure } = &mut d.value.embeds[0].content else {
            return Err("fixture closure".into());
        };
        closure.owner_environment.id += 1;
        let rejected = inspect(&d, &r, &store, &mut b());
        assert!(matches!(
            rejected,
            Err(PreparationError::Label(
                nepl3_doc_core::labels::LabelError::Structure(_)
            ))
        ));
    }
    Ok(())
}
