use nepl3_tools::doc::{export, source::compiled};

#[test]
fn page_output_budget_is_explicit_shared_and_sticky() -> Result<(), String> {
    use nepl3_core::budget::{Budget, Resource, StopReason};
    use nepl3_tools::doc::{
        export::pages::{self, Entry},
        source::budget,
    };
    let compiled = compiled()?;
    let inputs = ["one", "two"].map(|id| {
        (
            Entry {
                id: id.into(),
                source: format!("{id}.nepld"),
                route: format!("{id}/index.html"),
            },
            "article en \"Example\" body cons paragraph cons \"A sentence.\" nil nil".into(),
        )
    });
    let default = pages::generate(&compiled, &inputs)?;
    let mut selected = budget();
    selected.charge(Resource::Work, 7).map_err(super::err)?;
    let first = pages::generate_with_output_budget(&compiled, &inputs, &mut selected)?;
    assert_eq!(default.files, first.files);
    let manifest: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    assert_eq!(manifest["output_budget"]["initial_usage"]["work"], 7);
    assert_eq!(
        manifest["output_budget"]["usage"]["work"],
        selected.usage().work
    );
    assert_eq!(manifest["output_budget"]["limits"]["work"], 100_000_000);
    for page in manifest["pages"].as_array().ok_or("pages")? {
        assert_eq!(page["operation_limits"]["work"], 100_000_000);
        assert_eq!(
            page["parse_and_validate_usage"]
                .as_object()
                .ok_or("parse usage")?
                .len(),
            8
        );
        assert_eq!(
            page["lower_usage"].as_object().ok_or("lower usage")?.len(),
            8
        );
    }
    let mut limits = budget().limits();
    limits.work += 1;
    let changed = pages::generate_with_output_budget(&compiled, &inputs, &mut Budget::new(limits))?;
    let changed_manifest: serde_json::Value =
        serde_json::from_str(&changed.manifest).map_err(super::err)?;
    assert_eq!(changed.files, first.files);
    assert_eq!(changed_manifest["identity"], manifest["identity"]);
    assert_ne!(
        changed_manifest["execution_identity"],
        manifest["execution_identity"]
    );
    assert_eq!(changed_manifest["pages"], manifest["pages"]);
    let mut cancelled = budget();
    cancelled.cancel();
    let before = cancelled.usage();
    assert!(
        pages::generate_with_output_budget(&compiled, &[], &mut cancelled)
            .is_err_and(|e| e == "Cancelled")
    );
    assert_eq!(cancelled.usage(), before);
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::OutputLimit,
    ] {
        let mut limits = budget().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 1,
            StopReason::AllocationLimit => limits.allocation_units = 1,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::DepthLimit => limits.depth = 0,
            StopReason::OutputLimit => limits.output_bytes = 0,
            _ => unreachable!(),
        }
        let mut stopped = Budget::new(limits);
        assert!(
            pages::generate_with_output_budget(&compiled, &inputs, &mut stopped).is_err(),
            "{reason:?}"
        );
        assert_eq!(stopped.poll(), Err(reason));
        let before = stopped.usage();
        assert!(pages::generate_with_output_budget(&compiled, &inputs, &mut stopped).is_err());
        assert_eq!(stopped.usage(), before);
    }
    let mut one = budget();
    pages::generate_with_output_budget(&compiled, &inputs[..1], &mut one)?;
    let mut limits = budget().limits();
    limits.work = one.usage().work;
    let mut shared = Budget::new(limits);
    assert!(pages::generate_with_output_budget(&compiled, &inputs, &mut shared).is_err());
    assert_eq!(shared.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

#[test]
fn page_manifest_requires_all_explicit_limit_fields() -> Result<(), String> {
    use nepl3_tools::doc::export::pages::{Manifest, resources::OutputLimits};
    let old = r#"{"version":1,"pages":[]}"#;
    let default: Manifest = serde_json::from_str(old).map_err(super::err)?;
    assert_eq!(default.output_limits.work, 100_000_000);
    let mut value: serde_json::Value = serde_json::from_str(old).map_err(super::err)?;
    value["output_limits"] = serde_json::to_value(OutputLimits::default()).map_err(super::err)?;
    assert!(serde_json::from_value::<Manifest>(value.clone()).is_ok());
    for bad in [
        serde_json::Value::Null,
        serde_json::json!({"work":200_000_000}),
    ] {
        let mut broken = value.clone();
        broken["output_limits"] = bad;
        assert!(serde_json::from_value::<Manifest>(broken).is_err());
    }
    for (name, bad) in [
        ("work", serde_json::json!(-1)),
        ("work", serde_json::json!(1.5)),
        ("unlimited", serde_json::json!(true)),
    ] {
        let mut broken = value.clone();
        broken["output_limits"][name] = bad;
        assert!(serde_json::from_value::<Manifest>(broken).is_err());
    }
    Ok(())
}

#[test]
fn export_reuses_the_admitted_tree_proof_without_recharging_parse() -> Result<(), String> {
    use nepl3_tools::doc::source::{budget, with_input_route};
    let compiled = compiled()?;
    let source = r#"article ja "[文書/ぶんしょ]" body cons paragraph cons "{[本文/ほんぶん]/body}。" cons sentence cons strong text "次の文。" nil nil nil"#;
    let mut expected = None;
    for native in [false, true] {
        let usage = with_input_route(
            native,
            &compiled,
            source,
            "Article",
            |tree, profile, b, _| {
                let before = b.usage();
                let proof = tree.syntax();
                assert!(std::ptr::eq(proof.bundle(), &tree.tree().bundle));
                assert_eq!(b.usage(), before);
                // A changed copy cannot inherit the borrowed proof of the original.
                let mut broken = tree.tree().clone();
                broken.bundle.nodes.clear();
                assert!(
                    broken
                        .validate(profile, &mut budget(), &mut Default::default())
                        .is_err()
                );
                let mut cancelled = budget();
                cancelled.cancel();
                assert!(matches!(
                    tree.tree()
                        .validate(profile, &mut cancelled, &mut Default::default()),
                    Err(nepl3_engine::tree::TreeError::Stopped(
                        nepl3_core::budget::StopReason::Cancelled
                    ))
                ));
                Ok(before)
            },
        )?;
        if native {
            expected = Some(usage);
        }
    }
    let output = export::generate(&compiled, source)?;
    let manifest: serde_json::Value = serde_json::from_str(&output.manifest).map_err(super::err)?;
    let usage = expected.ok_or("native usage")?;
    let parsed = &manifest["operations"]["parse_and_validate"];
    // Compare independent production entrypoints, not a copied numeric golden.
    assert_eq!(parsed["nodes"], usage.nodes);
    assert_eq!(parsed["work"], usage.work);
    assert_eq!(parsed["allocation_units"], usage.allocation_units);
    assert!(output.html.contains("class=\"nepl-ruby\""));
    assert!(output.html.contains("class=\"nepl-anno\""));
    assert!(output.html.contains("<strong"));
    Ok(())
}

#[test]
fn export_preserves_content_and_binds_script_free_files() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article ja "[文書/ぶんしょ]" body cons paragraph cons "{[本文/ほんぶん]/body} & <tag>" nil nil"#;
    let first = export::generate(&compiled, source)?;
    let second = export::generate(&compiled, source)?;
    assert_eq!(first.html, second.html);
    assert_eq!(first.manifest, second.manifest);
    assert!(first.html.contains("&amp; &lt;tag&gt;"));
    assert!(first.html.contains("class=\"nepl-ruby\""));
    assert!(first.html.contains("class=\"nepl-anno\""));
    assert!(!first.html.contains("<script"));
    assert!(first.html.contains("default-src 'none'"));
    let manifest: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    for (entry, bytes) in manifest["files"]
        .as_array()
        .ok_or("files")?
        .iter()
        .zip([first.html.as_bytes(), export::CSS.as_bytes()])
    {
        let expected = nepl3_core::source::Digest::of(bytes)
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(entry["sha256"].as_str(), Some(expected.as_str()));
    }
    assert_eq!(manifest["viewer_scripts"], false);
    assert_eq!(
        manifest["profile_sha256"].as_str().ok_or("profile")?.len(),
        64
    );
    assert!(export::generate(&compiled, &(source.to_owned() + " unexpected")).is_err());
    assert!(export::generate(&compiled, "article ja \"unclosed").is_err());
    assert!(
        export::generate(
            &compiled,
            &" ".repeat(export::MAX_SOURCE_BYTES as usize + 1)
        )
        .is_err_and(|e| e == "SourceLimit")
    );
    Ok(())
}

