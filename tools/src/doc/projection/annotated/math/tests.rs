use super::*;
use crate::doc::source::{budget, compiled, err};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn batch_scope_guard_rejects_changed_bounds_and_preserves_stops() -> Result<(), Error> {
    let mut b = budget();
    let limits = b.limits();
    let depth = b.current_depth();
    check_scope(&mut b, limits, depth)?;
    b.with_depth_at_least(1, |b| {
        assert!(matches!(
            check_scope(b, limits, depth),
            Err(Error::Invalid(_))
        ));
        Ok::<_, Error>(())
    })?;
    let changed = nepl3_core::budget::Limits {
        work: limits.work - 1,
        ..limits
    };
    b.with_ceiling(changed, |b| {
        assert!(matches!(
            check_scope(b, limits, depth),
            Err(Error::Invalid(_))
        ));
        Ok::<_, Error>(())
    })?;
    b.cancel();
    assert!(matches!(
        check_scope(&mut b, limits, depth),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn math_batch_reuses_document_validation_without_changing_fragments() -> Result<(), String> {
    let c = compiled()?;
    let surfaces = MathSurfaces {
        math: &c.others[0].schema,
        sentence: Some(&c.others[3].schema),
        doc: Some(&c.doc.package.schema),
    };
    for count in [2, 4, 8] {
        let source = format!(
            "article en \"Math\" body {} nil",
            "cons display Math frac 1 2 ".repeat(count)
        );
        let document = crate::doc::source::with_named_input(
            true,
            &c,
            &source,
            "batch",
            "Article",
            |tree, profile, _, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                lower::document(
                    tree.syntax(),
                    &c.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut budget(),
                    &mut codec,
                )
                .map_err(err)
            },
        )?;
        let run = |legacy: bool| -> Result<(Vec<String>, nepl3_core::budget::Usage), String> {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
            let mut b = budget();
            let entries = if legacy {
                // Same loop, node charges and retained-entry allocation as the
                // batch, but each public call validates the containing Doc.
                let mut host = MathDisplayHost {
                    registry: &c.doc.registry,
                    math_surface: surfaces.math,
                    sentence_surface: surfaces.sentence,
                    doc_surface: surfaces.doc,
                    codec: &mut codec,
                };
                let mut entries = Vec::new();
                for (node, value) in document.value.nodes.iter().enumerate() {
                    b.charge(Resource::Work, 1).map_err(err)?;
                    let embed = match value.kind {
                        DocKind::InlineMath { syntax } | DocKind::DisplayMath { syntax } => syntax,
                        _ => continue,
                    };
                    let prepared = host
                        .prepare_markdown_node(&document, node as u64, &mut b)
                        .map_err(err)?;
                    push(
                        &mut entries,
                        MathEntry {
                            node: node as u64,
                            embed,
                            prepared,
                        },
                        &mut b,
                    )
                    .map_err(err)?;
                }
                entries
            } else {
                prepare(&document, surfaces, &c.doc.registry, &mut codec, &mut b).map_err(err)?
            };
            assert_eq!(entries.len(), count);
            let output = entries
                .iter()
                .map(|entry| {
                    entry
                        .prepared
                        .emit(&document, entry.node, Placement::DisplayBlock, &mut b)
                        .map_err(err)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((output, b.usage()))
        };
        let (old, repeated) = run(true)?;
        let (new, batched) = run(false)?;
        assert_eq!(new, old);
        assert!(
            batched.work < repeated.work,
            "{count}: {batched:?} versus {repeated:?}"
        );
        assert!(batched.allocation_units < repeated.allocation_units);
        assert_eq!(batched.source_bytes, repeated.source_bytes);
        assert_eq!(batched.depth, repeated.depth);
        assert_eq!(batched.output_bytes, repeated.output_bytes);
    }
    Ok(())
}
