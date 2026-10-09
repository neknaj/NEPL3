use super::*;
use crate::doc::source::{Compiled, budget, compiled, err, with_input};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower};
use nepl3_wire::foundation::FoundationCodec;

fn document(c: &Compiled, text: &str) -> Result<DocumentSyntax, String> {
    with_input(c, text, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        lower::document(
            tree.syntax(),
            &c.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)
    })
}
fn run(
    c: &Compiled,
    doc: &DocumentSyntax,
    batch: bool,
    b: &mut Budget,
) -> Result<Vec<String>, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut host = MathDisplayHost {
        registry: &c.doc.registry,
        math_surface: &c.others[0].schema,
        sentence_surface: Some(&c.others[3].schema),
        doc_surface: Some(&c.doc.package.schema),
        codec: &mut codec,
    };
    let prepared = if batch {
        host.prepare_markdown_document(doc, b).map_err(err)?
    } else {
        let mut prepared = Vec::new();
        for (node, value) in doc.value.nodes.iter().enumerate() {
            if matches!(
                value.kind,
                DocKind::InlineMath { .. } | DocKind::DisplayMath { .. }
            ) {
                prepared.push(
                    host.prepare_markdown_node(doc, node as u64, b)
                        .map_err(err)?,
                );
            }
        }
        prepared
    };
    let cloned = doc.clone();
    let mut output = Vec::new();
    for ready in prepared {
        let placement = if ready.display {
            Placement::DisplayBlock
        } else {
            Placement::Inline
        };
        assert_eq!(
            ready.emit(&cloned, ready.node, placement, b),
            Err(EmitError::InputMismatch)
        );
        assert_eq!(
            ready.emit(doc, u64::MAX, placement, b),
            Err(EmitError::InputMismatch)
        );
        output.push(ready.emit(doc, ready.node, placement, b).map_err(err)?);
    }
    Ok(output)
}

#[test]
fn batch_preserves_output_and_reuses_only_owner_structure() -> Result<(), String> {
    let c = compiled()?;
    for paragraphs in [0, 4] {
        for expressions in [1, 4, 8] {
            let source = format!(
                "article en sentence cons text \"Title \" cons math Math frac 1 0 nil body {} {} nil",
                "cons paragraph cons \"Plain\" nil ".repeat(paragraphs),
                "cons display Math add x y ".repeat(expressions)
            );
            let doc = document(&c, &source)?;
            let mut single = budget();
            let expected = run(&c, &doc, false, &mut single)?;
            let mut batch = budget();
            assert_eq!(run(&c, &doc, true, &mut batch)?, expected);
            assert!(batch.usage().work < single.usage().work);
            eprintln!(
                "MATH_BATCH_METRICS paragraphs={paragraphs} display_math={expressions} single_work={} batch_work={}",
                single.usage().work,
                batch.usage().work
            );
            // A new call must re-admit its complete owner, not reuse prior success.
            let mut again = budget();
            assert_eq!(run(&c, &doc, true, &mut again)?, expected);
            assert_eq!(again.usage(), batch.usage());
        }
    }
    Ok(())
}

