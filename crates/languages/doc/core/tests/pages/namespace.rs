use super::*;
use nepl3_doc_core::labels::namespace as labels;
use nepl3_doc_core::pages::namespace as routing;

fn b() -> Budget {
    let mut limits = super::b().limits();
    limits.source_bytes = 1_000_000;
    Budget::new(limits)
}

fn guest(target: LinkTarget) -> DocumentSyntax {
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Inline(InlineRef(0)),
            nodes: vec![
                DocKind::Concat {
                    inlines: vec![InlineRef(1), InlineRef(2)],
                },
                DocKind::Anchor {
                    id: "guest".into(),
                    label: InlineRef(3),
                },
                DocKind::Link {
                    target,
                    label: InlineRef(3),
                },
                DocKind::Text {
                    text: "guest label".into(),
                },
            ]
            .into_iter()
            .map(|kind| DocNode {
                kind,
                locations: vec![],
                origin: None,
                span: None,
            })
            .collect(),
            embeds: Default::default(),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}

fn with_namespaces<T>(
    input: &PageSet,
    guests: &[DocumentSyntax],
    operation: impl FnOnce(&[&labels::CheckedNamespace<'_, '_>], &SchemaRegistry) -> Result<T, String>,
) -> Result<T, String> {
    let registry = registry()?;
    let mut members = Vec::new();
    for (page, guest) in input.pages.iter().zip(guests) {
        members.push([
            labels::inspect(
                &page.document,
                &registry,
                &mut b(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?,
            labels::inspect(guest, &registry, &mut b(), &mut SourceAdmission::default())
                .map_err(err)?,
        ]);
    }
    let refs = members.iter().map(|m| [&m[0], &m[1]]).collect::<Vec<_>>();
    let namespaces = refs
        .iter()
        .map(|m| labels::resolve(m, &mut b()).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    operation(&namespaces.iter().collect::<Vec<_>>(), &registry)
}
fn fixtures() -> (PageSet, Vec<DocumentSyntax>) {
    let mut input = set();
    for page in &mut input.pages {
        if let DocKind::Link {
            target: LinkTarget::Page { fragment, .. } | LinkTarget::Relative { fragment, .. },
            ..
        } = &mut page.document.value.nodes[7].kind
        {
            *fragment = Some("guest".into());
        }
        // Outer local reference must be satisfied only by this page's guest.
        page.document.value.nodes[6].kind = DocKind::Reference {
            target: "guest".into(),
            label: InlineRef(2),
        };
    }
    let guests = vec![
        guest(LinkTarget::Page {
            page: "second".into(),
            fragment: Some("guest".into()),
        }),
        guest(LinkTarget::Relative {
            path: "../first.nepld".into(),
            fragment: Some("guest".into()),
        }),
    ];
    (input, guests)
}
#[test]
fn explicit_members_resolve_local_and_cross_page_targets_without_flattening() -> Result<(), String>
{
    let (input, guests) = fixtures();
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        let checked =
            routing::resolve(&input, namespaces, registry, &mut codec, &mut b()).map_err(err)?;
        assert_eq!(checked.links().len(), 4);
        for (index, link) in checked.links().iter().enumerate() {
            assert_eq!(link.site.page, (index / 2) as u64);
            assert_eq!(link.site.member, labels::MemberId((index % 2) as u64));
            assert_eq!(link.site.node, if index % 2 == 0 { 7 } else { 2 });
            assert_eq!(
                link.target,
                PageDestination::Page {
                    index: 1 - (index / 2) as u64
                }
            );
            assert_eq!(link.fragment.as_deref(), Some("guest"));
        }
        assert!(checked.remaining().is_empty());
        // Ordinary outer-only resolution remains unchanged and cannot import guests.
        assert!(resolve(&input, registry, &mut codec, &mut b()).is_err());
        Ok(())
    })
}
#[test]
fn exact_root_binding_and_namespace_count_are_required() -> Result<(), String> {
    let (input, guests) = fixtures();
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        assert!(matches!(
            routing::resolve(&input.clone(), namespaces, registry, &mut codec, &mut b()),
            Err(routing::Error::RootMismatch { page: 0 })
        ));
        assert!(matches!(
            routing::resolve(&input, &namespaces[..1], registry, &mut codec, &mut b()),
            Err(routing::Error::PageCount)
        ));
        let reversed = [namespaces[1], namespaces[0]];
        assert!(matches!(
            routing::resolve(&input, &reversed, registry, &mut codec, &mut b()),
            Err(routing::Error::RootMismatch { page: 0 })
        ));
        Ok(())
    })
}
#[test]
fn missing_fragment_reports_the_guest_site_not_an_outer_arena() -> Result<(), String> {
    let (input, mut guests) = fixtures();
    guests[1].value.nodes[2].kind = DocKind::Link {
        target: LinkTarget::Page {
            page: "first".into(),
            fragment: Some("absent".into()),
        },
        label: InlineRef(3),
    };
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        assert!(matches!(
            routing::resolve(&input, namespaces, registry, &mut codec, &mut b()),
            Err(routing::Error::MissingFragment {
                site: routing::Site {
                    page: 1,
                    member: labels::MemberId(1),
                    node: 2
                },
                target: 0
            })
        ));
        Ok(())
    })
}
#[test]
fn external_links_remain_typed_requirements_and_budget_is_cumulative() -> Result<(), String> {
    let (input, mut guests) = fixtures();
    guests[1].value.nodes[2].kind = DocKind::Link {
        target: LinkTarget::External {
            uri: "javascript:bad".into(),
        },
        label: InlineRef(3),
    };
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        let mut measured = b();
        measured.charge(Resource::Work, 19).map_err(err)?;
        let checked = routing::resolve(&input, namespaces, registry, &mut codec, &mut measured)
            .map_err(err)?;
        let ids = checked.remaining();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].page, 1);
        assert_eq!(ids[0].member, labels::MemberId(1));
        assert!(matches!(
            checked.requirement(ids[0]),
            Some(nepl3_doc_core::prepare::DocRequirement::Link {
                target: LinkTarget::External { .. },
                ..
            })
        ));
        for deficit in [0, 1] {
            let mut limits = b().limits();
            limits.work = measured.usage().work - deficit;
            let mut limited = Budget::new(limits);
            limited.charge(Resource::Work, 19).map_err(err)?;
            let result = routing::resolve(&input, namespaces, registry, &mut codec, &mut limited);
            if deficit == 0 {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(routing::Error::Stopped(_))));
            }
        }
        Ok(())
    })
}

