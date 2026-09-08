use nepl3_core::budget::Resource;
use nepl3_tools::doc::{
    export::pages::{
        self, Entry,
        resources::{OperationLimits, PhaseLimits},
    },
    source,
};

fn inputs() -> Vec<(Entry, String)> {
    vec![(
        Entry {
            id: "page".into(),
            source: "doc.md".into(),
            route: "index.html".into(),
            input: None,
        },
        "article ja \"[本文/ほんぶん]\" body cons paragraph cons \"abc\" nil nil".into(),
    )]
}
#[test]
fn phase_configuration_binds_real_profile_and_keeps_native_portable_tree_equal()
-> Result<(), String> {
    let compiled = source::compiled()?;
    let input = &inputs()[0].1;
    let mut limits = source::budget().limits();
    limits.work += 12345;
    let run = |native, limits| {
        source::with_named_input_limits(
            native,
            &compiled,
            input,
            "page",
            "Article",
            limits,
            |tree, profile, b, _| {
                assert_eq!(profile.profile().limits, limits);
                assert_eq!(b.limits(), limits);
                assert_eq!(tree.tree().profile_digest, profile.digest());
                Ok((tree.tree().clone(), profile.digest()))
            },
        )
    };
    let native = run(true, limits)?;
    assert_eq!(native, run(false, limits)?);
    assert_ne!(native.1, run(true, source::budget().limits())?.1);
    let default = source::with_named_input(
        true,
        &compiled,
        input,
        "page",
        "Article",
        |tree, profile, _, _| Ok((tree.tree().clone(), profile.digest())),
    )?;
    assert_eq!(default, run(true, source::budget().limits())?);
    Ok(())
}

#[test]
fn phase_settings_preserve_defaults_and_do_not_hide_stops() -> Result<(), String> {
    let compiled = source::compiled()?;
    let inputs = inputs();
    let first = pages::generate(&compiled, &inputs)?;
    let explicit = pages::generate_with_phase_limits(
        &compiled,
        &inputs,
        PhaseLimits::default(),
        &mut source::budget(),
    )?;
    assert_eq!(first.files, explicit.files);
    assert_eq!(first.manifest, explicit.manifest);
    let original: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    let mut phases = PhaseLimits::default();
    phases.lower.work += 1;
    let changed =
        pages::generate_with_phase_limits(&compiled, &inputs, phases, &mut source::budget())?;
    let manifest: serde_json::Value =
        serde_json::from_str(&changed.manifest).map_err(super::err)?;
    assert_eq!(first.files, changed.files);
    for key in ["identity", "execution_identity"] {
        assert_eq!(original[key], manifest[key]);
    }
    assert_ne!(original["phase_execution"], manifest["phase_execution"]);
    assert!(manifest["pages"][0]["operation_limits"].is_null());
    for lower in [false, true] {
        let mut phases = PhaseLimits::default();
        if lower {
            phases.lower.work = 0;
        } else {
            phases.parse.work = 0;
        }
        let mut output = source::budget();
        output.charge(Resource::Work, 7).map_err(super::err)?;
        let initial = output.usage();
        let error = pages::generate_with_phase_limits(&compiled, &inputs, phases, &mut output)
            .err()
            .ok_or("unexpected success")?;
        assert!(error.contains("WorkLimit"), "{error}");
        assert_eq!(error.contains("lower:"), lower, "{error}");
        assert_eq!(initial, output.usage());
    }
    Ok(())
}

#[test]
fn phase_manifest_requires_complete_finite_records() -> Result<(), String> {
    let minimal = serde_json::json!({"version":1,"pages":[]});
    let parsed: pages::Manifest = serde_json::from_value(minimal.clone()).map_err(super::err)?;
    assert_eq!(parsed.parse_limits, OperationLimits::default());
    assert_eq!(parsed.lower_limits, OperationLimits::default());
    for field in ["parse_limits", "lower_limits"] {
        for bad in [
            serde_json::Value::Null,
            serde_json::json!({}),
            serde_json::json!({"work":1}),
        ] {
            let mut value = minimal.clone();
            value[field] = bad;
            assert!(serde_json::from_value::<pages::Manifest>(value).is_err());
        }
        let limits = serde_json::to_value(OperationLimits::default()).map_err(super::err)?;
        for bad in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("18446744073709551616"),
        ] {
            let mut value = minimal.clone();
            value[field] = limits.clone();
            value[field]["work"] = bad;
            assert!(serde_json::from_value::<pages::Manifest>(value).is_err());
        }
        for edge in [0, u64::MAX] {
            let mut value = minimal.clone();
            value[field] = limits.clone();
            value[field]["work"] = serde_json::json!(edge);
            assert!(serde_json::from_value::<pages::Manifest>(value).is_ok());
        }
        let mut value = minimal.clone();
        value[field] = limits.clone();
        value[field]["extra"] = serde_json::json!(1);
        assert!(serde_json::from_value::<pages::Manifest>(value).is_err());
        let raw =
            format!("{{\"version\":1,\"pages\":[],\"{field}\":{limits},\"{field}\":{limits}}}");
        assert!(serde_json::from_str::<pages::Manifest>(&raw).is_err());
    }
    Ok(())
}
