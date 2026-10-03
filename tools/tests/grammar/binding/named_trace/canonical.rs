use super::*;

#[test]
fn birth_targets_keep_self_field_and_foreign_canonical_identity() -> Result<(), String> {
    let mut compiled = execution()?;
    let leaf = compiled
        .package
        .leaves
        .iter()
        .find(|v| matches!(v.payload, nepl3_core::schema::TypeDescriptor::Text))
        .ok_or("name leaf")?;
    compiled.package.bindings[leaf.binding.0 as usize] = nepl3_engine::package::Binding::Bind {
        namespace: "Value".into(),
        name: nepl3_engine::package::NameSelector::SelfValue,
    };
    with_input(&compiled, "x", |tree, profile, _, _| {
        let trace = named::analyze(
            "self",
            tree,
            profile,
            None,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(trace.references().complete().is_some());
        assert_eq!(trace.births().len(), 1);
        assert_eq!(trace.births()[0].owner, trace.births()[0].name_target);
        assert_eq!(trace.births()[0].kind, BirthKind::Bind);
        Ok(())
    })?;
    let compiled = execution()?;
    with_input(
        &compiled,
        "lambda outer guest lambda inner inner",
        |tree, profile, _, _| {
            let original = named::analyze(
                "canonical",
                tree,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let encoded = nepl3_engine::portable::tree::to_value(
                tree.tree(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&encoded, &mut budget()).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let mut received = nepl3_engine::portable::tree::from_value(
                &value,
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let count = received.bundle.nodes.len() as u64;
            let remap = |node: &mut nepl3_core::syntax::NodeRef| {
                node.0 = count - 1 - node.0;
            };
            received.bundle.nodes.reverse();
            remap(&mut received.bundle.root);
            for node in &mut received.bundle.nodes {
                for field in &mut node.fields {
                    match field {
                        nepl3_core::syntax::FieldValue::Child(child) => remap(child),
                        nepl3_core::syntax::FieldValue::Children(children) => {
                            for child in children {
                                remap(child);
                            }
                        }
                        _ => {}
                    }
                }
            }
            for context in &mut received.contexts {
                if let Some(step) = context.path.first_mut() {
                    remap(&mut step.node);
                } else {
                    for selected in &mut context.nodes {
                        remap(&mut selected.node);
                    }
                }
            }
            assert!(received.recovery.is_empty());
            let checked = received
                .validate(profile, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            assert_ne!(tree.tree().bundle.root, checked.tree().bundle.root);
            let decoded = named::analyze(
                "canonical",
                &checked,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(original.references().complete().is_some());
            assert!(decoded.references().complete().is_some());
            assert!(!core::ptr::eq(
                original.references().tree(),
                decoded.references().tree()
            ));
            assert_eq!(original.births().len(), 2);
            assert_eq!(decoded.births().len(), 2);
            for (left, right) in original.births().iter().zip(decoded.births()) {
                assert_eq!(left.owner, right.owner);
                assert_eq!(left.name_target, right.name_target);
                assert_eq!(left.package, right.package);
                assert_eq!(left.execution_digest, right.execution_digest);
                assert_eq!(left.binding, right.binding);
                assert_eq!(left.kind, right.kind);
            }
            assert!(original.births()[0].owner.path.is_empty());
            assert_eq!(original.births()[1].owner.path.len(), 1);
            Ok(())
        },
    )
}
