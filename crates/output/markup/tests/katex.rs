use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::katex::{computed_style, path_data, view_box};

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

#[test]
fn svg_commands_repetition_and_arc_flags() -> Result<(), StopReason> {
    for path in [
        "M0 0",
        "m.5-.5 1 2z",
        "M0,0L1 2 3 4H5V6C1 2 3 4 5 6S1 2 3 4Q1 2 3 4T5 6Z",
        "M0 0 A30 50 0 01162.55 162.45",
        "M0 0a1 2 30 1 0 4 5z",
        "M1. 2.\nL+3 -4",
        "M1000000000 -1000000000.0000000000000000",
    ] {
        assert!(path_data(path, &mut budget())?, "{path}");
    }
    Ok(())
}

#[test]
fn malformed_svg_path_is_not_partially_accepted() -> Result<(), StopReason> {
    for path in [
        "",
        " ",
        "Z",
        "L0 0",
        "M",
        "M0",
        "M,0 0",
        "M0 0,",
        "M0 0,L1 1",
        "M0 0L",
        "M0 0L1",
        "M0 0Z1 2",
        "M0 0C1 2 3 4 5",
        "M0 0R1 2",
        "M0 0A-1 2 0 0 1 3 4",
        "M0 0A+1 2 0 0 1 3 4",
        "M0 0A1 +.5 0 0 1 3 4",
        "M0 0A1 2 0 2 1 3 4",
        "M0 0A1 2 0 0.0 1 3 4",
        "M0 0A1 2 0 1 -1 3 4",
        "M0 0A1 2 0 1 1 3",
        "MNaN 0",
        "M1e5 0",
        "M1000000001 0",
        "M1000000000.0000000000000001 0",
        "M. 0",
        "M0\u{c}0",
        "M0 0<script>",
        "M0 0\0",
        "M0 0,,1 2",
    ] {
        assert!(!path_data(path, &mut budget())?, "{path:?}");
    }
    Ok(())
}

#[test]
fn svg_work_and_cancellation_boundaries() {
    // Four bytes: 4n+1 = 17; path grammar scanning allocates nothing.
    let mut exact = Budget::new(Limits {
        work: 17,
        ..Limits::default()
    });
    assert_eq!(path_data("M0 0", &mut exact), Ok(true));
    let mut short = Budget::new(Limits {
        work: 16,
        ..Limits::default()
    });
    assert_eq!(path_data("M0 0", &mut short), Err(StopReason::WorkLimit));
    short.cancel();
    assert_eq!(path_data("", &mut short), Err(StopReason::WorkLimit));
}

#[test]
fn svg_viewports_have_exactly_four_finite_values_and_positive_dimensions() -> Result<(), StopReason>
{
    for value in ["0 0 400000 1080", "-1 -.5 1 .5", "0 0 1000000000 1"] {
        assert!(view_box(value, &mut budget())?, "{value}");
    }
    for value in [
        "",
        "0 0 1",
        "0 0 1 1 1",
        "0 0 0 1",
        "0 0 .000 1",
        "0 0 -1 1",
        "0 0 +1 1",
        "0 0 1 NaN",
        "0 0 1 1e5",
        "0,0,1,1",
        "0  0 1 1",
        " 0 0 1 1",
        "0 0 1 1 ",
        "0\t0 1 1",
        "0 0 1000000001 1",
    ] {
        assert!(!view_box(value, &mut budget())?, "{value:?}");
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(view_box("", &mut cancelled), Err(StopReason::Cancelled));
    Ok(())
}
