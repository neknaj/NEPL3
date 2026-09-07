use super::{TestResult, budget, fixture};
use nepl3_core::{source::Digest, value::OperationRef};
use nepl3_engine::{profile::*, selection::HeadProviderRef};
#[path = "head/portable.rs"]
mod portable;
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn head_registrations_bind_alias_signature_host_identity_and_profile_digest() -> TestResult {
    let (package, registry) = fixture()?;
    let identity = package
        .check(&registry, &mut budget())
        .map_err(error)?
        .semantic_identity(&mut budget())
        .map_err(error)?;
    let engine = registry.selected("nepl3.engine", 1).ok_or("engine")?;
    let operation = |name: &str| OperationRef {
        schema: engine.clone(),
        name: name.into(),
    };
    let shape = operation("headShape");
    let child = operation("headChildContext");
    let facts = operation("bindingFacts");
    // This is a test host registration, never a distributed provider artifact.
    let implementation = Digest::of(b"head provider test implementation");
    let host = ProviderImplementation {
        provider: "head-test".into(),
        revision: 1,
        implementation_digest: implementation,
        operations: vec![shape.clone(), child.clone(), facts.clone()],
    };
    let mut profile = ParseProfile {
        id: "heads".into(),
        languages: vec![
            LanguageRegistration {
                alias: "A".into(),
                package: identity.clone(),
                default_category: "Expr".into(),
            },
            LanguageRegistration {
                alias: "B".into(),
                package: identity,
                default_category: "Expr".into(),
            },
        ],
        schemas: vec![
            package.schema.clone(),
            registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?
                .clone(),
            registry
                .selected("nepl3.reader", 1)
                .ok_or("reader")?
                .clone(),
            engine.clone(),
        ],
        category_modes: vec![],
        head_providers: vec![
            HeadRegistration {
                alias: "A".into(),
                category: "Expr".into(),
                provider: HeadProviderRef {
                    shape: shape.clone(),
                    child_context: child.clone(),
                },
            },
            HeadRegistration {
                alias: "B".into(),
                category: "Expr".into(),
                provider: HeadProviderRef {
                    shape: shape.clone(),
                    child_context: child.clone(),
                },
            },
        ],
        providers: [&shape, &child, &facts]
            .into_iter()
            .map(|op| ProviderRequirement {
                provider: host.provider.clone(),
                revision: 1,
                implementation_digest: implementation,
                operation: op.clone(),
            })
            .collect(),
        allowlist: vec![shape.clone(), child.clone(), facts.clone()],
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = [&package];
    let hosts = [host];
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &hosts,
        resources: &[],
    };
    let original = {
        let resolved = profile
            .resolve(&catalog, &registry, &mut budget())
            .map_err(error)?;
        assert_eq!(
            resolved
                .head_provider("A", "Expr", &mut budget())
                .map_err(error)?,
            Some(&profile.head_providers[0].provider)
        );
        resolved.digest()
    };
    profile.head_providers.reverse();
    assert_eq!(
        profile
            .resolve(&catalog, &registry, &mut budget())
            .map_err(error)?
            .digest(),
        original
    );
    profile.head_providers[0].provider.child_context = shape.clone();
    assert_ne!(
        profile
            .resolve(&catalog, &registry, &mut budget())
            .map_err(error)?
            .digest(),
        original
    );
    profile.head_providers[0].provider.child_context = child;
    for mutation in 0..5 {
        let mut bad = profile.clone();
        let expected = match mutation {
            0 => {
                bad.head_providers.push(bad.head_providers[0].clone());
                ProfileError::Duplicate
            }
            1 => {
                bad.head_providers[0].provider.shape = facts.clone();
                ProfileError::HeadSignature
            }
            2 => {
                bad.allowlist.retain(|op| op != &shape);
                ProfileError::NotAllowed
            }
            3 => {
                bad.providers[0].implementation_digest = Digest::of(b"different implementation");
                ProfileError::ProviderIdentity
            }
            _ => {
                bad.head_providers[0].alias = "Missing".into();
                ProfileError::MissingAlias
            }
        };
        assert!(
            matches!(bad.resolve(&catalog,&registry,&mut budget()),Err(e) if e==expected),
            "mutation {mutation}"
        );
    }
    Ok(())
}
