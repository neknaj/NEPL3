use super::*;
use nepl3_engine::package::{PackageError, SelectionRule, StyleSelector};
#[path = "region/boundaries.rs"]
mod boundaries;
#[path = "region/mapped.rs"]
mod mapped;

fn document(bytes: &[u8]) -> Result<nepl3_grammar_core::model::Document, String> {
    nepl3_tools::bootstrap::load(bytes, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))
}
#[test]
fn actual_parser_region_selects_field_priority_before_inner_depth() -> Result<(), String> {
    use nepl3_engine::{
        analysis::{BindingOptions, region::*},
        portable::{analysis, region},
    };
    use nepl3_wire::foundation::FoundationCodec;
    let doc = document(include_bytes!(
        "../../../conformance/fixtures/grammar/region/priority.json"
    ))?;
    let compiled = compile_document(&doc)?.map_err(|e| format!("{e:?}"))?;
    super::binding::with_completed_input(&compiled, "pair 1 2", |parsed, profile, _, _| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let keyed = analysis::prepare(
            "region",
            parsed.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let input = region::prepare(
            &keyed,
            Some(parsed.reader_facts()),
            &mut codec,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let request = RegionRequest {
            key: input.key(),
            source: parsed.tree().bundle.sources[0].reference(),
            offset: 5,
        };
        let reply = regions(
            &input,
            &request,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let RegionOutcome::Complete {
            selection: Some(i),
            regions,
        } = &reply.outcome
        else {
            return Err(format!("{reply:?}"));
        };
        assert_eq!(
            regions[*i as usize].target.part,
            RegionPart::Field {
                field: 0,
                element: None
            }
        );
        assert_eq!(regions[*i as usize].priority, 9);
        assert_eq!(
            (
                regions[*i as usize].span.start(),
                regions[*i as usize].span.end()
            ),
            (5, 6)
        );
        let packet = region::sidecar_value(
            &keyed,
            Some(parsed.reader_facts()),
            &mut codec,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let wire = nepl3_wire::encode(&packet, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let packet = nepl3_wire::decode(&wire, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let facts = region::sidecar_decode(&packet, &keyed, &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let received = region::prepare(&keyed, facts.as_deref(), &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(received.key(), input.key());
        let packet = region::reply_to_value(&reply, &request, &input, &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
        let decoded = region::reply_decode(&value, &request, &received, &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(reply, decoded);
        Ok(())
    })
}
#[test]
fn grammar_selection_priority_is_separate_from_style_and_changes_identity() -> Result<(), String> {
    let doc = document(include_bytes!(
        "../../../conformance/fixtures/grammar/region/priority.json"
    ))?;
    let mut result = compile_document(&doc)?.map_err(|e| format!("{e:?}"))?;
    let package = &mut result.package;
    assert_eq!(package.forms[0].styles.len(), 1);
    assert_eq!(
        package.forms[0].selection_rules,
        vec![
            SelectionRule {
                selector: StyleSelector::Head,
                priority: 7
            },
            SelectionRule {
                selector: StyleSelector::Field("first".into()),
                priority: 9
            },
        ]
    );
    assert_eq!(package.leaves[0].selection_rules[0].priority, 3);
    let before = package
        .check(&result.registry, &mut budget())
        .and_then(|p| p.semantic_identity(&mut budget()))
        .map_err(|e| format!("{e:?}"))?;
    package.forms[0].selection_rules[0].priority += 1;
    let after = package
        .check(&result.registry, &mut budget())
        .and_then(|p| p.semantic_identity(&mut budget()))
        .map_err(|e| format!("{e:?}"))?;
    assert_ne!(before, after);
    Ok(())
}
#[test]
fn grammar_selection_errors_keep_the_exact_operand_location() -> Result<(), String> {
    // Expected positions come from the literal producer operands, independently
    // of compiled package arena indices or the error's actual returned span.
    let cases: [(&[u8], &str, &str); 4] = [
        (
            include_bytes!("../../../conformance/fixtures/grammar/region/overflow.json"),
            include_str!("../../../conformance/fixtures/grammar/region/overflow.neplg"),
            "18446744073709551616",
        ),
        (
            include_bytes!("../../../conformance/fixtures/grammar/region/duplicate.json"),
            include_str!("../../../conformance/fixtures/grammar/region/duplicate.neplg"),
            "selection head 8",
        ),
        (
            include_bytes!("../../../conformance/fixtures/grammar/region/field.json"),
            include_str!("../../../conformance/fixtures/grammar/region/field.neplg"),
            "absent",
        ),
        (
            include_bytes!("../../../conformance/fixtures/grammar/region/capture.json"),
            include_str!("../../../conformance/fixtures/grammar/region/capture.neplg"),
            "absent",
        ),
    ];
    for (bytes, source, operand) in cases {
        let doc = document(bytes)?;
        let error = compile_document(&doc)?
            .err()
            .ok_or("invalid selection accepted")?;
        let start = source.find(operand).ok_or("expected operand")? as u64;
        let at = error
            .location()
            .ok_or_else(|| format!("unlocated: {error:?}"))?
            .primary();
        assert_eq!(
            (at.start(), at.end()),
            (start, start + operand.len() as u64)
        );
        if operand == "absent" {
            assert_eq!(
                error.cause(),
                &compile::CompileError::Package(PackageError::InvalidSelector)
            );
        }
    }
    Ok(())
}
#[test]
fn actual_reader_capture_and_nested_view_remain_token_owned() -> Result<(), String> {
    use nepl3_engine::{
        analysis::{BindingOptions, region::*},
        portable::{analysis, region},
    };
    use nepl3_wire::foundation::FoundationCodec;
    for (seed, input_text, capture) in [
        (
            &include_bytes!("../../../conformance/fixtures/grammar/region/captured.json")[..],
            "pair 1 2",
            true,
        ),
        (
            &include_bytes!("../../../conformance/fixtures/grammar/region/view.json")[..],
            "pair 123 45",
            false,
        ),
    ] {
        let doc = document(seed)?;
        let compiled =
            compile_document_views(&doc, &["Outer", "Reading"])?.map_err(|e| format!("{e:?}"))?;
        super::binding::with_completed_input(&compiled, input_text, |parsed, profile, _, _| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
            let keyed = analysis::prepare(
                "region",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let prepared = region::prepare(
                &keyed,
                Some(parsed.reader_facts()),
                &mut codec,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let request = RegionRequest {
                key: prepared.key(),
                source: parsed.tree().bundle.sources[0].reference(),
                offset: if capture { 5 } else { 6 },
            };
            let reply = regions(
                &prepared,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let RegionOutcome::Complete {
                selection: Some(i),
                regions: out,
            } = &reply.outcome
            else {
                return Err(format!("{reply:?}"));
            };
            let selected = &out[*i as usize];
            if capture {
                assert!(matches!(selected.target.part, RegionPart::Capture { .. }));
                assert_eq!(selected.priority, 11);
                assert_eq!((selected.span.start(), selected.span.end()), (5, 6));
                let without = region::prepare(&keyed, None, &mut codec, &mut budget())
                    .map_err(|e| format!("{e:?}"))?;
                assert_ne!(prepared.key(), without.key());
                assert_eq!(without.capability(), RegionCapability::SyntaxOnly);
            } else {
                assert_eq!(
                    selected.target.part,
                    RegionPart::View {
                        root: 0,
                        path: vec![ViewStep { field: 0, child: 0 }]
                    }
                );
                assert_eq!((selected.span.start(), selected.span.end()), (6, 8));
            }
            let packet =
                region::reply_to_value(&reply, &request, &prepared, &mut codec, &mut budget())
                    .map_err(|e| format!("{e:?}"))?;
            let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(|e| format!("{e:?}"))?;
            let packet = nepl3_wire::decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
            assert_eq!(
                region::reply_decode(&packet, &request, &prepared, &mut codec, &mut budget())
                    .map_err(|e| format!("{e:?}"))?,
                reply
            );
            Ok(())
        })?;
    }
    Ok(())
}
