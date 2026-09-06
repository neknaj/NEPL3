use nepl3_core::{
    budget::{Budget, Limits},
    origin::Origin,
    schema::{SchemaRegistry, foundation},
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry},
};
use nepl3_reader::{context::ContextError, model::ReaderContext};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}

#[test]
fn checked_context_uses_real_digest_and_exact_origin_source_closure() -> TestResult {
    let mut setup = budget();
    let descriptor = foundation::descriptor(&mut setup).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let source = |id: &str, text: &str| {
        SourceSnapshot::new(
            SourceId(id.into()),
            1,
            format!("memory:{id}"),
            text.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))
    };
    let auxiliary = source("auxiliary", "aux")?;
    let span = auxiliary.span(0, 3).map_err(|e| format!("{e:?}"))?;
    let mut sources = SourceStore::default();
    for source in [
        source("input", "input")?,
        auxiliary,
        source("unrelated", "unrelated source")?,
    ] {
        sources.insert(source).map_err(|e| format!("{e:?}"))?;
    }
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&environment, &schema, &registry, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let mut context = ReaderContext {
        schema,
        category: "fixture".into(),
        mode: "default".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        origins: vec![Origin::Direct(span.clone()), Origin::Direct(span)],
    };
    let mut usage = budget();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&registry, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    {
        let checked = context
            .check(&mut codec, &sources, &registry, &mut usage)
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(checked.sources().len(), 1);
        assert_eq!(checked.sources()[0].identity().source.0, "auxiliary");
        assert_eq!(usage.usage().source_bytes, 3);
    }
    context.environment.digest.0[0] ^= 1;
    assert!(matches!(
        context.check(&mut codec, &sources, &registry, &mut usage),
        Err(ContextError::DigestMismatch)
    ));
    Ok(())
}
