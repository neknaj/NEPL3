use nepl3_core::{budget::{Budget, StopReason}, schema::SchemaRegistry, source::{SourceAdmission, SourceStore, Digest}};
use nepl3_doc_core::{check::Category, lower::{self, DocumentLowerError}};
use nepl3_tools::doc::{source::{budget, compiled, with_input_route, err}, export, projection};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn retained_proof_preserves_fresh_boundaries_and_entrypoint_accounting() -> Result<(), String> {
    let compiled = compiled()?;
    for source in [
        "article en sentence cons strong text \"A < B\" nil body cons paragraph cons sentence cons text \"Body\" nil nil nil",
        "article ja \"[文/ぶん]\" body cons paragraph cons \"{[語/ご]/word}。\" nil nil",
        "article en \"Simple\" body cons paragraph cons \"Body\" nil nil",
    ] {
        let mut expected_native = None;
        let mut docs = Vec::new();
        for native in [false, true] {
            let result = with_input_route(native, &compiled, source, "Article", |tree, profile, b, _| {
                let before = b.usage();
                let syntax = tree.syntax();
                assert!(std::ptr::eq(syntax.bundle(), &tree.tree().bundle));
                assert_eq!(before, b.usage());
                // Every independently changed tree must establish its own proof.
                let mut bad = tree.tree().clone();
                bad.profile_digest = Digest::of(b"different effective profile");
                assert!(matches!(bad.validate(profile, &mut budget(), &mut SourceAdmission::default()), Err(nepl3_engine::tree::TreeError::Selection)));
                let mut bad = tree.tree().clone();
                bad.bundle.sources.clear();
                assert!(bad.validate(profile, &mut budget(), &mut SourceAdmission::default()).is_err());
                let mut bad = tree.tree().clone();
                bad.bundle.root.0 = u64::MAX;
                assert!(bad.validate(profile, &mut budget(), &mut SourceAdmission::default()).is_err());
                // The retained proof is not an exemption for a fresh lower operation.
                for reason in [StopReason::SourceLimit, StopReason::WorkLimit, StopReason::AllocationLimit, StopReason::DepthLimit, StopReason::NodeLimit, StopReason::Cancelled] {
                    let mut limits = budget().limits();
                    match reason {
                        StopReason::SourceLimit => limits.source_bytes = 0,
                        StopReason::WorkLimit => limits.work = 0,
                        StopReason::AllocationLimit => limits.allocation_units = 0,
                        StopReason::DepthLimit => limits.depth = 0,
                        StopReason::NodeLimit => limits.nodes = 0,
                        _ => (),
                    }
                    let mut limited = Budget::new(limits);
                    if reason == StopReason::Cancelled { limited.cancel(); }
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
                    let result = lower::document(syntax, &compiled.doc.package.schema, Category::Article, profile.registry(), &mut limited, &mut codec);
                    assert!(matches!(result, Err(DocumentLowerError::Stopped(s)) if s == reason), "{reason:?}: {result:?}");
                    assert_eq!(limited.poll(), Err(reason));
                }
                let mut empty_registry = SchemaRegistry::default();
                empty_registry.finalize(&mut budget()).map_err(err)?;
                assert!(lower::prefix(syntax, &compiled.doc.package.schema, Category::Article, &empty_registry, &mut budget(), &mut SourceAdmission::default()).is_err());
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
                let mut next = budget();
                let doc = lower::document(syntax, &compiled.doc.package.schema, Category::Article, profile.registry(), &mut next, &mut codec).map_err(err)?;
                assert!(next.usage().source_bytes >= source.len() as u64);
                // An independently minted syntax proof must produce the same domain value.
                let another = tree.tree().bundle.validate_with_sources(profile.registry(), &mut budget(), &mut SourceAdmission::default()).map_err(err)?;
                let mut other_admission = SourceAdmission::default();
                let mut other_codec = FoundationCodec::new(profile.registry(), &store, &mut other_admission).map_err(err)?;
                let mut other_budget = budget();
                let other_doc = lower::document(&another, &compiled.doc.package.schema, Category::Article, profile.registry(), &mut other_budget, &mut other_codec).map_err(err)?;
                assert_eq!(doc, other_doc);
                assert_eq!(next.usage(), other_budget.usage());
                assert_eq!(before, b.usage());
                println!("proof source={:?} native={native} parse={before:?} lower={:?}", Digest::of(source.as_bytes()), next.usage());
                Ok((before, doc))
            })?;
            if native { expected_native = Some(result.0); }
            docs.push(result.1);
        }
        assert_eq!(docs[0], docs[1]);
        let output = export::generate(&compiled, source)?;
        let manifest: serde_json::Value = serde_json::from_str(&output.manifest).map_err(err)?;
        let u = expected_native.ok_or("missing native usage")?;
        let parse = &manifest["operations"]["parse_and_validate"];
        for (name, value) in [("source_bytes",u.source_bytes),("work",u.work),("depth",u.depth),("nodes",u.nodes),("allocation_units",u.allocation_units),("output_bytes",u.output_bytes),("diagnostics",u.diagnostics),("events",u.events)] {
            assert_eq!(parse[name], value, "{name}");
        }
        let md = projection::from_source(&compiled, source);
        if source.contains("Simple") {
            assert_eq!(md?, "# Simple\n\nBody\n");
        } else {
            // This separate projection explicitly supports only Text/Code/Break.
            assert!(md.is_err_and(|e| e.starts_with("Unsupported")));
        }
    }
    Ok(())
}
