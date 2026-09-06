//! The first seed adapter is only an input path. These tests execute the actual
//! no_std compiler and package checker against complete repository sources.
use nepl3_core::{budget::*, schema::*, source::*, view::*};
use nepl3_grammar_core::compile::{
    self, NamedClass,
    package::{CompiledLanguage, PackageContext},
};
use std::{path::PathBuf, process::Command};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 500_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn load(path: &str) -> Result<nepl3_grammar_core::model::Document, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("root")?
        .to_path_buf();
    let output = Command::new("python")
        .arg(root.join("tools/bootstrap/grammar.py"))
        .arg(root.join(path))
        .args(["--source-id", path, "--uri", "memory:grammar-example"])
        .env("PYTHONIOENCODING", "utf-8")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    nepl3_tools::bootstrap::load(
        &output.stdout,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))
}
fn compile(path: &str) -> Result<Result<CompiledLanguage, compile::CompileError>, String> {
    let doc = load(path)?;
    let mut b = budget();
    let checked = doc
        .validate(&mut b, &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_reader::schema::descriptor(&mut b),
        nepl3_engine::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let schema = descriptor.reference(&mut b).map_err(|e| format!("{e:?}"))?;
        registry
            .register(schema, descriptor, &mut b)
            .map_err(|e| format!("{e:?}"))?;
    }
    registry.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    let schema = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let classes = [
        ("content", FallbackRole::Content),
        ("marker", FallbackRole::Marker),
        ("name", FallbackRole::Name),
        ("quantity", FallbackRole::Quantity),
    ]
    .into_iter()
    .map(|(n, f)| NamedClass {
        name: n.into(),
        class: PresentationClass {
            schema: schema.clone(),
            name: n.into(),
            fallback: f,
        },
    })
    .collect::<Vec<_>>();
    Ok(compile::package::compile(
        &checked,
        &PackageContext {
            package: "test.surface",
            state_type: &TypeDescriptor::Unit,
            reader_imports: &[],
            extensions: &[],
            classes: &classes,
            views: &[],
        },
        registry,
        &mut b,
    ))
}
#[test]
fn full_example_assembly_reaches_actual_plan_and_binding_validation() -> Result<(), String> {
    assert!(matches!(
        compile("tools/tests/fixtures/grammar/angle-choice-output.neplg")?,
        Err(compile::CompileError::Reader(
            nepl3_reader::plan::PlanError::OutputType
        ))
    ));
    let compiled = compile("examples/grammar/angle-tag.neplg")?
        .map_err(|e| format!("corrected angle: {e:?}"))?;
    compiled
        .package
        .check(&compiled.registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        compile("examples/grammar/binding.neplg")?,
        Err(compile::CompileError::Package(
            nepl3_engine::package::PackageError::InvalidBinding
        ))
    ));
    Ok(())
}

#[test]
fn shared_kind_is_category_local_with_one_shape_and_distinct_provenance() -> Result<(), String> {
    let original =
        compile("tools/tests/fixtures/grammar/shared-kind.neplg")?.map_err(|e| format!("{e:?}"))?;
    let reordered = compile("tools/tests/fixtures/grammar/shared-kind-reordered.neplg")?
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(original.package.forms.len(), 2);
    assert_eq!(
        original.package.forms[0].kind,
        original.package.forms[1].kind
    );
    let a = original
        .package
        .check(&original.registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let b = reordered
        .package
        .check(&reordered.registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        a.semantic_identity(&mut budget())
            .map_err(|e| format!("{e:?}"))?,
        b.semantic_identity(&mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    let declarations = original
        .package
        .provenance
        .declarations
        .iter()
        .filter(|d| d.name == "Atom")
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0].category.as_deref(), Some("A"));
    assert_eq!(declarations[1].category.as_deref(), Some("B"));
    assert_ne!(declarations[0].origin, declarations[1].origin);
    for declaration in declarations {
        let nepl3_core::origin::Origin::Direct(span) =
            &original.package.provenance.origins[declaration.origin.0 as usize]
        else {
            return Err("direct declaration provenance".into());
        };
        let raw = original.package.provenance.sources[0]
            .slice(span)
            .map_err(|e| format!("{e:?}"))?;
        assert!(raw.starts_with(&format!(
            "form Atom {} ",
            declaration.category.as_deref().ok_or("category")?
        )));
    }
    for (file, reason) in [
        (
            "shared-kind-duplicate",
            compile::DeclarationError::DuplicateName,
        ),
        ("shared-kind-shape", compile::DeclarationError::KindShape),
    ] {
        let err = compile(&format!("tools/tests/fixtures/grammar/{file}.neplg"))?
            .err()
            .ok_or("expected rejection")?;
        assert!(
            matches!(err,compile::CompileError::Declaration{node,related:Some(other),reason:r} if r==reason && node!=other)
        );
    }
    Ok(())
}
