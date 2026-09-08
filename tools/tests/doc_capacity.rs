//! Desktop Doc host capacity against an authored sequence of short sentences.
use nepl3_tools::doc::{export, source::compiled};

#[test]
fn annotated_sentences_exceed_the_old_host_node_cap_without_losing_content() -> Result<(), String> {
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
    let parsed = &manifest["operations"]["parse_and_validate"];
    println!("{}", manifest["operations"]);
    assert!(parsed["nodes"].as_u64().ok_or("nodes")? > 1_000_000);
    Ok(())
}
