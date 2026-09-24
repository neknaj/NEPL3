//! Explicit host measurements of native validation and portable materialization.
//! Operations use independent default budgets and fresh admission contexts.
use nepl3_core::{
    source::{SourceAdmission, SourceStore},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{check::Category, lower, model::DocContent, portable};
use nepl3_tools::doc::source::{budget, compiled, err, with_input_route};
use nepl3_wire::foundation::FoundationCodec;
use std::time::Instant;

#[test]
#[ignore = "explicit portable materialization measurement; records bounded failures"]
fn measure_sentence_closure_materialization() -> Result<(), String> {
    let compiled = compiled()?;
    for paragraphs in [8, 32, 128] {
        // Parser workload construction: each paragraph has four authored
        // sentences. This source is never a production document generator.
        let mut input = String::from("article ja sentence \"T\" body ");
        for _ in 0..paragraphs {
            input.push_str("cons paragraph ");
            for _ in 0..4 {
                input.push_str("cons sentence \"[文/ぶん]。\" ");
            }
            input.push_str("nil ");
        }
        input.push_str("nil");
        with_input_route(true, &compiled, &input, "Article", |tree, profile, _, _| {
            let registry = profile.registry();
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
            let document = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let embeds = &document.value.embeds;
            assert_eq!(embeds.len(), paragraphs * 4 + 1);
            let DocContent::Syntax { closure } = &embeds[1].content else {
                return Err("Sentence syntax closure".into());
            };
            println!(
                "input bytes={} paragraphs={paragraphs} embeds={} owner_origins={} owner_sources={} owner_maps={}",
                input.len(),
                embeds.len(),
                closure.provenance.origins().len(),
                closure.provenance.sources().len(),
                closure.provenance.source_maps().len(),
            );
            let mut measured = budget();
            let mut admission = SourceAdmission::default();
            let start = Instant::now();
            let checked = document.validate_structure(registry, &mut measured, &mut admission);
            println!(
                "operation=native_validation elapsed_ns={} usage={:?} error={:?}",
                start.elapsed().as_nanos(),
                measured.usage(),
                checked.as_ref().err()
            );
            checked.map_err(err)?;
            let mut measured = budget();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
            let start = Instant::now();
            let encoded = codec.encode_foreign_closure(closure, &mut measured);
            println!(
                "operation=one_closure elapsed_ns={} usage={:?} error={:?}",
                start.elapsed().as_nanos(),
                measured.usage(),
                encoded.as_ref().err()
            );
            encoded.map_err(err)?;
            let mut measured = budget();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(registry, &sources, &mut admission).map_err(err)?;
            let start = Instant::now();
            let encoded = portable::to_value(&document, registry, &mut codec, &mut measured);
            println!(
                "operation=document_ndf elapsed_ns={} usage={:?} error={:?}",
                start.elapsed().as_nanos(),
                measured.usage(),
                encoded.as_ref().err()
            );
            // This diagnostic run records a finite-budget stop and continues
            // to the other scales. The mandatory capacity test still requires
            // successful rendering of all 512 sentences at unchanged limits.
            match encoded {
                Ok(_) | Err(portable::PortableError::Stopped(_)) => Ok(()),
                Err(error) => Err(err(error)),
            }
        })?;
    }
    Ok(())
}
