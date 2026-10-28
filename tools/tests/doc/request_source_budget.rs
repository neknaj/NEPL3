//! The largest retained annotation corpus uses the selected canonical policy.
//! Request-source lookup now charges previously unmetered union work. The old
//! 700M policy stopped in final tree validation; the explicit 720M registry
//! policy is selected before parsing. User defaults and allocation caps stay
//! unchanged, and this is accounting compatibility, not a performance claim.
use super::*;

#[test]
fn canonical_annotation_source_respects_selected_parse_policy() -> Result<(), String> {
    let compiled = compiled()?;
    let registry: nepl3_tools::doc::canonical::Registry =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    let page = registry
        .pages
        .iter()
        .find(|page| page.source == "doc/spec/23-sentence-annotation.nepld")
        .ok_or("annotation source registration")?;
    let limits = registry.parse_limits.budget().limits();
    let mut observed = None;
    let mut completed = false;
    nepl3_tools::doc::source::with_named_input_limits_observed(
        true,
        &compiled,
        include_str!("../../../doc/spec/23-sentence-annotation.nepld"),
        &page.id,
        "Article",
        limits,
        |observation| {
            eprintln!("canonical annotation parse: {observation:?}");
            observed = Some(observation);
        },
        |_, _, _, _| {
            completed = true;
            Ok(())
        },
    )?;
    let observation = observed.ok_or("missing terminal observation")?;
    assert!(completed);
    assert_eq!(
        observation.outcome,
        nepl3_tools::doc::source::ParseObservationOutcome::Complete
    );
    assert_eq!(observation.limits, limits);
    assert!(observation.profile.is_some());
    assert!(observation.usage.work <= limits.work);
    assert!(observation.usage.allocation_units <= limits.allocation_units);
    Ok(())
}
