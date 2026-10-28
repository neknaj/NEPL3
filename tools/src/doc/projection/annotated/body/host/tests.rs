use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> crate::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "nepl3-body-markdown-source-test-{}-{}",
            std::process::id(),
            NEXT_STAGE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn source_ceiling_and_exact_output_budget_are_enforced() -> crate::Result<()> {
    let c = crate::doc::source::compiled()?;
    let source = r#"body cons paragraph cons "hello" nil nil"#;
    let phases = PhaseLimits::default();
    let mut baseline = crate::doc::source::budget();
    let expected = from_source_profile(&c, source, phases, &mut baseline)?;
    let mut limits = baseline.limits();
    limits.output_bytes = baseline.usage().output_bytes;
    assert_eq!(
        from_source_profile(&c, source, phases, &mut Budget::new(limits))?.markdown,
        expected.markdown
    );
    limits.output_bytes -= 1;
    assert!(
        from_source_profile(&c, source, phases, &mut Budget::new(limits))
            .err()
            .ok_or("expected error")?
            .contains("OutputLimit")
    );
    let mut cancelled = crate::doc::source::budget();
    cancelled.cancel();
    assert!(
        from_source_profile(&c, source, phases, &mut cancelled)
            .err()
            .ok_or("expected error")?
            .contains("Cancelled")
    );
    let oversized = " ".repeat(crate::doc::export::MAX_SOURCE_BYTES as usize + 1);
    assert!(
        from_source(&c, &oversized, Default::default())
            .err()
            .ok_or("expected error")?
            .contains("SourceLimit")
    );
    let f = Fixture::new()?;
    let input = f.0.join("big.nepld");
    let output = f.0.join("out.md");
    fs::write(&input, oversized)?;
    assert!(write(&input, &output, Default::default()).is_err());
    assert!(!output.exists());
    Ok(())
}