#[test]
fn guest_links_preserve_relative_path_and_file_boundaries() -> Result<(), String> {
    for (target, succeeds) in [
        (
            LinkTarget::Relative {
                path: "".into(),
                fragment: Some("guest".into()),
            },
            true,
        ),
        (
            LinkTarget::Relative {
                path: "../../../escape".into(),
                fragment: None,
            },
            false,
        ),
        (
            LinkTarget::Page {
                page: "missing".into(),
                fragment: None,
            },
            false,
        ),
        (
            LinkTarget::Relative {
                path: "asset.bin".into(),
                fragment: None,
            },
            true,
        ),
        (
            LinkTarget::Relative {
                path: "asset.bin".into(),
                fragment: Some("guest".into()),
            },
            false,
        ),
        (
            LinkTarget::Page {
                page: "file".into(),
                fragment: None,
            },
            false,
        ),
    ] {
        let (mut input, mut guests) = fixtures();
        input.files.push(PageFile {
            registration: PageRegistration {
                id: "file".into(),
                source: "doc/next/asset.bin".into(),
                route: "downloads/data.bin".into(),
            },
            content: FileBytes(vec![0, 255]),
        });
        guests[1].value.nodes[2].kind = DocKind::Link {
            target,
            label: InlineRef(3),
        };
        with_namespaces(&input, &guests, |namespaces, registry| {
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
            let result = routing::resolve(&input, namespaces, registry, &mut codec, &mut b());
            assert_eq!(result.is_ok(), succeeds);
            if let Err(routing::Error::Destination { member, .. }) = result {
                assert_eq!(member, labels::MemberId(1));
            }
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn preexisting_stop_and_allocation_limits_never_return_partial_routing() -> Result<(), String> {
    let (input, guests) = fixtures();
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        let mut stopped = b();
        let reason = stopped.stop(nepl3_core::budget::StopReason::Cancelled);
        assert!(
            matches!(routing::resolve(&input, namespaces, registry, &mut codec, &mut stopped), Err(routing::Error::Stopped(s)) if s == reason)
        );
        let mut limits = b().limits();
        limits.allocation_units = 0;
        let mut limited = Budget::new(limits);
        assert!(matches!(
            routing::resolve(&input, namespaces, registry, &mut codec, &mut limited),
            Err(routing::Error::Stopped(
                nepl3_core::budget::StopReason::AllocationLimit
            ))
        ));
        Ok(())
    })
}

#[test]
fn fresh_source_admission_counts_shared_sources_once_and_enforces_ceiling() -> Result<(), String> {
    use nepl3_core::source::{SourceId, SourceSnapshot};
    let (mut input, mut guests) = fixtures();
    let mut creation = b();
    let source = SourceSnapshot::new(
        SourceId("shared".into()),
        0,
        "memory:shared".into(),
        "共有".as_bytes().to_vec(),
        &mut creation,
    )
    .map_err(err)?;
    for page in &mut input.pages {
        page.document.sources.push(source.clone());
    }
    for guest in &mut guests {
        guest.sources.push(source.clone());
    }
    with_namespaces(&input, &guests, |namespaces, registry| {
        for allowance in [6, 5] {
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
            let mut limits = b().limits();
            limits.source_bytes = allowance;
            let mut budget = Budget::new(limits);
            let result = routing::resolve(&input, namespaces, registry, &mut codec, &mut budget);
            if allowance == 6 {
                result.map_err(err)?;
                assert_eq!(budget.usage().source_bytes, 6);
            } else {
                assert!(matches!(
                    result,
                    Err(routing::Error::Stopped(
                        nepl3_core::budget::StopReason::SourceLimit
                    ))
                ));
            }
        }
        Ok(())
    })
}
#[test]
fn deeper_reentry_rechecks_members_under_current_depth() -> Result<(), String> {
    let (input, guests) = fixtures();
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        let mut budget = b();
        let result = budget.with_depth_at_least(999, |budget| {
            routing::resolve(&input, namespaces, registry, &mut codec, budget)
        });
        assert!(matches!(
            result,
            Err(routing::Error::Stopped(
                nepl3_core::budget::StopReason::DepthLimit
            ))
        ));
        Ok(())
    })
}

#[test]
fn later_guest_source_conflict_preserves_page_and_member() -> Result<(), String> {
    use nepl3_core::source::{SourceId, SourceSnapshot};
    let (input, mut guests) = fixtures();
    for (index, guest) in guests.iter_mut().enumerate() {
        guest.sources.push(
            SourceSnapshot::new(
                SourceId("same".into()),
                0,
                "memory:same".into(),
                vec![b'a' + index as u8],
                &mut b(),
            )
            .map_err(err)?,
        );
    }
    with_namespaces(&input, &guests, |namespaces, registry| {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
        assert!(matches!(
            routing::resolve(&input, namespaces, registry, &mut codec, &mut b()),
            Err(routing::Error::Preparation {
                page: 1,
                member: Some(labels::MemberId(1)),
                ..
            })
        ));
        Ok(())
    })
}
