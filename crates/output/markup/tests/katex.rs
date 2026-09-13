use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::katex::computed_style;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000,
        ..Limits::default()
    })
}

#[test]
fn current_layout_values_without_allocation() -> Result<(), StopReason> {
    for value in [
        "",
        "height:0.7em;vertical-align:-0.2em;",
        "position:relative;top:.2em",
        "width:100%;min-width:1.056em;padding-left:0em",
        "border-bottom-width:0.04em;margin-right:-0.1667em;",
        "height:1000000em",
        "top:-1000000.0000em",
        "left:0",
    ] {
        assert!(computed_style(value, &mut budget())?, "{value}");
    }
    Ok(())
}

#[test]
fn reject_unsafe_ambiguous_and_out_of_profile_values() -> Result<(), StopReason> {
    for value in [
        "height:1em;height:2em",
        "height:1em;;",
        ";",
        "HEIGHT:1em",
        "height: 1em",
        "height:calc(1em)",
        "height:var(--x)",
        "width:url(https://example.test)",
        "background:url(javascript:alert(1))",
        "--x:1em",
        "position:fixed",
        "height:1em!important",
        "height:1em/*x*/",
        "he\\69ght:1em",
        "height:NaNem",
        "height:Infinityem",
        "height:1e5em",
        "height:1000001em",
        "height:1000000.0001em",
        "height:-1em",
        "padding-left:-.1em",
        "height:+1em",
        "height:1.em",
        "height:em",
        "height:.em",
        "height:.00001em",
        "height:1px",
        "width:99%",
        "color:red",
        "height:1em\n",
        "height:1em:2em",
        "height:\0em",
    ] {
        assert!(!computed_style(value, &mut budget())?, "{value:?}");
    }
    Ok(())
}

#[test]
fn stop_is_preserved_even_for_empty_or_invalid_input() {
    let mut limited = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    });
    assert_eq!(computed_style("", &mut limited), Err(StopReason::WorkLimit));
    assert_eq!(
        computed_style("height:1em", &mut limited),
        Err(StopReason::WorkLimit)
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        computed_style("bad", &mut cancelled),
        Err(StopReason::Cancelled)
    );
}

#[test]
fn work_boundary_matches_the_declared_cost() {
    // Ten input bytes use the documented 4n+1 logical work units.
    let mut exact = Budget::new(Limits {
        work: 41,
        ..Limits::default()
    });
    assert_eq!(computed_style("height:1em", &mut exact), Ok(true));
    let mut short = Budget::new(Limits {
        work: 40,
        ..Limits::default()
    });
    assert_eq!(
        computed_style("height:1em", &mut short),
        Err(StopReason::WorkLimit)
    );
}
