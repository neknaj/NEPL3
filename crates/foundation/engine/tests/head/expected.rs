use super::*;
use nepl3_engine::{
    analysis::{BindingOptions, expected::*},
    portable::analysis,
    recovery::{BundleRecovery, RecoveryEntry, RecoveryKind},
};

#[test]
fn admitted_dynamic_foreign_missing_retains_saved_guest_context() -> TestResult {
    run_case_inspect(
        "choose alt @let z x tail",
        Case::Foreign,
        false,
        |reply, profile, source| {
            let ParseOutcome::Complete { tree, .. } = &reply.outcome else {
                return Err("complete".into());
            };
            let mut tree = tree.clone();
            // This is deliberately validator-admitted input, not a claim that the
            // fixture provider executes with a Missing completed selector.
            let guest_context = tree
                .contexts
                .iter_mut()
                .find(|c| !c.path.is_empty())
                .ok_or("guest context")?;
            let path = guest_context.path.clone();
            let selected = guest_context
                .nodes
                .first_mut()
                .ok_or("guest root selection")?;
            selected.entry.category = "Expr".into();
            selected.shape = ShapeSelection::Recovery;
            let expected = selected.entry.clone();
            let guest_node = selected.node;
            let host_context = tree
                .contexts
                .iter_mut()
                .find(|c| c.path.is_empty())
                .ok_or("host context")?;
            let selected = host_context
                .nodes
                .iter_mut()
                .find(|n| n.node == tree.bundle.root)
                .ok_or("host root")?;
            let ShapeSelection::Dynamic { child_contexts, .. } = &mut selected.shape else {
                return Err("dynamic".into());
            };
            child_contexts[0] = expected.clone();
            let FieldValue::Foreign(guest) =
                &mut tree.bundle.nodes[tree.bundle.root.0 as usize].fields[0]
            else {
                return Err("foreign".into());
            };
            let schema = profile
                .registry()
                .selected("nepl3.engine", 1)
                .ok_or("engine schema")?
                .clone();
            guest.schema = schema.clone();
            guest.category = expected.category.clone();
            let root = &mut guest.bundle.nodes[guest_node.0 as usize];
            let anchor = source.span(10, 10).map_err(|e| format!("{e:?}"))?;
            root.schema = schema;
            root.kind = "RecoveryMissing".into();
            root.fields.clear();
            root.head = None;
            root.token = None;
            root.cover = Some(anchor.clone());
            tree.recovery.push(BundleRecovery {
                path,
                entries: vec![RecoveryEntry {
                    node: guest_node,
                    kind: RecoveryKind::Missing {
                        expected: expected.clone(),
                        anchor,
                    },
                }],
            });
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
            let input = analysis::prepare(
                "expected-dynamic-foreign",
                &tree,
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let result = expected_read(
                &input,
                &ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: 10,
                },
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let ExpectedReadOutcome::Complete(Some(value)) = result.outcome else {
                return Err(format!("{result:?}"));
            };
            assert_eq!(value.path, vec![ExpectedReadStep::Foreign { field: 0 }]);
            assert_eq!(value.expected, expected);
            let ExpectedReadOrigin::Field {
                declared,
                foreign,
                resolved_read,
                ..
            } = value.origin
            else {
                return Err("origin".into());
            };
            assert!(foreign);
            assert_eq!(resolved_read, None);
            let package = profile
                .language("Host", &mut budget())
                .map_err(|e| format!("{e:?}"))?;
            assert!(
                matches!(package.read(declared), Ok(ReadSpec::Foreign { category, .. }) if category == "Selector")
            );
            assert_eq!(value.expected.category, "Expr");
            Ok(())
        },
    )?;
    Ok(())
}
