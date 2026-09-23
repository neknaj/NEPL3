//! Compile and resolve the example packages through the consumer's public API.
use external_hello_language::{Languages, budget, composition, error};
use nepl3_engine::profile::RuntimeCatalog;

#[test]
fn independent_host_resolves_shared_definitions_and_rejects_missing_guest() -> Result<(), String> {
    let languages: Languages<'_> = composition::languages("Arithmetic", "Wrapper")?;
    assert_eq!(languages.packages.len(), 2);
    let profile = languages.profile("independent-host")?;
    assert_eq!(profile.languages[0].alias, "Arithmetic");
    assert_eq!(profile.languages[1].alias, "Wrapper");
    let packages = languages
        .packages
        .iter()
        .map(|(_, package)| package)
        .collect::<Vec<_>>();
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &[],
        resources: &[],
    };
    profile
        .resolve(&catalog, &languages.registry, &mut budget())
        .map_err(error)?;
    // A profile names exact semantic identities. An absent Frame definition
    // must fail resolution before a host can execute or grant an operation.
    let missing = RuntimeCatalog {
        packages: &packages[..1],
        providers: &[],
        resources: &[],
    };
    assert!(
        profile
            .resolve(&missing, &languages.registry, &mut budget())
            .is_err()
    );
    assert!(profile.providers.is_empty());
    assert!(profile.allowlist.is_empty());
    Ok(())
}
