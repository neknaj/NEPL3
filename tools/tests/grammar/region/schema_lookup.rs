use super::*;
use nepl3_core::{
    budget::StopReason,
    view::{FallbackRole, PresentationClass},
};
use nepl3_engine::{
    analysis::{BindingOptions, region::RegionError},
    portable::{PortableError, analysis, region},
};
use nepl3_reader::model::ReaderFact;
use nepl3_wire::foundation::FoundationCodec;
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn region_fact_schema_admission_stops_after_nonempty_vocabulary() -> Result<(), String> {
    let doc = document(include_bytes!(
        "../../../../conformance/fixtures/grammar/binding/execution.json"
    ))?;
    let compiled = nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.region.lookup",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    super::super::binding::with_completed_input(&compiled, "lambda x x", |parsed, profile, _, _| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let keyed = analysis::prepare(
            "lookup",
            parsed.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let template = parsed.reader_facts().first().ok_or("reader batch")?;
        let span = parsed.tree().bundle.sources[0].span(0, 1).map_err(err)?;
        let schema = profile
            .registry()
            .selected("nepl3.foundation", 1)
            .ok_or("foundation")?;
        for relation in [false, true] {
            for defect in 0..3 {
                let mut unknown = schema.clone();
                match defect {
                    0 => unknown.package.push('x'),
                    1 => unknown.revision += 1,
                    _ => unknown.digest.0[0] ^= 1,
                }
                let make = |name: &str| {
                    let mut batch = template.clone();
                    batch.facts = vec![if relation {
                        ReaderFact::Relation {
                            schema: unknown.clone(),
                            kind: name.into(),
                            from: span.clone(),
                            to: span.clone(),
                        }
                    } else {
                        ReaderFact::Presentation {
                            class: PresentationClass {
                                schema: unknown.clone(),
                                name: name.into(),
                                fallback: FallbackRole::Content,
                            },
                            span: span.clone(),
                        }
                    }];
                    vec![batch]
                };
                let control = make("");
                let mut b = budget();
                let mut a = SourceAdmission::default();
                let mut c =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                assert!(matches!(
                    region::prepare(&keyed, Some(&control), &mut c, &mut b),
                    Err(PortableError::Region(RegionError::Sidecar))
                ));
                let before = b.usage();
                let facts = make("x");
                let original = facts.clone();
                let mut b = Budget::new(Limits {
                    work: before.work + 1,
                    ..budget().limits()
                });
                let mut a = SourceAdmission::default();
                let mut c =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                // Empty vocabulary short-circuits the lookup. One extra byte
                // consumes the remaining Work, so lookup entry must stop.
                assert!(matches!(
                    region::prepare(&keyed, Some(&facts), &mut c, &mut b),
                    Err(PortableError::Stopped(StopReason::WorkLimit))
                ));
                assert_eq!(b.poll(), Err(StopReason::WorkLimit));
                assert_eq!(facts, original);
                assert_eq!(
                    b.usage(),
                    Usage {
                        work: before.work + 1,
                        ..before
                    }
                );
                let mut b = budget();
                let mut a = SourceAdmission::default();
                let mut c =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                assert!(matches!(
                    region::prepare(&keyed, Some(&facts), &mut c, &mut b),
                    Err(PortableError::Region(RegionError::Sidecar))
                ));
            }
            let mut batch = template.clone();
            batch.facts = vec![if relation {
                ReaderFact::Relation {
                    schema: schema.clone(),
                    kind: "arbitrary-vocabulary".into(),
                    from: span.clone(),
                    to: span.clone(),
                }
            } else {
                ReaderFact::Presentation {
                    class: PresentationClass {
                        schema: schema.clone(),
                        name: "arbitrary-vocabulary".into(),
                        fallback: FallbackRole::Content,
                    },
                    span: span.clone(),
                }
            }];
            let facts = vec![batch];
            let mut b = budget();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            region::prepare(&keyed, Some(&facts), &mut c, &mut b).map_err(err)?;
        }
        Ok(())
    })
}
