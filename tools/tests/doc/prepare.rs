use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{check::Category, lower, model::*, portable, prepare::*};

// Both Math guests contain division by zero. Code display must retain the
// guest's syntax; discovering requirements must not evaluate either guest.
const INPUT: &str = r#"article ja "準備"
body
 cons paragraph
   cons sentence
     cons link page "guide" some "intro" text "参照"
     cons image asset "logo" none "代替"
     cons math Math frac 1 0
     nil
   nil
 cons code Math frac 1 0
 nil"#;

#[test]
fn source_preparation_retains_code_and_identifies_each_foreign_closure() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(&compiled, INPUT, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut c,
        )
        .map_err(err)?;
        let original = doc.clone();
        let plan = inspect(&doc, profile.registry(), &mut c, &mut budget()).map_err(err)?;
        assert_eq!(plan.requirements.len(), 4);
        assert!(matches!(&plan.requirements[0], DocRequirement::Link {
            target: LinkTarget::Page { page, fragment }, ..
        } if page == "guide" && fragment.as_deref() == Some("intro")));
        assert!(matches!(&plan.requirements[1], DocRequirement::Asset {
            asset: AssetRef { id, digest: None }, ..
        } if id == "logo"));
        for (index, kind) in [EmbedKind::InlineMath, EmbedKind::Code]
            .into_iter()
            .enumerate()
        {
            // Independent digest calculation uses actual CBOR bytes and the
            // specified domain, rather than calling inspect's digest helper.
            let closure = c
                .encode_foreign_closure(&doc.value.embeds[index].closure, &mut budget())
                .map_err(err)?;
            let mut bytes = b"NEPL3.Doc.Prepare.Guest.v1\0".to_vec();
            bytes.extend(nepl3_wire::encode(&closure, &mut budget()).map_err(err)?);
            assert_eq!(
                plan.requirements[2 + index],
                DocRequirement::Foreign {
                    embed: EmbedRef(index as u64),
                    kind,
                    guest_digest: Digest::of(&bytes),
                }
            );
        }
        let doc_bytes = nepl3_wire::encode(
            &portable::to_value(&doc, profile.registry(), &mut c, &mut budget()).map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let plan_bytes = nepl3_wire::encode(
            &portable::prepare::plan_to_value(
                &plan,
                &doc,
                profile.registry(),
                &mut c,
                &mut budget(),
            )
            .map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let mut fresh_admission = SourceAdmission::default();
        let mut fresh =
            FoundationCodec::new(profile.registry(), &empty, &mut fresh_admission).map_err(err)?;
        let received = portable::from_value(
            &nepl3_wire::decode(&doc_bytes, &mut budget()).map_err(err)?,
            profile.registry(),
            &mut fresh,
            &mut budget(),
        )
        .map_err(err)?;
        let actual = portable::prepare::plan_from_value(
            &nepl3_wire::decode(&plan_bytes, &mut budget()).map_err(err)?,
            &received,
            profile.registry(),
            &mut fresh,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(actual, plan);
        assert_eq!(doc, original);
        assert_eq!(received.value.root, doc.value.root);
        assert_eq!(received.value.nodes, doc.value.nodes);
        for (index, (a, b)) in received
            .value
            .embeds
            .iter()
            .zip(&doc.value.embeds)
            .enumerate()
        {
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.closure.owner_environment, b.closure.owner_environment);
            assert_eq!(a.closure.owner_origins, b.closure.owner_origins);
            assert_eq!(a.closure.owner_source_maps, b.closure.owner_source_maps);
            assert_eq!(a.closure.syntax.schema, b.closure.syntax.schema);
            assert_eq!(a.closure.syntax.category, b.closure.syntax.category);
            let sources = |values: &[SourceSnapshot]| {
                let mut rows: Vec<_> = values
                    .iter()
                    .map(|s| (s.identity().clone(), s.text().to_owned()))
                    .collect();
                rows.sort_by(|a, b| a.0.cmp(&b.0));
                rows
            };
            assert_eq!(
                sources(&a.closure.owner_sources),
                sources(&b.closure.owner_sources),
                "embed {index} sources"
            );
        }
        assert!(received.origins == doc.origins, "origin table differs");
        assert!(received.views == doc.views, "views differ");
        assert!(
            received.source_maps == doc.source_maps,
            "source maps differ"
        );
        // Source tables and guest NodeRef coordinates canonicalize on wire.
        // The full canonical bytes must still preserve every field and source.
        let reencoded = nepl3_wire::encode(
            &portable::to_value(&received, profile.registry(), &mut fresh, &mut budget())
                .map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(reencoded, doc_bytes);
        Ok(())
    })
}
