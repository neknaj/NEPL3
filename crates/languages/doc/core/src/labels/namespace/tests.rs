use super::*;
use alloc::{format, string::String};
use nepl3_core::budget::Limits;
fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 100,
        ..Limits::default()
    })
}
fn err(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        crate::schema::descriptor(&mut budget()),
        nepl3_core::schema::foundation::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    Ok(registry)
}
fn document(kinds: Vec<DocKind>) -> DocumentSyntax {
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Inline(InlineRef(0)),
            nodes: kinds
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
fn reference(name: &str) -> DocumentSyntax {
    document(vec![
        DocKind::Reference {
            target: name.into(),
            label: InlineRef(1),
        },
        DocKind::Text {
            text: "参照".into(),
        },
    ])
}
fn anchor(name: &str) -> DocumentSyntax {
    document(vec![
        DocKind::Anchor {
            id: name.into(),
            label: InlineRef(1),
        },
        DocKind::Text {
            text: "定義".into(),
        },
    ])
}
#[test]
fn ordered_members_resolve_forward_references_and_keep_owners() -> Result<(), String> {
    let registry = registry()?;
    let mut admission = SourceAdmission::default();
    let from = reference("target");
    let to = anchor("target");
    let other = anchor("other");
    let from = inspect(&from, &registry, &mut budget(), &mut admission).map_err(err)?;
    let to = inspect(&to, &registry, &mut budget(), &mut admission).map_err(err)?;
    let other = inspect(&other, &registry, &mut budget(), &mut admission).map_err(err)?;
    let members = [&from, &other, &to, &from];
    let checked = resolve(&members, &mut budget()).map_err(err)?;
    assert_eq!(
        checked
            .definitions()
            .iter()
            .map(|site| (site.member, site.site.name))
            .collect::<Vec<_>>(),
        vec![(MemberId(1), "other"), (MemberId(2), "target")]
    );
    assert_eq!(
        checked
            .references()
            .iter()
            .map(|site| (site.reference.member, site.target))
            .collect::<Vec<_>>(),
        vec![(MemberId(0), DocLabelId(1)), (MemberId(3), DocLabelId(1))]
    );
    for reference in checked.references() {
        assert!(core::ptr::eq(
            checked
                .document(reference.reference.member)
                .ok_or("owner")?,
            from.document()
        ));
        let target = &checked.definitions()[reference.target.0 as usize];
        assert!(core::ptr::eq(
            checked.document(target.member).ok_or("target owner")?,
            to.document()
        ));
    }
    assert!(checked.document(MemberId(u64::MAX)).is_none());
    // A valid but unselected member contributes no implicit/ambient binding.
    assert!(
        matches!(resolve(&[&from, &other], &mut budget()), Err(Error::Unresolved { reference }) if reference.member == MemberId(0) && reference.site.name == "target")
    );
    assert!(
        matches!(resolve(&[&to, &to], &mut budget()), Err(Error::Duplicate { definition, previous }) if definition.member == MemberId(1) && previous.member == MemberId(0))
    );
    Ok(())
}
#[test]
fn rejects_invalid_graph_repeated_declaration_and_empty_budget() -> Result<(), String> {
    let registry = registry()?;
    let mut admission = SourceAdmission::default();
    let repeated = document(vec![
        DocKind::Concat {
            inlines: vec![InlineRef(1), InlineRef(1)],
        },
        DocKind::Anchor {
            id: "target".into(),
            label: InlineRef(2),
        },
        DocKind::Text {
            text: "定義".into(),
        },
    ]);
    assert!(matches!(
        inspect(&repeated, &registry, &mut budget(), &mut admission),
        Err(Error::Input(LabelError::DuplicateOccurrence { .. }))
    ));
    let broken = document(vec![DocKind::Reference {
        target: "target".into(),
        label: InlineRef(100),
    }]);
    assert!(matches!(
        inspect(&broken, &registry, &mut budget(), &mut admission),
        Err(Error::Input(LabelError::Structure(_)))
    ));
    let empty = document(vec![DocKind::Concat { inlines: vec![] }]);
    let member = inspect(&empty, &registry, &mut budget(), &mut admission).map_err(err)?;
    let checked = resolve(&[], &mut budget()).map_err(err)?;
    assert!(checked.definitions().is_empty() && checked.references().is_empty());
    let members = [&member];
    for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
        let to = anchor("entry");
        let to = inspect(&to, &registry, &mut budget(), &mut admission).map_err(err)?;
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            _ => unreachable!(),
        }
        let mut limited = Budget::new(limits);
        assert!(
            matches!(resolve(&[&to], &mut limited), Err(Error::Stopped(actual)) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        resolve(&members, &mut cancelled),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
#[test]
fn resolution_scales_with_index_and_stops_at_success_boundaries() -> Result<(), String> {
    let registry = registry()?;
    let mut admission = SourceAdmission::default();
    let mut previous = None;
    for count in [128, 256, 512] {
        let documents: Vec<_> = (0..count)
            .rev()
            .flat_map(|index| {
                let name = format!("target-{index:04}");
                [reference(&name), anchor(&name)]
            })
            .collect();
        let inputs: Vec<_> = documents
            .iter()
            .map(|document| {
                inspect(document, &registry, &mut budget(), &mut admission).map_err(err)
            })
            .collect::<Result<_, _>>()?;
        let members: Vec<_> = inputs.iter().collect();
        let mut measured = budget();
        let checked = resolve(&members, &mut measured).map_err(err)?;
        assert_eq!(checked.definitions().len(), count);
        assert_eq!(checked.references().len(), count);
        for (index, reference) in checked.references().iter().enumerate() {
            assert_eq!(reference.target, DocLabelId(index as u64));
            assert_eq!(
                checked.definitions()[index].member,
                MemberId(index as u64 * 2 + 1)
            );
        }
        let used = measured.usage();
        if let Some(prior) = previous {
            assert!(used.work < prior * 3);
        }
        previous = Some(used.work);
        for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
            let mut limits = measured.limits();
            match reason {
                StopReason::WorkLimit => limits.work = used.work - 1,
                StopReason::AllocationLimit => limits.allocation_units = used.allocation_units - 1,
                _ => unreachable!(),
            }
            let mut limited = Budget::new(limits);
            assert!(
                matches!(resolve(&members, &mut limited), Err(Error::Stopped(actual)) if actual == reason)
            );
            assert_eq!(limited.poll(), Err(reason));
        }
    }
    Ok(())
}

fn definition(member: u64, index: u64) -> DefinitionIndex {
    DefinitionIndex {
        member: MemberId(member),
        index,
    }
}
fn reference_index(member: u64, index: u64) -> ReferenceIndex {
    ReferenceIndex {
        member: MemberId(member),
        index,
    }
}
#[test]
fn interleaving_preserves_local_order_and_member_ownership() -> Result<(), String> {
    let r = registry()?;
    let mut a = SourceAdmission::default();
    let outer = document(vec![
        DocKind::Concat {
            inlines: vec![InlineRef(1), InlineRef(3), InlineRef(5), InlineRef(7)],
        },
        DocKind::Anchor {
            id: "first".into(),
            label: InlineRef(2),
        },
        DocKind::Text {
            text: "first".into(),
        },
        DocKind::Anchor {
            id: "last".into(),
            label: InlineRef(4),
        },
        DocKind::Text {
            text: "last".into(),
        },
        DocKind::Reference {
            target: "middle".into(),
            label: InlineRef(6),
        },
        DocKind::Text {
            text: "middle".into(),
        },
        DocKind::Reference {
            target: "last".into(),
            label: InlineRef(8),
        },
        DocKind::Text {
            text: "last".into(),
        },
    ]);
    let guest = anchor("middle");
    let outer = inspect(&outer, &r, &mut budget(), &mut a).map_err(err)?;
    let guest = inspect(&guest, &r, &mut budget(), &mut a).map_err(err)?;
    let members = [&outer, &guest];
    assert_eq!(outer.definitions().len(), 2);
    assert_eq!(outer.references().len(), 2);
    let definitions = [definition(0, 0), definition(1, 0), definition(0, 1)];
    let references = [reference_index(0, 0), reference_index(0, 1)];
    let checked =
        resolve_interleaved(&members, &definitions, &references, &mut budget()).map_err(err)?;
    assert_eq!(
        checked
            .definitions()
            .iter()
            .map(|s| s.site.name)
            .collect::<Vec<_>>(),
        vec!["first", "middle", "last"]
    );
    assert_eq!(
        checked
            .references()
            .iter()
            .map(|s| s.target)
            .collect::<Vec<_>>(),
        vec![DocLabelId(1), DocLabelId(2)]
    );
    for site in checked.definitions() {
        assert!(core::ptr::eq(
            checked.document(site.member).ok_or("member")?,
            members[site.member.0 as usize].document()
        ));
    }
    let contiguous = [definition(0, 0), definition(0, 1), definition(1, 0)];
    let old = resolve(&members, &mut budget()).map_err(err)?;
    let same =
        resolve_interleaved(&members, &contiguous, &references, &mut budget()).map_err(err)?;
    assert_eq!(old.definitions(), same.definitions());
    assert_eq!(old.references(), same.references());
    for bad in [
        vec![definition(0, 0), definition(1, 0)],
        vec![definition(0, 0), definition(0, 0), definition(1, 0)],
        vec![definition(0, 1), definition(1, 0), definition(0, 0)],
        vec![definition(0, 2)],
        vec![definition(0, u64::MAX)],
        vec![definition(2, 0)],
        vec![definition(u64::MAX, 0)],
    ] {
        assert!(matches!(
            resolve_interleaved(&members, &bad, &references, &mut budget()),
            Err(Error::Order(_))
        ));
    }
    for bad in [
        vec![],
        vec![reference_index(0, 0)],
        vec![reference_index(0, 1), reference_index(0, 0)],
        vec![reference_index(0, 0), reference_index(0, 0)],
        vec![reference_index(1, 0)],
        vec![reference_index(u64::MAX, 0)],
        vec![reference_index(0, u64::MAX)],
    ] {
        assert!(matches!(
            resolve_interleaved(&members, &definitions, &bad, &mut budget()),
            Err(Error::Order(_))
        ));
    }
    Ok(())
}
#[test]
fn interleaving_counts_member_occurrences_and_preserves_stops() -> Result<(), String> {
    let r = registry()?;
    let mut a = SourceAdmission::default();
    let from = reference("target");
    let to = anchor("target");
    let from = inspect(&from, &r, &mut budget(), &mut a).map_err(err)?;
    let to = inspect(&to, &r, &mut budget(), &mut a).map_err(err)?;
    let members = [&from, &to, &from];
    let definitions = [definition(1, 0)];
    let references = [reference_index(2, 0), reference_index(0, 0)];
    let mut measured = budget();
    let checked =
        resolve_interleaved(&members, &definitions, &references, &mut measured).map_err(err)?;
    assert_eq!(
        checked
            .references()
            .iter()
            .map(|r| r.reference.member)
            .collect::<Vec<_>>(),
        vec![MemberId(2), MemberId(0)]
    );
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = measured.limits();
        match reason {
            StopReason::WorkLimit => limits.work = measured.usage().work - 1,
            StopReason::AllocationLimit => {
                limits.allocation_units = measured.usage().allocation_units - 1
            }
            _ => {}
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert!(
            matches!(resolve_interleaved(&members, &definitions, &references, &mut b), Err(Error::Stopped(actual)) if actual == reason)
        );
        assert_eq!(b.poll(), Err(reason));
    }
    assert!(
        matches!(resolve_interleaved(&[&to,&to], &[definition(1,0),definition(0,0)], &[], &mut budget()), Err(Error::Duplicate {definition, previous}) if definition.member == MemberId(0) && previous.member == MemberId(1))
    );
    assert!(matches!(
        resolve_interleaved(&[&from], &[], &[reference_index(0, 0)], &mut budget()),
        Err(Error::Unresolved { .. })
    ));
    for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            _ => {}
        }
        let mut limited = Budget::new(limits);
        assert!(
            matches!(resolve_interleaved(&members, &definitions, &references, &mut limited),
            Err(Error::Stopped(actual)) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
    }
    let mut nested = budget();
    nested
        .with_depth(|b| -> Result<(), Error<'_>> {
            let entered = b.current_depth();
            resolve_interleaved(&members, &definitions, &references, b)?;
            assert_eq!(b.current_depth(), entered);
            assert!(matches!(
                resolve_interleaved(&members, &[], &references, b),
                Err(Error::Order(_))
            ));
            assert_eq!(b.current_depth(), entered);
            b.cancel();
            assert!(matches!(
                resolve_interleaved(&members, &definitions, &references, b),
                Err(Error::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(b.current_depth(), entered);
            Ok(())
        })
        .map_err(err)?;
    assert_eq!(nested.current_depth(), 0);
    assert_eq!(nested.poll(), Err(StopReason::Cancelled));
    let empty = document(vec![DocKind::Concat { inlines: vec![] }]);
    let empty = inspect(&empty, &r, &mut budget(), &mut a).map_err(err)?;
    let empty_members = vec![&empty; 100];
    let mut measured = budget();
    resolve_interleaved(&empty_members, &[], &[], &mut measured).map_err(err)?;
    assert!(measured.usage().work >= 400);
    assert!(measured.usage().allocation_units >= 1600);
    Ok(())
}

#[test]
fn schedule_errors_identify_exact_domain_member_and_local_index() -> Result<(), String> {
    let r = registry()?;
    let mut a = SourceAdmission::default();
    let from = reference("target");
    let to = anchor("target");
    let from = inspect(&from, &r, &mut budget(), &mut a).map_err(err)?;
    let to = inspect(&to, &r, &mut budget(), &mut a).map_err(err)?;
    let members = [&to, &from];
    let definitions = [definition(0, 0)];
    let references = [reference_index(1, 0)];
    for (defs, refs, expected) in [
        (
            vec![definition(2, 0)],
            references.to_vec(),
            OrderError::Member {
                kind: SiteKind::Definition,
                member: MemberId(2),
            },
        ),
        (
            vec![definition(0, 1)],
            references.to_vec(),
            OrderError::Index {
                kind: SiteKind::Definition,
                member: MemberId(0),
                index: 1,
            },
        ),
        (
            vec![definition(0, 0), definition(0, 0)],
            references.to_vec(),
            OrderError::Next {
                kind: SiteKind::Definition,
                member: MemberId(0),
                expected: 1,
                actual: 0,
            },
        ),
        (
            vec![],
            references.to_vec(),
            OrderError::Missing {
                kind: SiteKind::Definition,
                member: MemberId(0),
                next: 0,
                total: 1,
            },
        ),
        (
            definitions.to_vec(),
            vec![reference_index(2, 0)],
            OrderError::Member {
                kind: SiteKind::Reference,
                member: MemberId(2),
            },
        ),
        (
            definitions.to_vec(),
            vec![reference_index(1, 1)],
            OrderError::Index {
                kind: SiteKind::Reference,
                member: MemberId(1),
                index: 1,
            },
        ),
        (
            definitions.to_vec(),
            vec![reference_index(1, 0), reference_index(1, 0)],
            OrderError::Next {
                kind: SiteKind::Reference,
                member: MemberId(1),
                expected: 1,
                actual: 0,
            },
        ),
        (
            definitions.to_vec(),
            vec![],
            OrderError::Missing {
                kind: SiteKind::Reference,
                member: MemberId(1),
                next: 0,
                total: 1,
            },
        ),
    ] {
        let error = resolve_interleaved(&members, &defs, &refs, &mut budget())
            .err()
            .ok_or("schedule accepted")?;
        assert_eq!(error, Error::Order(expected));
    }
    Ok(())
}
