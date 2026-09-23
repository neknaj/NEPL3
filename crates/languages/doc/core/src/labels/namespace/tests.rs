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
            embeds: vec![],
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
