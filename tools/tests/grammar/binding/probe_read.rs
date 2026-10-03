use super::*;
use nepl3_core::syntax::{FieldValue, NodeRef};
use nepl3_engine::{
    analysis::{
        BindingOptions,
        expected::ExpectedReadRequest,
        probe::read::{ReadError, correlate},
    },
    portable::analysis as keyed,
};

#[test]
fn probe_read_rejects_two_paths_through_a_shared_ancestor() -> Result<(), String> {
    let compiled = super::missing_probe::named_lambda()?;
    with_source_profile(
        &compiled,
        None,
        "apply lambda x",
        |source, profile, b, a| {
            let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
            let ParseCompletion::Break(ParseReply {
                outcome: ParseOutcome::Recovered { mut tree, .. },
                ..
            }) = parsed
            else {
                return Err("recovered".into());
            };
            let root = tree
                .bundle
                .nodes
                .get(tree.bundle.root.0 as usize)
                .ok_or("root")?;
            let (Some(FieldValue::Child(keep)), Some(FieldValue::Child(remove))) =
                (root.fields.first(), root.fields.get(1))
            else {
                return Err("apply children".into());
            };
            let (keep, remove) = (*keep, *remove);
            let remap = |node: &mut NodeRef| {
                if *node == remove {
                    *node = keep;
                }
                if node.0 > remove.0 {
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
                context.nodes.retain(|s| s.node != remove);
                for selected in &mut context.nodes {
                    remap(&mut selected.node);
                }
            }
            for recovery in &mut tree.recovery {
                assert!(recovery.path.is_empty());
                recovery.entries.retain(|e| e.node != remove);
                for entry in &mut recovery.entries {
                    remap(&mut entry.node);
                }
            }
            // A source-less enclosing application permits a shared positive-width
            // child in the syntax model. Keep each referenced subtree unchanged.
            let root = tree
                .bundle
                .nodes
                .get_mut(tree.bundle.root.0 as usize)
                .ok_or("root")?;
            root.head = None;
            root.cover = None;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let limits = b.limits();
            // Preparation validates that sharing an ancestor is legal input, not a
            // malformed tree rejected before the new uniqueness check.
            let prepared = keyed::prepare(
                "shared-ancestor",
                &tree,
                BindingOptions,
                limits,
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let bound = prepared
                .probe_missing_reference(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            let request = ExpectedReadRequest {
                key: bound.key(),
                source: source.reference(),
                offset: "apply lambda x".len() as u64,
            };
            assert!(matches!(
                correlate(
                    &bound,
                    &prepared,
                    &request,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default()
                ),
                Err(ReadError::AmbiguousTarget)
            ));
            Ok(())
        },
    )
}
