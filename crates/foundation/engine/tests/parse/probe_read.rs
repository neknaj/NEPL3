use super::*;
use nepl3_core::budget::Budget;
use nepl3_engine::{
    analysis::{
        BindingOptions,
        expected::ExpectedReadRequest,
        probe::read::{ReadError, ReadOutcome, correlate},
    },
    package::{Binding, BindingId, NameSelector, ReadSpecId},
    portable::analysis,
};

#[test]
fn probe_read_joins_unique_targets_and_rejects_anchor_or_shared_node_aliases() -> TestResult {
    for (input, selector, shared, repeated) in [
        ("let x", "body", false, false),
        ("let x", "body", false, true),
        ("let", "body", false, false),
        ("let", "name", true, false),
        ("let", "body", true, false),
    ] {
        retained::with_edited_package(
            input,
            |p| {
                p.forms[0].fields[1].read = ReadSpecId(0);
                p.bindings[1] = Binding::Reference {
                    namespace: "Value".into(),
                    name: NameSelector::Field(selector.into()),
                };
                p.bindings[2] = Binding::Group(if repeated {
                    vec![BindingId(1), BindingId(1)]
                } else {
                    vec![BindingId(1)]
                });
            },
            |profile, environments, sources, source, entry| {
                let states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                }];
                let mut b = budget();
                let mut a = SourceAdmission::default();
                let mut session = RetainedParseSession::new(
                    "join".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: input.len() as u64,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let RetainedParseExecution::Continue(parsed) =
                    session.read(&mut b, &mut a).map_err(|e| format!("{e:?}"))?
                else {
                    return Err("parse".into());
                };
                let mut tree = parsed.execution().tree().clone();
                if shared {
                    expected::share_missing(&mut tree)?;
                }
                let empty = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a)
                    .map_err(|e| format!("{e:?}"))?;
                let limits = b.limits();
                let prepared = analysis::prepare(
                    "join",
                    &tree,
                    BindingOptions,
                    limits,
                    profile,
                    &mut codec,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let bound = prepared
                    .probe_missing_reference(
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default(),
                    )
                    .map_err(|e| format!("{e:?}"))?;
                let original = bound.reply().report.clone();
                let request = ExpectedReadRequest {
                    key: bound.key(),
                    source: source.reference(),
                    offset: input.len() as u64,
                };
                let result = correlate(
                    &bound,
                    &prepared,
                    &request,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                );
                if shared && selector == "name" {
                    assert!(matches!(result, Err(ReadError::AmbiguousTarget)));
                } else if input == "let" {
                    assert!(matches!(result, Err(ReadError::DifferentTarget)));
                } else {
                    let result = result.map_err(|e| format!("{e:?}"))?;
                    let ReadOutcome::Hit(proof) = result.outcome() else {
                        return Err("hit proof".into());
                    };
                    assert_eq!(proof.expected().path.len(), 1);
                    assert!(core::ptr::eq(proof.reply(), &bound));
                    assert_eq!(proof.reply().key(), prepared.key());
                    assert_eq!(proof.limits(), limits);
                    assert_eq!(proof.hit().site().execution_step, 2);
                    let separately_allocated = tree.clone();
                    let other = analysis::prepare(
                        "join",
                        &separately_allocated,
                        BindingOptions,
                        limits,
                        profile,
                        &mut codec,
                        &mut b,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    assert_eq!(other.key(), prepared.key());
                    assert!(matches!(
                        correlate(
                            &bound,
                            &other,
                            &request,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(ReadError::ProofMismatch)
                    ));
                }
                assert_eq!(bound.reply().report, original);
                Ok(())
            },
        )?;
    }
    Ok(())
}
