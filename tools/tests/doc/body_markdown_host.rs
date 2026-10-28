use super::*;
use nepl3_tools::doc::{
    export::pages::resources::ExecutionLimits, projection::annotated::body::host,
};
#[test]
fn body_markdown_source_host_preserves_bytes_and_phase_limits() -> Result<(), String> {
    let c = compiled()?;
    let source = "body nil";
    assert_eq!(
        host::from_source(&c, source, Default::default())?.markdown,
        ""
    );
    let mut limits = ExecutionLimits::default();
    // Parse admission includes provider/validation boundaries, not only file bytes.
    let admitted = nepl3_tools::doc::source::with_named_input_limits(
        true,
        &c,
        source,
        "doc-input",
        "Body",
        budget().limits(),
        |_, _, b, _| Ok(b.usage().source_bytes),
    )?;
    limits.parse_limits.source_bytes = admitted;
    host::from_source(&c, source, limits)?;
    limits.parse_limits.source_bytes -= 1;
    assert!(
        host::from_source(&c, source, limits)
            .err()
            .ok_or("expected error")?
            .contains("SourceLimit")
    );
    for phase in ["parse", "lower", "output"] {
        let mut limits = ExecutionLimits::default();
        match phase {
            "parse" => limits.parse_limits.work = 0,
            "lower" => limits.lower_limits.work = 0,
            _ => limits.output_limits.work = 0,
        }
        assert!(
            host::from_source(&c, source, limits)
                .err()
                .ok_or("expected error")?
                .contains("WorkLimit")
        );
    }
    assert!(host::from_source(&c, "article en \"T\" body nil", Default::default()).is_err());
    Ok(())
}
