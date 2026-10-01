use super::*;
use nepl3_core::syntax::{FieldValue, NodeRef};
use nepl3_engine::recovery::RecoveryKind;

fn share(tree: &mut ParseTree) -> Result<(), String> {
    let entries = &tree.recovery.first().ok_or("recovery")?.entries;
    if entries.len() != 2 {
        return Err("two missing nodes".into());
    }
    let (a, d) = (&entries[0], &entries[1]);
    let (
        RecoveryKind::Missing {
            expected: ae,
            anchor: aa,
        },
        RecoveryKind::Missing {
            expected: de,
            anchor: da,
        },
    ) = (&a.kind, &d.kind)
    else {
        return Err("missing".into());
    };
    assert_eq!(ae, de);
    assert_eq!(aa, da);
    let keep = a.node;
    let remove = d.node;
    assert!(keep.0 < remove.0);
    let remap = |node: &mut NodeRef| {
        if *node == remove {
            *node = keep;
        } else if node.0 > remove.0 {
            node.0 -= 1;
        }
    };
    tree.bundle.nodes.remove(remove.0 as usize);
    remap(&mut tree.bundle.root);
    for node in &mut tree.bundle.nodes {
        for field in &mut node.fields {
            if let FieldValue::Child(child) = field {
                remap(child);
            }
        }
    }
    for context in &mut tree.contexts {
        assert!(context.path.is_empty());
        context.nodes.retain(|n| n.node != remove);
        for node in &mut context.nodes {
            remap(&mut node.node);
        }
    }
    for recovery in &mut tree.recovery {
        assert!(recovery.path.is_empty());
        recovery.entries.retain(|e| e.node != remove);
        for entry in &mut recovery.entries {
            remap(&mut entry.node);
        }
    }
    Ok(())
}
fn permute(tree: &mut ParseTree) -> Result<(), String> {
    let len = tree.bundle.nodes.len() as u64;
    let remap = |node: &mut NodeRef| {
        node.0 = len - 1 - node.0;
    };
    tree.bundle.nodes.reverse();
    remap(&mut tree.bundle.root);
    for node in &mut tree.bundle.nodes {
        for field in &mut node.fields {
            if let FieldValue::Child(child) = field {
                remap(child);
            }
        }
    }
    for context in &mut tree.contexts {
        assert!(context.path.is_empty());
        context.nodes.reverse();
        for node in &mut context.nodes {
            remap(&mut node.node);
        }
    }
    for recovery in &mut tree.recovery {
        assert!(recovery.path.is_empty());
        recovery.entries.reverse();
        for entry in &mut recovery.entries {
            remap(&mut entry.node);
        }
    }
    Ok(())
}
#[test]
fn shared_missing_uses_incoming_field_and_canonical_storage_independent_identity()
-> Result<(), String> {
    let mut outcomes = Vec::new();
    for reverse in [false, true] {
        let result = with_input(
            "lambda",
            |tree| {
                share(tree)?;
                if reverse {
                    permute(tree)?;
                }
                Ok(())
            },
            |input, source, _| {
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: 6,
                };
                let result = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let ExpectedReadOutcome::Complete(Some(value)) = &result.outcome else {
                    return Err(err(result));
                };
                assert_eq!(value.path, vec![ExpectedReadStep::Child { field: 0 }]);
                assert!(matches!(
                    value.origin,
                    ExpectedReadOrigin::Field {
                        field: 0,
                        resolved_read: Some(_),
                        ..
                    }
                ));
                let no_match = expected_read(
                    input,
                    &ExpectedReadRequest {
                        offset: 0,
                        ..request
                    },
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                assert_eq!(no_match.outcome, ExpectedReadOutcome::Complete(None));
                // Root plus both incoming occurrences of the one shared Missing.
                assert_eq!(no_match.report.usage.nodes, 3);
                Ok((result.key, result.outcome, result.sources))
            },
        )?;
        outcomes.push(result);
    }
    assert_eq!(outcomes[0], outcomes[1]);
    Ok(())
}

#[test]
fn no_match_returns_sorted_full_source_closure_and_rejects_admission_conflict() -> Result<(), String>
{
    for reverse in [false, true] {
        with_input(
            "x",
            |tree| {
                for id in ["z-extra", "a-extra"] {
                    tree.bundle.sources.push(
                        SourceSnapshot::new(
                            SourceId(id.into()),
                            0,
                            format!("memory:{id}"),
                            b"unused".to_vec(),
                            &mut budget(),
                        )
                        .map_err(err)?,
                    );
                }
                if reverse {
                    tree.bundle.sources.reverse();
                }
                Ok(())
            },
            |input, source, _| {
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: 1,
                };
                let reply = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                assert_eq!(reply.outcome, ExpectedReadOutcome::Complete(None));
                assert_eq!(
                    reply
                        .sources
                        .iter()
                        .map(|s| s.identity().source.0.as_str())
                        .collect::<Vec<_>>(),
                    vec!["a-extra", "expected-input", "z-extra"]
                );
                let conflict = SourceSnapshot::new(
                    source.identity().source.clone(),
                    source.identity().revision,
                    "memory:other-locator".into(),
                    source.text().as_bytes().to_vec(),
                    &mut budget(),
                )
                .map_err(err)?;
                let mut admission = SourceAdmission::default();
                admission
                    .admit_existing(&conflict, &mut budget())
                    .map_err(err)?;
                let reply = expected_read(input, &request, &mut budget(), &mut admission);
                assert_eq!(
                    reply.outcome,
                    ExpectedReadOutcome::Invalid(ExpectedReadError::Source(
                        SourceError::IdentityConflict
                    ))
                );
                assert!(reply.sources.is_empty());
                Ok(())
            },
        )?;
    }
    Ok(())
}
