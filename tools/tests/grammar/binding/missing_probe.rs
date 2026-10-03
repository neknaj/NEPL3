use super::*;
use nepl3_engine::{
    binding::probe::{MissingReference, ProbeOutcome, first_missing_reference},
    package::{Binding, BindingId, NameSelector},
};

pub(super) fn named_lambda() -> Result<CompiledLanguage, String> {
    let mut compiled = execution()?;
    let lambda = compiled
        .package
        .forms
        .iter_mut()
        .find(|f| f.spelling == "lambda")
        .ok_or("lambda")?;
    lambda.fields[1].read = lambda.fields[0].read;
    let binding = lambda.binding;
    let Binding::Scope(mut actions) = compiled.package.bindings[binding.0 as usize].clone() else {
        return Err("lambda scope".into());
    };
    let id = BindingId(compiled.package.bindings.len() as u64);
    compiled.package.bindings.push(Binding::Reference {
        namespace: "Value".into(),
        name: NameSelector::Field("body".into()),
    });
    *actions.last_mut().ok_or("body action")? = id;
    compiled.package.bindings[binding.0 as usize] = Binding::Scope(actions);
    Ok(compiled)
}
fn introduced_on_chain(hit: &MissingReference) -> Result<usize, String> {
    let mut at = Some(hit.site().namespace_stage);
    let mut count = 0;
    let mut remaining = hit.stages().len();
    while let Some(id) = at {
        if remaining == 0 {
            return Err("stage cycle".into());
        }
        remaining -= 1;
        let stage = hit.stages().get(id.0 as usize).ok_or("stage")?;
        count += stage.introduced.len();
        at = stage.previous;
    }
    Ok(count)
}
#[test]
fn missing_reference_prefix_preserves_ordered_headers_and_foreign_isolation() -> Result<(), String>
{
    let compiled = named_lambda()?;
    for (input, expected, foreign) in [
        ("sequence cons define a b nil lambda x", 2, false),
        (
            "recursive cons define a b cons define b a nil lambda x",
            3,
            false,
        ),
        ("repeat cons define a 1 nil lambda x", 2, false),
        ("let outer 1 guest lambda inner", 1, true),
    ] {
        with_source_profile(&compiled, None, input, |source, profile, b, a| {
            let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
            let ParseCompletion::Break(ParseReply {
                outcome: ParseOutcome::Recovered { tree, .. },
                ..
            }) = parsed
            else {
                return Err(format!("expected recovery: {input}"));
            };
            let checked = tree.validate(profile, b, a).map_err(err)?;
            let reply = first_missing_reference("probe", &checked, profile, b, a);
            let ProbeOutcome::Hit(hit) = &reply.outcome else {
                return Err(format!("probe {input}: {:?}", reply.outcome));
            };
            assert_eq!(introduced_on_chain(hit)?, expected, "{input}");
            assert_eq!(!hit.site().owner.path.is_empty(), foreign);
            assert_eq!(hit.site().anchor.start(), input.len() as u64);
            assert_eq!(reply.report.usage, b.usage());
            if input.starts_with("sequence") {
                assert!(!reply.report.diagnostics.is_empty());
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn unfinished_declaration_headers_block_before_a_missing_body_can_be_reported() -> Result<(), String>
{
    let compiled = named_lambda()?;
    for input in ["sequence cons define", "recursive cons define"] {
        with_source_profile(&compiled, None, input, |source, profile, b, a| {
            let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
            let ParseCompletion::Break(ParseReply {
                outcome: ParseOutcome::Recovered { tree, .. },
                ..
            }) = parsed
            else {
                return Err("recovered declarations".into());
            };
            let checked = tree.validate(profile, b, a).map_err(err)?;
            assert!(matches!(
                first_missing_reference("blocked-header", &checked, profile, b, a).outcome,
                ProbeOutcome::Blocked(_)
            ));
            Ok(())
        })?;
    }
    Ok(())
}
