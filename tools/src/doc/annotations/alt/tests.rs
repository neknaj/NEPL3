use super::*;
use crate::doc::source::{budget, compiled, err, with_input_route};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::check::Category;
use nepl3_wire::foundation::FoundationCodec;
#[test]
fn guest_projection_charges_owner_depth_independently_of_outer_doc_validation() -> Result<(), String>
{
    let compiled = compiled()?;
    let doc = with_input_route(
        true,
        &compiled,
        r#"Sentence "{[漢/かん]/note}""#,
        "Sentence",
        |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)
        },
    )?;
    let guest = doc.value.embeds.get(0).ok_or("guest")?;
    // No outer Doc traversal is charged here: only the actual selected operation.
    let run = |owner_depth, limits| -> Result<_, String> {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
        let mut b = Budget::new(limits);
        let result = project_guest(
            &guest.closure,
            owner_depth,
            &mut SentenceAnnotationRenderer {
                registry: &compiled.doc.registry,
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            },
            &mut b,
        );
        Ok((result.map(|(_, value)| value).map_err(err), b.usage()))
    };
    let (plain, baseline) = run(0, budget().limits())?;
    assert_eq!(plain?, "漢");
    let (nested, usage) = run(19, budget().limits())?;
    assert_eq!(nested?, "漢");
    assert_eq!(usage.depth, baseline.depth + 19);
    let mut exact = budget().limits();
    exact.depth = usage.depth;
    assert!(run(19, exact)?.0.is_ok());
    exact.depth -= 1;
    assert!(run(19, exact)?.0.is_err());
    Ok(())
}
