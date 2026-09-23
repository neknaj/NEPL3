//! Public Sentence -> suite -> markup route, with no Doc core dependency.
use nepl3_core::{
    budget::{Budget, Limits},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot},
};
use nepl3_sentence_core::literal::{self, SentenceOutcome};
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn independent_sentence_renders_ruby_without_doc() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        registry
            .register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    registry.finalize(&mut b()).map_err(err)?;
    let source = SourceSnapshot::new(
        SourceId("external-html".into()),
        1,
        "memory:html".into(),
        "\"[漢/かん]\"".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let parsed = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &registry,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let SentenceOutcome::Matched(parsed) = parsed.outcome else {
        return Err("literal".into());
    };
    let result = nepl3_suite::adapters::sentence::html::render(
        &parsed.syntax,
        &registry,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(result.input().sources, vec![source]);
    assert_eq!(result.origins().len(), result.markup().fragment.nodes.len());
    let markup = result.markup();
    let proof =
        nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, &mut b())
            .map_err(err)?;
    let output = nepl3_markup::html::serialize(&proof, &mut b()).map_err(err)?;
    // Exact expected structure is authored independently of the renderer.
    assert_eq!(
        output,
        "<span class=\"nepl-sentence\"><span><span class=\"nepl-ruby\"><span class=\"nepl-base\">漢</span><span class=\"nepl-reading\">かん</span></span></span></span>"
    );
    Ok(())
}
