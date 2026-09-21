use super::*;
use nepl3_core::{
    source::SourceStore,
    syntax::{FieldValue, NodeRef},
    value::{Integer, NdfValue},
};
use nepl3_engine::portable;
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn prefix_shape_payloads_and_source_print_are_observable() -> Result<(), String> {
    let observation = inspect("add 1 mul 2 3", true)?;
    assert!(observation.parse.report.diagnostics.is_empty());
    let ParseOutcome::Complete { tree, cursor, .. } = observation.parse.outcome else {
        return Err("complete expression required".into());
    };
    assert_eq!(cursor, 13);
    assert_eq!(
        tree.bundle
            .nodes
            .iter()
            .map(|node| node.kind.as_str())
            .collect::<Vec<_>>(),
        ["Add", "Natural", "Mul", "Natural", "Natural"]
    );
    // Prefix arities give Add(1, Mul(2, 3)), independently of printer output.
    assert_eq!(tree.bundle.root, NodeRef(0));
    assert_eq!(
        tree.bundle.nodes[0].fields,
        [FieldValue::Child(NodeRef(1)), FieldValue::Child(NodeRef(2))]
    );
    assert_eq!(
        tree.bundle.nodes[2].fields,
        [FieldValue::Child(NodeRef(3)), FieldValue::Child(NodeRef(4))]
    );
    for (token_index, value, range) in [(1, 1_i64, (4, 5)), (3, 2, (10, 11)), (4, 3, (12, 13))] {
        let token = &tree.bundle.tokens[token_index];
        assert_eq!(token.payload, NdfValue::Integer(Integer::from(value)));
        assert_eq!((token.head.start(), token.head.end()), range);
    }
    assert_eq!(
        observation.printed.ok_or("print")?.outcome,
        print::PrintOutcome::Complete("add 1 mul 2 3".into())
    );
    for input in ["7", "neg 7", "neg add 1 2", " \tadd 1\r\nmul 2 3"] {
        let result = inspect(input, true)?;
        assert!(matches!(
            result.parse.outcome,
            ParseOutcome::Complete { .. }
        ));
        assert_eq!(
            result.printed.ok_or("print")?.outcome,
            print::PrintOutcome::Complete(input.into())
        );
    }
    Ok(())
}

#[test]
fn invalid_and_incomplete_inputs_keep_their_outcomes() -> Result<(), String> {
    for input in ["add 1", "unknown 1", "add 1 unknown"] {
        let result = inspect(input, true)?;
        assert!(matches!(
            result.parse.outcome,
            ParseOutcome::Recovered { .. }
        ));
        assert!(!result.parse.report.diagnostics.is_empty());
        assert!(result.printed.is_none());
    }
    let unfinished = inspect("add 1 ", false)?;
    assert!(matches!(
        unfinished.parse.outcome,
        ParseOutcome::NeedMore { .. }
    ));
    assert!(unfinished.printed.is_none());
    super::super::parse::with_language(
        "add 1 2",
        true,
        true,
        language()?,
        ("MiniExpr", "miniexpr"),
        |reply, _, _, _| {
            assert!(matches!(reply.outcome, ParseOutcome::Stopped { .. }));
            Ok(())
        },
    )
}

#[test]
fn checked_tree_survives_portable_exchange_and_rejects_corruption() -> Result<(), String> {
    super::super::parse::with_language(
        "add 1 mul 2 3",
        true,
        false,
        language()?,
        ("MiniExpr", "miniexpr"),
        |reply, resolved, registry, codec| {
            let ParseOutcome::Complete { tree, .. } = reply.outcome else {
                return Err("complete".into());
            };
            let mut transfer = budget();
            let value =
                portable::tree::to_value(&tree, resolved, codec, &mut transfer).map_err(error)?;
            let bytes = nepl3_wire::encode(&value, &mut transfer).map_err(error)?;
            let decoded = nepl3_wire::decode(&bytes, &mut transfer).map_err(error)?;
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(registry, &sources, &mut admission).map_err(error)?;
            let restored =
                portable::tree::from_value(&decoded, resolved, &mut receiver, &mut transfer)
                    .map_err(error)?;
            assert_eq!(
                portable::tree::to_value(&restored, resolved, &mut receiver, &mut transfer)
                    .map_err(error)?,
                value
            );
            assert_eq!(tree.bundle.tokens, restored.bundle.tokens);
            let proof = restored
                .validate(resolved, &mut budget(), &mut SourceAdmission::default())
                .map_err(error)?;
            let mut limits = budget().limits();
            limits.output_bytes = 0;
            let printed = print::source_tree(
                &proof,
                &mut Budget::new(limits),
                &mut SourceAdmission::default(),
            )
            .map_err(error)?;
            assert!(matches!(
                printed.outcome,
                print::PrintOutcome::Stopped {
                    reason: nepl3_core::budget::StopReason::OutputLimit,
                    ..
                }
            ));
            let mut corrupt = tree.clone();
            corrupt.bundle.nodes[0].fields.clear();
            assert!(
                corrupt
                    .validate(resolved, &mut budget(), &mut SourceAdmission::default())
                    .is_err()
            );
            assert!(
                portable::tree::from_value(&NdfValue::Unit, resolved, &mut receiver, &mut transfer)
                    .is_err()
            );
            Ok(())
        },
    )
}

#[test]
fn changed_language_definition_changes_the_accepted_head() -> Result<(), String> {
    let (mut package, registry) = language()?;
    package
        .forms
        .iter_mut()
        .find(|form| form.spelling == "add")
        .ok_or("add")?
        .spelling = "sum".into();
    super::super::parse::with_language(
        "sum 1 2",
        true,
        false,
        (package, registry),
        ("MiniExpr", "miniexpr"),
        |reply, resolved, _, _| {
            let ParseOutcome::Complete { tree, .. } = reply.outcome else {
                return Err("complete".into());
            };
            let proof = tree
                .validate(resolved, &mut budget(), &mut SourceAdmission::default())
                .map_err(error)?;
            assert_eq!(
                print::source_tree(&proof, &mut budget(), &mut SourceAdmission::default())
                    .map_err(error)?
                    .outcome,
                print::PrintOutcome::Complete("sum 1 2".into())
            );
            Ok(())
        },
    )
}