#[test]
fn export_reserves_depth_for_the_document_shell() -> Result<(), String> {
    let compiled = compiled()?;
    for count in [250, 251, 252] {
        let source = format!(
            "article ja sentence cons {}text \"x\" nil body nil",
            "strong ".repeat(count)
        );
        let result = export::generate(&compiled, &source);
        if count == 250 {
            result?;
        } else {
            assert!(result.is_err_and(|e| e.starts_with("OutputDepth")));
        }
    }
    Ok(())
}

#[test]
fn page_export_shares_script_free_shell_and_verifies_every_file() -> Result<(), String> {
    use nepl3_tools::doc::export::pages::{self, Entry};
    let compiled = compiled()?;
    let inputs = vec![
        (Entry { id: "intro".into(), source: "intro.nepld".into(), route: "docs/intro/index.html".into() },
            r#"article en "Intro" body cons paragraph cons sentence cons link page "guide" none text "Guide" nil nil nil"#.into()),
        (Entry { id: "guide".into(), source: "guide.nepld".into(), route: "docs/guide/index.html".into() },
            r#"article en "Guide" body cons paragraph cons sentence cons link page "intro" none text "Back" nil nil nil"#.into()),
    ];
    let first = pages::generate(&compiled, &inputs)?;
    let second = pages::generate(&compiled, &inputs)?;
    assert_eq!(first.files, second.files);
    assert_eq!(first.manifest, second.manifest);
    assert_eq!(first.files.len(), 4);
    let intro = std::str::from_utf8(&first.files["docs/intro/index.html"]).map_err(super::err)?;
    assert!(intro.contains("href=\"../guide/index.html\""));
    assert!(intro.contains("default-src 'none'"));
    assert!(intro.contains("href=\"assets/doc.css\""));
    assert!(!intro.contains("<script"));
    let manifest: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    for record in manifest["files"].as_array().ok_or("files")? {
        let path = record["path"].as_str().ok_or("path")?;
        let expected = nepl3_core::source::Digest::of(&first.files[path])
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(record["sha256"].as_str(), Some(expected.as_str()));
    }
    let mut inputs = inputs;
    for route in [
        "CON.html",
        "com1.html",
        "dir./page.html",
        "DOCS/guide.html",
        "docs/intro/INDEX.html",
    ] {
        inputs[1].0.route = route.into();
        assert!(pages::generate(&compiled, &inputs).is_err(), "{route}");
    }
    inputs[1].0.route = "docs/intro/assets/doc.css/nested.html".into();
    assert!(pages::generate(&compiled, &inputs).is_err_and(|e| e.contains("collision")));
    inputs[1].0.route = "../escape.html".into();
    assert!(pages::generate(&compiled, &inputs).is_err());
    inputs.pop();
    assert!(pages::generate(&compiled, &inputs).is_err_and(|e| e.contains("MissingPage")));
    assert!(pages::generate(&compiled, &[]).is_err());
    Ok(())
}
