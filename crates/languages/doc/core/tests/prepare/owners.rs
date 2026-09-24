use super::*;
use nepl3_core::syntax::OwnerProvenance;

fn pair(r: &SchemaRegistry, separate: bool, different: bool) -> Result<DocumentSyntax, String> {
    let mut d = document(r)?;
    let DocContent::Syntax { closure } = &mut d.value.embeds[0].content else {
        return Err("closure".into());
    };
    closure.provenance = OwnerProvenance::from_parts(
        vec![Origin::Synthetic {
            reason: "owner zero (guest zero is a Direct origin)".into(),
            anchor: None,
        }],
        vec![],
        vec![],
    );
    let mut second = d.value.embeds[0].clone();
    if separate || different {
        let DocContent::Syntax { closure } = &mut second.content else {
            return Err("closure".into());
        };
        let mut origins = closure.provenance.origins().to_vec();
        if different {
            origins.push(Origin::Synthetic {
                reason: "second".into(),
                anchor: None,
            });
        }
        closure.provenance = OwnerProvenance::from_parts(origins, vec![], vec![]);
    }
    d.value.embeds.push(second);
    d.value.nodes.push(DocNode {
        kind: DocKind::Sentence {
            syntax: EmbedRef(1),
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    let DocKind::Image { alt, .. } = &mut d.value.nodes[4].kind else {
        return Err("image".into());
    };
    *alt = SentenceRef(5);
    Ok(d)
}

fn doc_fields(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    let NdfValue::Record(document) = value else {
        return Err("DocumentSyntax".into());
    };
    let NdfValue::Record(value) = &mut document.fields[0] else {
        return Err("DocValue".into());
    };
    Ok(&mut value.fields)
}

#[test]
fn structure_reuses_only_identical_owner_tables_and_checks_each_guest() -> Result<(), String> {
    use nepl3_core::syntax::SyntaxError;
    use nepl3_doc_core::check::StructureError;
    let r = registry()?;
    let mut shared = pair(&r, false, false)?;
    let mut origins = (0..16)
        .map(|i| Origin::Composite(vec![OriginId(i + 1)]))
        .collect::<Vec<_>>();
    origins.push(Origin::Synthetic {
        reason: "deep owner".into(),
        anchor: None,
    });
    let owner = OwnerProvenance::from_parts(origins, vec![], vec![]);
    for embed in &mut shared.value.embeds {
        let DocContent::Syntax { closure } = &mut embed.content else {
            return Err("syntax".into());
        };
        closure.provenance = owner.clone();
    }
    let mut independent = shared.clone();
    for embed in &mut independent.value.embeds {
        let DocContent::Syntax { closure } = &mut embed.content else {
            return Err("syntax".into());
        };
        closure.provenance = OwnerProvenance::from_parts(owner.origins().to_vec(), vec![], vec![]);
    }
    let run = |doc: &DocumentSyntax, budget: &mut Budget| {
        doc.validate_structure(&r, budget, &mut SourceAdmission::default())
            .map(|_| ())
    };
    let mut shared_budget = b();
    run(&shared, &mut shared_budget).map_err(err)?;
    let mut independent_budget = b();
    run(&independent, &mut independent_budget).map_err(err)?;
    // The title uses slot 0; the deeper image-alt occurrence uses slot 1.
    // Reusing the owner must preserve the full traversal's relative depth.
    assert!(shared_budget.usage().depth > 16);
    assert_eq!(
        shared_budget.usage().depth,
        independent_budget.usage().depth
    );
    #[cfg(target_has_atomic = "ptr")]
    assert!(shared_budget.usage().work < independent_budget.usage().work);
    // The second slot shares the first owner's tables, but its selected
    // environment and guest graph must still be validated independently.
    for environment in [true, false] {
        let mut bad = shared.clone();
        let DocContent::Syntax { closure } = &mut bad.value.embeds[1].content else {
            return Err("syntax".into());
        };
        if environment {
            closure.owner_environment.id += 1;
        } else {
            closure.syntax.root = NodeRef(u64::MAX);
        }
        assert!(matches!(
            run(&bad, &mut b()),
            Err(StructureError::Syntax(_))
        ));
    }
    // A replacement owner cannot inherit the preceding proof.
    let mut bad = shared.clone();
    let DocContent::Syntax { closure } = &mut bad.value.embeds[1].content else {
        return Err("syntax".into());
    };
    closure.provenance = OwnerProvenance::from_parts(
        vec![closure.syntax.bundle.origins[0].clone()],
        vec![],
        vec![],
    );
    assert!(matches!(
        run(&bad, &mut b()),
        Err(StructureError::Syntax(SyntaxError::Origin(_)))
    ));
    let usage = shared_budget.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, usage.work),
        (StopReason::AllocationLimit, usage.allocation_units),
        (StopReason::NodeLimit, usage.nodes),
        (StopReason::DepthLimit, usage.depth),
    ] {
        for limit in [amount - 1, amount] {
            let mut limits = b().limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::NodeLimit => limits.nodes = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => return Err("test resource".into()),
            }
            let mut limited = Budget::new(limits);
            if limit == amount {
                run(&shared, &mut limited).map_err(err)?;
            } else {
                assert!(
                    matches!(run(&shared, &mut limited), Err(StructureError::Stopped(s)) if s == reason)
                );
                let stopped = limited.usage();
                assert!(
                    matches!(run(&shared, &mut limited), Err(StructureError::Stopped(s)) if s == reason)
                );
                assert_eq!(limited.usage(), stopped);
            }
        }
    }
    Ok(())
}
fn owner_table(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    let NdfValue::List(values) = &mut doc_fields(value)?[3] else {
        return Err("owners".into());
    };
    Ok(values)
}
fn closure_fields(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    closure_fields_at(value, 0)
}
fn closure_fields_at(value: &mut NdfValue, index: usize) -> Result<&mut Vec<NdfValue>, String> {
    let NdfValue::List(embeds) = &mut doc_fields(value)?[2] else {
        return Err("embeds".into());
    };
    let NdfValue::Record(embed) = &mut embeds[index] else {
        return Err("embed".into());
    };
    let NdfValue::Variant(content) = &mut embed.fields[1] else {
        return Err("content".into());
    };
    let NdfValue::Record(closure) = &mut content.fields[0] else {
        return Err("closure".into());
    };
    Ok(&mut closure.fields)
}

