//! Desktop Doc host capacity against an authored sequence of short sentences.
use nepl3_tools::doc::{export, source::compiled};

#[test]
fn large_annotated_document_preserves_every_sentence() -> Result<(), String> {
    let mut source = String::from("article ja \"T\" body ");
    for _ in 0..128 {
        source.push_str("cons paragraph ");
        for _ in 0..4 {
            source.push_str("cons \"[文/ぶん]。\" ");
        }
        source.push_str("nil ");
    }
    source.push_str("nil");
    let output = export::generate(&compiled()?, &source)?;
    assert_eq!(output.html.matches("class=\"nepl-ruby\"").count(), 512);
    assert_eq!(output.html.matches("ぶん").count(), 512);
    assert_eq!(output.html.matches('。').count(), 512);
    assert!(!output.html.contains("<script"));
    let manifest: serde_json::Value =
        serde_json::from_str(&output.manifest).map_err(|e| e.to_string())?;
    // Record usage for observation. This capacity test checks content under the
    // bounded host pipeline; reducing validation visits must remain successful.
    // Budget rejection and unpublished output are checked independently by
    // canonical::tests::html_output_allowance_is_explicit_independent_and_recorded.
    println!("{}", manifest["operations"]);
    Ok(())
}
