//! The source host must use the resolved registrations, not its four-language
//! fixture's names, order, root categories or reader modes.
use nepl3_core::source::{Digest, SourceAdmission, SourceId, SourceSnapshot};
use nepl3_engine::profile::*;
use nepl3_tools::doc::source::{budget, compiled, err, with_input};

#[test]
fn source_host_rejects_unavailable_implementation_on_both_routes() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "article en sentence \"Host\" body nil",
        "Article",
        |_, original, _, _| {
            let mut profile = original.profile().clone();
            let unavailable = Digest::of(b"different executable, same operation schemas");
            for requirement in &mut profile.providers {
                requirement.implementation_digest = unavailable;
            }
            // The profile is internally consistent but names an implementation
            // which this host does not supply. Schema equality is insufficient.
            let providers = profile
                .providers
                .iter()
                .map(|requirement| ProviderImplementation {
                    provider: requirement.provider.clone(),
                    revision: requirement.revision,
                    implementation_digest: requirement.implementation_digest,
                    operations: vec![requirement.operation.clone()],
                })
                .collect::<Vec<_>>();
            let packages = std::iter::once(&compiled.doc.package)
                .chain(compiled.others.iter())
                .collect::<Vec<_>>();
            let resolved = profile
                .resolve(
                    &RuntimeCatalog {
                        packages: &packages,
                        providers: &providers,
                        resources: &[],
                    },
                    original.registry(),
                    &mut budget(),
                )
                .map_err(err)?;
            for native in [false, true] {
                let mut b = budget();
                let source = SourceSnapshot::new(
                    SourceId("provider-check".into()),
                    0,
                    "memory:provider-check".into(),
                    b"anchor mark text \"kept\"".to_vec(),
                    &mut b,
                )
                .map_err(err)?;
                let result = nepl3_tools::doc::source::parse_source_route(
                    &source,
                    &resolved,
                    "Doc",
                    "Inline",
                    &mut b,
                    &mut SourceAdmission::default(),
                    native,
                );
                assert!(
                    matches!(result, Err(ref e) if e.contains("Context")),
                    "{result:?}"
                );
            }
            Ok(())
        },
    )
}

#[test]
fn source_host_uses_registered_aliases_and_default_categories() -> Result<(), String> {
    let mut compiled = compiled()?;
    let mut alternate = compiled.doc.package.modes[0].clone();
    alternate.name = "ProseMode".into();
    compiled.doc.package.modes.push(alternate);
    with_input(
        &compiled,
        "article en sentence \"Host\" body nil",
        "Article",
        |_, original, _, _| {
            let mut profile = original.profile().clone();
            let mut language = profile.languages[0].clone();
            language.alias = "Prose".into();
            language.default_category = "Inline".into();
            profile.languages.reverse();
            profile.languages.insert(0, language);
            profile.category_modes.push(CategoryMode {
                alias: "Prose".into(),
                category: "Inline".into(),
                mode: "ProseMode".into(),
            });
            let providers = profile
                .providers
                .iter()
                .map(|requirement| ProviderImplementation {
                    provider: requirement.provider.clone(),
                    revision: requirement.revision,
                    implementation_digest: requirement.implementation_digest,
                    operations: vec![requirement.operation.clone()],
                })
                .collect::<Vec<_>>();
            let packages = std::iter::once(&compiled.doc.package)
                .chain(compiled.others.iter())
                .collect::<Vec<_>>();
            let resolved = profile
                .resolve(
                    &RuntimeCatalog {
                        packages: &packages,
                        providers: &providers,
                        resources: &[],
                    },
                    original.registry(),
                    &mut budget(),
                )
                .map_err(err)?;
            // The package's foreign dependencies remain explicit. The extra
            // alias is first in a reordered catalog and selects Inline instead
            // of the package root. It must get its own environment and state.
            let mut trees = Vec::new();
            for native in [false, true] {
                let mut b = budget();
                let source = SourceSnapshot::new(
                    SourceId("registered-alias".into()),
                    0,
                    "memory:registered-alias".into(),
                    b"anchor mark text \"kept\"".to_vec(),
                    &mut b,
                )
                .map_err(err)?;
                let mut admission = SourceAdmission::default();
                let tree = nepl3_tools::doc::source::parse_source_route(
                    &source,
                    &resolved,
                    "Prose",
                    "Inline",
                    &mut b,
                    &mut admission,
                    native,
                )?;
                tree.validate(&resolved, &mut b, &mut admission)
                    .map_err(err)?;
                assert_eq!(
                    tree.bundle.nodes[tree.bundle.root.0 as usize].schema,
                    compiled.doc.package.schema
                );
                trees.push(tree);
            }
            assert_eq!(trees[0], trees[1]);
            Ok(())
        },
    )
}
