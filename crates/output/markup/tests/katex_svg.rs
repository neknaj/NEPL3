use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::katex::svg::{Aspect, Element, Endpoint, validate_attributes};

fn budget(work: u64) -> Budget {
    // No tree/allocation/output allowances: this is attribute-only validation.
    Budget::new(Limits {
        work,
        ..Limits::default()
    })
}
fn svg<'a>(
    width: &'a str,
    height: &'a str,
    view_box: Option<&'a str>,
    aspect: Option<Aspect>,
) -> Element<'a> {
    Element::Svg {
        width,
        height,
        view_box,
        aspect,
    }
}
fn line(stroke_width: &str) -> Element<'_> {
    Element::Line {
        x1: Endpoint::Zero,
        y1: Endpoint::Full,
        x2: Endpoint::Full,
        y2: Endpoint::Zero,
        stroke_width,
    }
}
#[test]
fn dimensions_are_the_existing_generated_profile_not_general_svg_lengths() -> Result<(), StopReason>
{
    for value in ["0", "0em", ".0001em", "1000000em", "1000000.0000em"] {
        assert!(validate_attributes(
            svg(value, value, None, None),
            &mut budget(1000)
        )?);
        assert!(validate_attributes(line(value), &mut budget(1000))?);
    }
    assert!(validate_attributes(
        svg("100%", "1em", None, None),
        &mut budget(1000)
    )?);
    for value in [
        "",
        "1",
        "1px",
        "1%",
        "-0em",
        "-1em",
        "+1em",
        "1e2em",
        "NaNem",
        "1.em",
        ".00001em",
        "1000000.0001em",
        "1000001em",
        "00000000em",
        "1em ",
        "url(x)",
        "1em\" onload=\"x",
        "１em",
    ] {
        assert!(
            !validate_attributes(svg(value, "1em", None, None), &mut budget(1000))?,
            "width {value:?}"
        );
        assert!(
            !validate_attributes(svg("1em", value, None, None), &mut budget(1000))?,
            "height {value:?}"
        );
        assert!(
            !validate_attributes(line(value), &mut budget(1000))?,
            "stroke {value:?}"
        );
    }
    assert!(!validate_attributes(
        svg("1em", "100%", None, None),
        &mut budget(1000)
    )?);
    assert!(!validate_attributes(line("100%"), &mut budget(1000))?);
    Ok(())
}
#[test]
fn aspect_requires_viewport_and_each_closed_value_is_accepted() -> Result<(), StopReason> {
    for aspect in [
        Aspect::None,
        Aspect::MinSlice,
        Aspect::MidSlice,
        Aspect::MaxSlice,
    ] {
        assert!(!validate_attributes(
            svg("1em", "1em", None, Some(aspect)),
            &mut budget(1000)
        )?);
        assert!(validate_attributes(
            svg("1em", "1em", Some("0 0 1 1"), Some(aspect)),
            &mut budget(1000)
        )?);
    }
    for viewport in ["0 0 0 1", "0 0 -1 1", "0,0,1,1", "0 0 1 1\"", "0 0 1 1e2"] {
        assert!(!validate_attributes(
            svg("1em", "1em", Some(viewport), None),
            &mut budget(1000)
        )?);
    }
    Ok(())
}
#[test]
fn paths_reuse_the_bounded_lexical_validator() -> Result<(), StopReason> {
    for (data, expected) in [
        ("M0 0L1 1Z", true),
        ("M0 0A1 1 0 0 1 2 2", true),
        ("", false),
        ("M0 0<script>", false),
        ("M0 0L", false),
        ("MNaN 0", false),
    ] {
        assert_eq!(
            validate_attributes(Element::Path { data }, &mut budget(1000))?,
            expected
        );
    }
    Ok(())
}
#[test]
fn exact_work_and_sticky_stops_cover_all_variants() -> Result<(), StopReason> {
    // One dispatch unit, then length bytes; viewport/path reuse 4n+1.
    for (element, work) in [
        (svg("100%", "1em", None, None), 8),
        (svg("1em", "1em", Some("0 0 1 1"), Some(Aspect::None)), 36),
        (line(".046em"), 7),
        (Element::Path { data: "M0 0" }, 18),
    ] {
        let mut exact = budget(work);
        assert!(validate_attributes(element, &mut exact)?);
        assert_eq!(exact.usage().work, work);
        assert_eq!(exact.usage().allocation_units, 0);
        assert_eq!(exact.usage().output_bytes, 0);
        assert_eq!(exact.usage().nodes, 0);
        let mut short = budget(work - 1);
        assert_eq!(
            validate_attributes(element, &mut short),
            Err(StopReason::WorkLimit)
        );
        let stopped_usage = short.usage();
        short.cancel();
        assert_eq!(
            validate_attributes(element, &mut short),
            Err(StopReason::WorkLimit)
        );
        assert_eq!(short.usage(), stopped_usage);
        let mut cancelled = budget(1000);
        cancelled.cancel();
        assert_eq!(
            validate_attributes(element, &mut cancelled),
            Err(StopReason::Cancelled)
        );
    }
    assert_eq!(
        validate_attributes(Element::Path { data: "" }, &mut budget(0)),
        Err(StopReason::WorkLimit)
    );
    Ok(())
}
#[test]
fn existing_fragment_enum_paths_remain_source_compatible() {
    let aspect: nepl3_markup::katex::fragment::Aspect = Aspect::MidSlice;
    let endpoint: nepl3_markup::katex::fragment::Endpoint = Endpoint::Full;
    assert_eq!(aspect, Aspect::MidSlice);
    assert_eq!(endpoint, Endpoint::Full);
}

#[test]
fn validation_borrows_and_preserves_noncanonical_lexical_bytes() -> Result<(), StopReason> {
    let width = String::from("0001.5000em");
    let height = String::from(".5000em");
    let viewport = String::from("-01.0 0 01.00 01");
    let value = svg(&width, &height, Some(&viewport), None);
    assert!(validate_attributes(value, &mut budget(1000))?);
    assert_eq!(width, "0001.5000em");
    assert_eq!(height, ".5000em");
    assert_eq!(viewport, "-01.0 0 01.00 01");
    assert!(matches!(value, Element::Svg { width: borrowed, .. }
        if std::ptr::eq(borrowed.as_ptr(), width.as_ptr())));
    Ok(())
}
