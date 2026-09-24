use super::*;
use nepl3_doc_core::portable::pages::namespace as packet;

fn guests(r: &SchemaRegistry) -> Result<[Vec<DocumentSyntax>; 2], String> {
    let mut out = [Vec::new(), Vec::new()];
    for (index, page) in out.iter_mut().enumerate() {
        page.push(inline(
            DocKind::Anchor {
                id: "導入".into(),
                label: EmbedRef(0),
            },
            r,
        )?);
        page.push(inline(
            DocKind::Link {
                target: LinkTarget::Page {
                    page: if index == 0 { "second" } else { "first" }.into(),
                    fragment: Some("導入".into()),
                },
                label: EmbedRef(0),
            },
            r,
        )?);
    }
    Ok(out)
}

fn with_scope<T>(
    set: &pages::PageSet,
    guests: &[Vec<DocumentSyntax>; 2],
    r: &SchemaRegistry,
    finish: impl FnOnce(
        &pagespaces::CheckedPageNamespaces<'_, '_, '_>,
        &mut FoundationCodec<'_>,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let mut admission = SourceAdmission::default();
    let mut members = Vec::new();
    for (page, guests) in set.pages.iter().zip(guests) {
        members.push(
            std::iter::once(&page.document)
                .chain(guests.iter())
                .map(|d| names::inspect(d, r, &mut b(), &mut admission).map_err(err))
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    let members = members
        .iter()
        .map(|page| page.iter().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let namespaces = members
        .iter()
        .map(|page| names::resolve(page, &mut b()).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let namespaces = namespaces.iter().collect::<Vec<_>>();
    let sources = SourceStore::default();
    let mut c = FoundationCodec::new(r, &sources, &mut admission).map_err(err)?;
    let checked = pagespaces::resolve(set, &namespaces, r, &mut c, &mut b()).map_err(err)?;
    finish(&checked, &mut c)
}

#[test]
fn first_receiver_rebuilds_namespaces_and_rejects_forged_member_plans() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let guests = guests(&r)?;
    let encoded = with_scope(&set, &guests, &r, |proof, c| {
        packet::to_value(proof, &r, c, &mut b()).map_err(err)
    })?;
    let bytes = nepl3_wire::encode(&encoded, &mut b()).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    // Transfer all selected input documents through the public wire boundary.
    // The receiver owns new documents and constructs its own namespace proof.
    let sources = SourceStore::default();
    let mut outgoing = SourceAdmission::default();
    let mut sender = FoundationCodec::new(&r, &sources, &mut outgoing).map_err(err)?;
    let set_value = portable::pages::set_to_value(&set, &r, &mut sender, &mut b()).map_err(err)?;
    let set_bytes = nepl3_wire::encode(&set_value, &mut b()).map_err(err)?;
    let mut incoming = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(&r, &sources, &mut incoming).map_err(err)?;
    let set = portable::pages::set_from_value(
        &nepl3_wire::decode(&set_bytes, &mut b()).map_err(err)?,
        &r,
        &mut receiver,
        &mut b(),
    )
    .map_err(err)?;
    let mut selected = [Vec::new(), Vec::new()];
    for (output, input) in selected.iter_mut().zip(&guests) {
        for guest in input {
            let value = portable::to_value(guest, &r, &mut sender, &mut b()).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
            output.push(
                portable::from_value(
                    &nepl3_wire::decode(&bytes, &mut b()).map_err(err)?,
                    &r,
                    &mut receiver,
                    &mut b(),
                )
                .map_err(err)?,
            );
        }
    }
    with_scope(&set, &selected, &r, |proof, c| {
        let raw = packet::from_value(&received, proof, &r, c, &mut b()).map_err(err)?;
        assert!(matches!(
            packet::from_value(&NdfValue::Unit, proof, &r, c, &mut b()),
            Err(portable::PortableError::Schema(_))
        ));
        assert_eq!(raw.identity, proof.identity());
        assert_eq!(raw.members.len(), 6);
        for (page, index) in [(0, 2), (1, 5)] {
            let member = &raw.members[index];
            assert_eq!((member.page, member.member), (page, 2));
            assert_eq!(
                member.links,
                vec![pages::PageLink {
                    page,
                    node: 0,
                    target: pages::PageDestination::Page { index: 1 - page },
                    fragment: Some("導入".into()),
                }]
            );
            assert_eq!(
                member.remaining.len(),
                1,
                "independent Inline label requires preparation"
            );
        }
        for case in 0..10 {
            let mut bad = received.clone();
            let NdfValue::Record(plan) = &mut bad else {
                return Err("plan".into());
            };
            let NdfValue::List(members) = &mut plan.fields[1] else {
                return Err("members".into());
            };
            match case {
                0 => {
                    plan.fields[0] = NdfValue::Bytes(vec![0; 32]);
                }
                1 | 2 | 3 | 8 => {
                    let NdfValue::Record(member) = &mut members[2] else {
                        return Err("member".into());
                    };
                    match case {
                        1 => member.fields[0] = NdfValue::U64(1),
                        2 => member.fields[1] = NdfValue::U64(1),
                        3 => member.fields[2] = NdfValue::Bytes(vec![0; 32]),
                        _ => member.fields[4] = NdfValue::List(vec![]),
                    }
                }
                4 => members.swap(1, 2),
                5 => {
                    members.pop();
                }
                6 | 7 => {
                    let NdfValue::Record(member) = &mut members[2] else {
                        return Err("member".into());
                    };
                    let NdfValue::List(links) = &mut member.fields[3] else {
                        return Err("links".into());
                    };
                    let NdfValue::Record(link) = &mut links[0] else {
                        return Err("link".into());
                    };
                    if case == 6 {
                        let NdfValue::Variant(destination) = &mut link.fields[2] else {
                            return Err("destination".into());
                        };
                        destination.fields[0] = NdfValue::U64(0);
                    } else {
                        link.fields[3] = NdfValue::Some(Box::new(NdfValue::Text("forged".into())));
                    }
                }
                _ => members.push(members[2].clone()),
            }
            assert!(
                matches!(
                    packet::from_value(&bad, proof, &r, c, &mut b()),
                    Err(portable::PortableError::Shape)
                ),
                "case={case}"
            );
        }
        Ok(())
    })?;
    let mut changed = set.clone();
    changed.pages[1].registration.route = "moved/second.html".into();
    with_scope(&changed, &selected, &r, |proof, c| {
        assert!(matches!(
            packet::from_value(&received, proof, &r, c, &mut b()),
            Err(portable::PortableError::Shape)
        ));
        Ok(())
    })?;
    for repeat in [false, true] {
        let mut changed = selected.clone();
        if repeat {
            changed[0].push(changed[0][1].clone());
        } else {
            changed[0].remove(1);
        }
        with_scope(&set, &changed, &r, |proof, c| {
            assert!(matches!(
                packet::from_value(&received, proof, &r, c, &mut b()),
                Err(portable::PortableError::Shape)
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn file_link_plan_receipt_binds_registered_file_bytes() -> Result<(), String> {
    let r = registry()?;
    let set = set(&r)?;
    let mut selected = guests(&r)?;
    selected[0][1] = inline(
        DocKind::Link {
            target: LinkTarget::Relative {
                path: "../data/attachment.bin".into(),
                fragment: None,
            },
            label: EmbedRef(0),
        },
        &r,
    )?;
    let value = with_scope(&set, &selected, &r, |proof, c| {
        let value = packet::to_value(proof, &r, c, &mut b()).map_err(err)?;
        let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
        let received = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
        let plan = packet::from_value(&received, proof, &r, c, &mut b()).map_err(err)?;
        assert_eq!(
            plan.members[2].links,
            vec![pages::PageLink {
                page: 0,
                node: 0,
                target: pages::PageDestination::File { index: 0 },
                fragment: None,
            }]
        );
        Ok(received)
    })?;
    let mut changed = set;
    changed.files[0].content.0[1] = 254;
    with_scope(&changed, &selected, &r, |proof, c| {
        assert!(matches!(
            packet::from_value(&value, proof, &r, c, &mut b()),
            Err(portable::PortableError::Shape)
        ));
        Ok(())
    })
}

#[test]
fn namespace_packet_encoding_and_receipt_obey_sticky_limits() -> Result<(), String> {
    let r = registry()?;
    with_scope(&set(&r)?, &guests(&r)?, &r, |proof, c| {
        let value = packet::to_value(proof, &r, c, &mut b()).map_err(err)?;
        for encode in [true, false] {
            let mut run = |budget: &mut Budget| {
                if encode {
                    packet::to_value(proof, &r, c, budget).map(|_| ())
                } else {
                    packet::from_value(&value, proof, &r, c, budget).map(|_| ())
                }
            };
            let mut measured = b();
            run(&mut measured).map_err(err)?;
            for (reason, used) in [
                (StopReason::WorkLimit, measured.usage().work),
                (
                    StopReason::AllocationLimit,
                    measured.usage().allocation_units,
                ),
                (StopReason::NodeLimit, measured.usage().nodes),
                (StopReason::DepthLimit, measured.usage().depth),
            ] {
                for amount in [used - 1, used] {
                    let mut limits = b().limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = amount,
                        StopReason::AllocationLimit => limits.allocation_units = amount,
                        StopReason::NodeLimit => limits.nodes = amount,
                        StopReason::DepthLimit => limits.depth = amount,
                        _ => unreachable!(),
                    }
                    let mut limited = Budget::new(limits);
                    if amount == used {
                        run(&mut limited).map_err(err)?;
                    } else {
                        assert!(
                            matches!(run(&mut limited), Err(portable::PortableError::Stopped(s)) if s == reason)
                        );
                        assert_eq!(limited.poll(), Err(reason));
                        assert!(
                            matches!(run(&mut limited), Err(portable::PortableError::Stopped(s)) if s == reason)
                        );
                    }
                }
            }
        }
        Ok(())
    })
}