#[test]
fn batch_keeps_cumulative_exact_limits_and_sticky_stops() -> Result<(), String> {
    let c = compiled()?;
    let doc = document(
        &c,
        "article en \"T\" body cons display Math frac 1 0 cons display Math add x y cons display Math mul a b nil",
    )?;
    let mut measured = budget();
    let expected = run(&c, &doc, true, &mut measured)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
    ] {
        let mut limits = measured.limits();
        match reason {
            StopReason::WorkLimit => limits.work = measured.usage().work,
            StopReason::AllocationLimit => {
                limits.allocation_units = measured.usage().allocation_units
            }
            _ => limits.output_bytes = measured.usage().output_bytes,
        }
        let mut exact = Budget::new(limits);
        assert_eq!(run(&c, &doc, true, &mut exact)?, expected);
        assert_eq!(exact.usage(), measured.usage());
        match reason {
            StopReason::WorkLimit => limits.work -= 1,
            StopReason::AllocationLimit => limits.allocation_units -= 1,
            _ => limits.output_bytes -= 1,
        }
        let mut short = Budget::new(limits);
        assert!(run(&c, &doc, true, &mut short).is_err());
        assert_eq!(short.poll(), Err(reason));
        assert!(run(&c, &doc, true, &mut short).is_err());
        assert_eq!(short.poll(), Err(reason));
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(run(&c, &doc, true, &mut cancelled).is_err());
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    let mut limits = measured.limits();
    limits.depth = 0;
    let mut shallow = Budget::new(limits);
    assert!(run(&c, &doc, true, &mut shallow).is_err());
    assert_eq!(shallow.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn batch_rejects_later_owner_corruption_and_unsupported_guest() -> Result<(), String> {
    let c = compiled()?;
    let good = document(
        &c,
        "article en \"T\" body cons display Math frac 1 0 cons paragraph cons \"After Math\" nil nil",
    )?;
    let mut orphan = good.clone();
    orphan.value.nodes.push(orphan.value.nodes[0].clone());
    let mut unused = good.clone();
    unused.value.embeds.push(unused.value.embeds[0].clone());
    let mut category = good.clone();
    category.value.embeds[0].closure.syntax.category = "NotExpr".into();
    let mut bad_origin = good.clone();
    let text = bad_origin
        .value
        .nodes
        .iter_mut()
        .rfind(|n| matches!(&n.kind, DocKind::Text { .. }))
        .ok_or("text node")?;
    text.origin = Some(nepl3_core::origin::OriginId(u64::MAX));
    for doc in [orphan, unused, category, bad_origin] {
        assert!(run(&c, &doc, true, &mut budget()).is_err());
    }
    let unsupported = document(
        &c,
        r#"article en "T" body cons display Math frac 1 0 cons display Math label x Sentence "[字/じ]" nil"#,
    )?;
    assert!(run(&c, &unsupported, true, &mut budget()).is_err());
    Ok(())
}

#[test]
fn batch_rejects_wrong_surface_and_conflicting_source_admission() -> Result<(), String> {
    use nepl3_core::source::SourceSnapshot;
    let c = compiled()?;
    let doc = document(&c, "article en \"T\" body cons display Math frac 1 0 nil")?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut host = MathDisplayHost {
        registry: &c.doc.registry,
        math_surface: &c.others[3].schema,
        sentence_surface: Some(&c.others[3].schema),
        doc_surface: Some(&c.doc.package.schema),
        codec: &mut codec,
    };
    assert!(matches!(
        host.prepare_markdown_document(&doc, &mut budget()),
        Err(PrepareError::Host(crate::doc::math::Error::Selection))
    ));
    let original = doc.sources.first().ok_or("source")?;
    let conflicting = SourceSnapshot::new(
        original.identity().source.clone(),
        original.identity().revision,
        original.uri().into(),
        b"conflicting content".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let mut admission = SourceAdmission::default();
    admission
        .admit_existing(&conflicting, &mut budget())
        .map_err(err)?;
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut host = MathDisplayHost {
        registry: &c.doc.registry,
        math_surface: &c.others[0].schema,
        sentence_surface: Some(&c.others[3].schema),
        doc_surface: Some(&c.doc.package.schema),
        codec: &mut codec,
    };
    assert!(matches!(
        host.prepare_markdown_document(&doc, &mut budget()),
        Err(PrepareError::Host(crate::doc::math::Error::Document(_)))
    ));
    Ok(())
}

#[derive(Debug)]
struct StageFailure;
impl From<StopReason> for StageFailure {
    fn from(_: StopReason) -> Self {
        Self
    }
}
fn prepare_only(
    c: &Compiled,
    doc: &DocumentSyntax,
    baseline: u64,
    b: &mut Budget,
) -> Result<usize, StageFailure> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(|_| StageFailure)?;
    let mut host = MathDisplayHost {
        registry: &c.doc.registry,
        math_surface: &c.others[0].schema,
        sentence_surface: Some(&c.others[3].schema),
        doc_surface: Some(&c.doc.package.schema),
        codec: &mut codec,
    };
    b.with_depth_at_least(baseline, |b| {
        host.prepare_markdown_document(doc, b)
            .map(|v| v.len())
            .map_err(|_| StageFailure)
    })
}

#[test]
fn batch_preparation_stops_before_emission_and_keeps_caller_depth() -> Result<(), String> {
    let c = compiled()?;
    let doc = document(
        &c,
        "article en \"T\" body cons display Math frac 1 0 cons display Math add x y cons display Math mul a b nil",
    )?;
    for baseline in [0, 25] {
        let mut measured = budget();
        assert_eq!(
            prepare_only(&c, &doc, baseline, &mut measured).map_err(err)?,
            3
        );
        for reason in [StopReason::WorkLimit, StopReason::DepthLimit] {
            let mut limits = measured.limits();
            if reason == StopReason::WorkLimit {
                limits.work = measured.usage().work;
            } else {
                limits.depth = measured.usage().depth;
            }
            let mut exact = Budget::new(limits);
            assert_eq!(
                prepare_only(&c, &doc, baseline, &mut exact).map_err(err)?,
                3
            );
            if reason == StopReason::WorkLimit {
                limits.work -= 1;
            } else {
                limits.depth -= 1;
            }
            let mut short = Budget::new(limits);
            assert!(prepare_only(&c, &doc, baseline, &mut short).is_err());
            assert_eq!(short.poll(), Err(reason));
            assert_eq!(short.current_depth(), 0);
        }
    }
    let empty = document(&c, "article en \"No Math\" body nil")?;
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    let mut no_allocation = Budget::new(limits);
    assert_eq!(
        prepare_only(&c, &empty, 0, &mut no_allocation).map_err(err)?,
        0
    );
    assert_eq!(no_allocation.usage().allocation_units, 0);
    no_allocation.cancel();
    assert!(prepare_only(&c, &empty, 0, &mut no_allocation).is_err());
    assert_eq!(no_allocation.poll(), Err(StopReason::Cancelled));
    Ok(())
}