#[test]
fn owner_storage_independence_roundtrip_and_standalone_closure() -> Result<(), String> {
    let r = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let shared = pair(&r, false, false)?;
    let independent = pair(&r, true, false)?;
    let mut a = portable::to_value(&shared, &r, &mut c, &mut b()).map_err(err)?;
    let z = portable::to_value(&independent, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(a, z);
    assert_eq!(owner_table(&mut a)?.len(), 1);
    let bytes = nepl3_wire::encode(&a, &mut b()).map_err(err)?;
    assert_eq!(bytes, nepl3_wire::encode(&z, &mut b()).map_err(err)?);
    let incoming = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let restored = portable::from_value(&incoming, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(restored, shared);
    #[cfg(target_has_atomic = "ptr")]
    {
        let [first, second] = restored.value.embeds.as_slice() else {
            return Err("two embeds".into());
        };
        let (DocContent::Syntax { closure: a }, DocContent::Syntax { closure: z }) =
            (&first.content, &second.content)
        else {
            return Err("syntax".into());
        };
        assert!(core::ptr::eq(
            a.provenance.origins(),
            z.provenance.origins()
        ));
    }
    for embed in &restored.value.embeds {
        let DocContent::Syntax { closure } = &embed.content else {
            return Err("syntax".into());
        };
        assert!(matches!(
            closure.provenance.origins()[0],
            Origin::Synthetic { .. }
        ));
        assert!(matches!(
            closure.syntax.bundle.origins[0],
            Origin::Direct(_)
        ));
        let value = c.encode_foreign_closure(closure, &mut b()).map_err(err)?;
        let mut incoming = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(&r, &sources, &mut incoming).map_err(err)?;
        assert_eq!(
            receiver
                .decode_foreign_closure(&value, &mut b())
                .map_err(err)?,
            **closure
        );
    }
    Ok(())
}

#[test]
fn compact_closure_checks_environment_digest_and_budget_boundaries() -> Result<(), String> {
    let r = registry()?;
    let d = pair(&r, false, false)?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let value = portable::to_value(&d, &r, &mut c, &mut b()).map_err(err)?;
    let mut corrupt = value.clone();
    let NdfValue::Record(environment) = &mut closure_fields(&mut corrupt)?[4] else {
        return Err("environment".into());
    };
    environment.fields[1] = NdfValue::Bytes(vec![0; 32]);
    assert!(matches!(
        portable::from_value(&corrupt, &r, &mut c, &mut b()),
        Err(portable::PortableError::Foundation(
            nepl3_wire::WireError::Syntax(nepl3_core::syntax::SyntaxError::Environment)
        ))
    ));
    for encode in [true, false] {
        let run = |budget: &mut Budget| {
            // Each boundary uses fresh admission, so successful earlier calls
            // cannot lower the work required by the receiving operation.
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &sources, &mut admission)
                .map_err(portable::PortableError::Foundation)?;
            if encode {
                let actual = portable::to_value(&d, &r, &mut c, budget)?;
                assert_eq!(actual, value);
            } else {
                let actual = portable::from_value(&value, &r, &mut c, budget)?;
                assert_eq!(actual, d);
            }
            Ok::<_, portable::PortableError<nepl3_wire::WireError>>(())
        };
        let mut measured = b();
        run(&mut measured).map_err(err)?;
        let usage = measured.usage();
        for (reason, amount) in [
            (StopReason::WorkLimit, usage.work),
            (StopReason::AllocationLimit, usage.allocation_units),
            (StopReason::NodeLimit, usage.nodes),
            (StopReason::DepthLimit, usage.depth),
        ] {
            for limit in [amount - 1, amount] {
                let mut limits = b().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = limit,
                    StopReason::AllocationLimit => limits.allocation_units = limit,
                    StopReason::NodeLimit => limits.nodes = limit,
                    StopReason::DepthLimit => limits.depth = limit,
                    _ => unreachable!(),
                }
                let mut limited = Budget::new(limits);
                if limit == amount {
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
}

#[test]
fn owner_sources_roundtrip_and_reject_duplicate_or_forged_content() -> Result<(), String> {
    let r = registry()?;
    let mut d = pair(&r, false, false)?;
    let source = SourceSnapshot::new(
        SourceId("owner".into()),
        4,
        "memory:owner".into(),
        "元".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let owner = OwnerProvenance::from_parts(
        vec![Origin::Direct(source.span(0, 3).map_err(err)?)],
        vec![source],
        vec![],
    );
    for embed in &mut d.value.embeds {
        let DocContent::Syntax { closure } = &mut embed.content else {
            return Err("syntax".into());
        };
        closure.provenance = owner.clone();
    }
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let value = portable::to_value(&d, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        portable::from_value(&value, &r, &mut c, &mut b()).map_err(err)?,
        d
    );
    for duplicate in [true, false] {
        let mut bad = value.clone();
        let owner = &mut owner_table(&mut bad)?[0];
        let NdfValue::Record(record) = owner else {
            return Err("owner".into());
        };
        let NdfValue::List(entries) = &mut record.fields[0] else {
            return Err("sources".into());
        };
        if duplicate {
            entries.push(entries[0].clone());
        } else {
            let NdfValue::Record(source) = &mut entries[0] else {
                return Err("source".into());
            };
            source.fields[2] = NdfValue::Text("改".into());
        }
        // Rebind the owner digest so that the intended source validation, not
        // an unresolved owner reference, rejects this schema-shaped input.
        let mut bytes = b"NEPL3.Doc.Owner.v1\0".to_vec();
        bytes.extend(nepl3_wire::encode(owner, &mut b()).map_err(err)?);
        let digest = Digest::of(&bytes);
        for index in 0..2 {
            closure_fields_at(&mut bad, index)?[0] = NdfValue::Bytes(digest.0.to_vec());
        }
        let mut incoming = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(&r, &sources, &mut incoming).map_err(err)?;
        let expected = if duplicate {
            nepl3_core::source::SourceError::IdentityConflict
        } else {
            nepl3_core::source::SourceError::ExpectedDigest
        };
        assert_eq!(
            portable::from_value(&bad, &r, &mut receiver, &mut b()),
            Err(portable::PortableError::Foundation(
                nepl3_wire::WireError::Source(expected)
            ))
        );
    }
    Ok(())
}

#[test]
fn owner_table_order_is_canonical_and_malformed_references_fail() -> Result<(), String> {
    let r = registry()?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    let d = pair(&r, true, true)?;
    let mut value = portable::to_value(&d, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(owner_table(&mut value)?.len(), 2);
    let mut reordered = d.clone();
    reordered.value.embeds.swap(0, 1);
    let mut other = portable::to_value(&reordered, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(owner_table(&mut value)?, owner_table(&mut other)?);
    assert_ne!(value, other); // Embed order remains semantic data.
    assert_eq!(
        portable::from_value(&value, &r, &mut c, &mut b()).map_err(err)?,
        d
    );
    for case in 0..7 {
        let mut bad = value.clone();
        match case {
            0 => {
                let owners = owner_table(&mut bad)?;
                owners.push(owners[0].clone());
            }
            1 => owner_table(&mut bad)?.reverse(),
            2 => {
                owner_table(&mut bad)?.clear();
            }
            3 => {
                closure_fields(&mut bad)?[0] = NdfValue::Bytes(vec![0; 32]);
            }
            4 => {
                closure_fields(&mut bad)?[0] = NdfValue::Bytes(vec![0; 31]);
            }
            5 => {
                closure_fields(&mut bad)?[0] = NdfValue::Bytes(vec![0; 33]);
            }
            _ => {
                let first = closure_fields(&mut bad)?[0].clone();
                // Both references resolve, but the second owner is unused.
                closure_fields_at(&mut bad, 1)?[0] = first;
            }
        }
        assert!(
            portable::from_value(&bad, &r, &mut c, &mut b()).is_err(),
            "case={case}"
        );
    }
    Ok(())
}
