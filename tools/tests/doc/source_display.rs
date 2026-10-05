//! Recovered source is display input, never a successfully lowered Article.
use super::*;
use nepl3_engine::{
    analysis::{BindingOptions, region},
    portable,
};
use nepl3_tools::doc::source::{parse_source_for_display, parse_source_route};

#[test]
fn doc_source_display_preserves_complete_input_and_formal_stops() -> Result<(), String> {
    let c = compiled()?;
    with_input(
        &c,
        r#"article en "Profile" body nil"#,
        "Article",
        |_, profile, _, _| {
            for native in [false, true] {
                let source = SourceSnapshot::new(
                    SourceId("display-control".into()),
                    0,
                    "memory:display-control".into(),
                    b"article en \"OK\" body nil\r\n".to_vec(),
                    &mut budget(),
                )
                .map_err(err)?;
                let strict = parse_source_route(
                    &source,
                    profile,
                    "Doc",
                    "Article",
                    &mut budget(),
                    &mut SourceAdmission::default(),
                    native,
                )?;
                let displayed = parse_source_for_display(
                    &source,
                    profile,
                    "Doc",
                    "Article",
                    &mut budget(),
                    &mut SourceAdmission::default(),
                    native,
                )?;
                assert_eq!(strict, displayed);
                assert!(displayed.recovery.is_empty());
                for stop in [
                    StopReason::Cancelled,
                    StopReason::WorkLimit,
                    StopReason::AllocationLimit,
                ] {
                    let mut limits = budget().limits();
                    if stop == StopReason::WorkLimit {
                        limits.work = 0;
                    }
                    if stop == StopReason::AllocationLimit {
                        limits.allocation_units = 0;
                    }
                    let mut b = Budget::new(limits);
                    if stop == StopReason::Cancelled {
                        b.cancel();
                    }
                    assert!(
                        parse_source_for_display(
                            &source,
                            profile,
                            "Doc",
                            "Article",
                            &mut b,
                            &mut SourceAdmission::default(),
                            native
                        )
                        .is_err()
                    );
                    assert_eq!(b.poll(), Err(stop));
                }
                let trailing = SourceSnapshot::new(
                    SourceId("display-trailing".into()),
                    0,
                    "memory:display-trailing".into(),
                    b"article en \"OK\" body nil junk".to_vec(),
                    &mut budget(),
                )
                .map_err(err)?;
                assert!(
                    parse_source_for_display(
                        &trailing,
                        profile,
                        "Doc",
                        "Article",
                        &mut budget(),
                        &mut SourceAdmission::default(),
                        native
                    )
                    .is_err_and(|e| e.contains("unexpected trailing input"))
                );
            }
            Ok(())
        },
    )
}

#[test]
fn doc_source_display_retains_recovery_on_both_host_routes() -> Result<(), String> {
    let c = compiled()?;
    with_input(
        &c,
        r#"article en "Profile" body nil"#,
        "Article",
        |_, profile, _, _| {
            for (case, input) in [
                r#"article en "Host" body cons paragraph cons code Math frac 1"#,
                r#"article en "Host" body cons paragraph cons code Math frac 1 0 nil"#,
                r#"article en "Host" body unexpected"#,
            ]
            .into_iter()
            .enumerate()
            {
                let mut trees = Vec::new();
                for native in [false, true] {
                    let mut b = budget();
                    let source = SourceSnapshot::new(
                        SourceId("display".into()),
                        0,
                        "memory:display".into(),
                        input.as_bytes().to_vec(),
                        &mut b,
                    )
                    .map_err(err)?;
                    assert!(
                        parse_source_route(
                            &source,
                            profile,
                            "Doc",
                            "Article",
                            &mut budget(),
                            &mut SourceAdmission::default(),
                            native
                        )
                        .is_err()
                    );
                    let initial_work = b.usage().work;
                    let mut admission = SourceAdmission::default();
                    let tree = parse_source_for_display(
                        &source,
                        profile,
                        "Doc",
                        "Article",
                        &mut b,
                        &mut admission,
                        native,
                    )?;
                    assert!(!tree.recovery.is_empty(), "recovery lost: {input}");
                    use nepl3_engine::recovery::RecoveryKind;
                    let end = input.len() as u64;
                    assert!(tree.recovery.iter().any(|bundle| bundle.path.is_empty()));
                    if case == 0 {
                        assert!(tree.recovery.iter().any(|bundle| !bundle.path.is_empty()
                            && bundle.entries.iter().any(|entry| matches!(&entry.kind,
                                RecoveryKind::Missing { expected, anchor }
                                if expected.alias == "Math" && anchor.start() == end
                                    && anchor.end() == end))));
                    } else {
                        assert!(tree.recovery.iter().all(|bundle| bundle.path.is_empty()));
                    }
                    if case == 2 {
                        assert!(tree.recovery.iter().flat_map(|bundle| &bundle.entries).any(
                            |entry| matches!(&entry.kind, RecoveryKind::Unparsed { span, .. }
                                if source.slice(span).is_ok_and(|text| text == "unexpected"))
                        ));
                    } else {
                        assert!(tree.recovery.iter().flat_map(|bundle| &bundle.entries).all(
                            |entry| matches!(&entry.kind, RecoveryKind::Missing { anchor, .. }
                                if anchor.start() == end && anchor.end() == end)
                        ));
                    }
                    let mut limits = budget().limits();
                    limits.work = b.usage().work - initial_work - 1;
                    let mut stopped = Budget::new(limits);
                    let error = parse_source_for_display(
                        &source,
                        profile,
                        "Doc",
                        "Article",
                        &mut stopped,
                        &mut SourceAdmission::default(),
                        native,
                    )
                    .err()
                    .ok_or("late WorkLimit unexpectedly succeeded")?;
                    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
                    assert!(
                        error.contains("candidate stopped") || error.contains("native host"),
                        "{error}"
                    );
                    tree.validate(profile, &mut b, &mut admission)
                        .map_err(err)?;
                    let retained = tree
                        .bundle
                        .sources
                        .iter()
                        .find(|s| s.identity() == source.identity())
                        .ok_or("original source lost")?;
                    assert_eq!(retained.text(), input);
                    let store = SourceStore::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &store, &mut admission)
                            .map_err(err)?;
                    let keyed = portable::analysis::prepare(
                        "doc-source-display",
                        &tree,
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut codec,
                        &mut b,
                    )
                    .map_err(err)?;
                    let prepared =
                        portable::region::prepare(&keyed, None, &mut codec, &mut b).map_err(err)?;
                    let key = prepared.key();
                    let reply = region::regions(
                        &prepared,
                        &region::RegionRequest {
                            key,
                            source: source.reference(),
                            offset: 0,
                        },
                        &mut b,
                        &mut SourceAdmission::default(),
                    );
                    let spans =
                        region::highlight::normalize(&reply, &key, &source, &mut b).map_err(err)?;
                    assert!(!spans.is_empty(), "known syntax lost: {input}");
                    let region::RegionOutcome::Complete { regions, .. } = &reply.outcome else {
                        return Err("source regions incomplete".into());
                    };
                    assert!(
                        regions
                            .iter()
                            .any(|r| r.target.part == region::RegionPart::Recovery)
                    );
                    trees.push(tree);
                }
                assert_eq!(trees[0], trees[1]);
            }
            Ok(())
        },
    )
}
