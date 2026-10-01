//! A recovered node is not an incoming read slot: valid syntax can share it.
use nepl3_core::syntax::{FieldValue, NodeRef};
use nepl3_engine::recovery::{ParseTree, RecoveryKind};

pub(super) fn share_missing(tree: &mut ParseTree) -> Result<(), String> {
    let recovery = tree.recovery.first().ok_or("recovery")?;
    if recovery.entries.len() != 2 {
        return Err("two missing slots".into());
    }
    let a = &recovery.entries[0];
    let b = &recovery.entries[1];
    let (
        RecoveryKind::Missing {
            expected: ae,
            anchor: aa,
        },
        RecoveryKind::Missing {
            expected: be,
            anchor: ba,
        },
    ) = (&a.kind, &b.kind)
    else {
        return Err("missing slots".into());
    };
    assert_eq!(ae, be);
    assert_eq!(aa, ba);
    assert_eq!(aa.start(), aa.end());
    let keep = a.node;
    let remove = b.node;
    assert!(keep.0 < remove.0);
    let remap = |node: &mut NodeRef| {
        if *node == remove {
            *node = keep;
        } else if node.0 > remove.0 {
            node.0 -= 1;
        }
    };
    let removed = usize::try_from(remove.0).map_err(|e| e.to_string())?;
    if removed >= tree.bundle.nodes.len() {
        return Err("node index".into());
    }
    tree.bundle.nodes.remove(removed);
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
        context.nodes.retain(|entry| entry.node != remove);
        for entry in &mut context.nodes {
            remap(&mut entry.node);
        }
    }
    for recovery in &mut tree.recovery {
        assert!(recovery.path.is_empty());
        recovery.entries.retain(|entry| entry.node != remove);
        for entry in &mut recovery.entries {
            remap(&mut entry.node);
        }
    }
    Ok(())
}

#[test]
fn shared_missing_node_has_distinct_incoming_read_slots() -> Result<(), String> {
    // run_scenario validates the mutated tree using the original resolved profile.
    let reply = super::run_scenario(
        "let",
        true,
        super::Scenario {
            share_missing: true,
            ..super::Scenario::default()
        },
    )?;
    let super::ParseOutcome::Recovered { tree, .. } = reply.outcome else {
        return Err("recovered tree".into());
    };
    let root = tree
        .bundle
        .node(tree.bundle.root)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(root.fields[0], root.fields[1]);
    let (package, _) = super::support::fixture()?;
    assert_ne!(
        package.forms[0].fields[0].read,
        package.forms[0].fields[1].read
    );
    assert!(matches!(
        package.read(package.forms[0].fields[0].read),
        Ok(nepl3_engine::package::ReadSpec::Builtin {
            reader: nepl3_reader::builtin::BuiltinReader::Name,
            ..
        })
    ));
    assert!(
        matches!(package.read(package.forms[0].fields[1].read), Ok(nepl3_engine::package::ReadSpec::Local { category }) if category == "Expr")
    );
    assert_eq!(tree.recovery[0].entries.len(), 1);
    Ok(())
}
