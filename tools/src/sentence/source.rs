//! Standalone Sentence composition through the production prefix engine.
use crate::source::{
    driver,
    host::{NativeHost, NativeReader},
};
use nepl3_core::{
    budget::{Budget, Resource},
    source::{Digest, SourceAdmission, SourceSnapshot},
};
use nepl3_engine::{profile::*, tree::ValidatedParseTree};
use nepl3_grammar_core::compile::package::CompiledLanguage;
use nepl3_reader::builtin::{BuiltinReader, provider};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

/// `implementation` is supplied by the host from its actual executable/asset.
/// `native=false` exercises the owned Await boundary with the same registered code.
#[allow(clippy::too_many_arguments)]
pub fn with_tree<T>(
    compiled: &CompiledLanguage,
    source: &SourceSnapshot,
    implementation: Digest,
    b: &mut Budget,
    a: &mut SourceAdmission,
    native: bool,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let r = &compiled.registry;
    let prefix = driver::reservation_prefix(source, b)?;
    b.charge(
        Resource::AllocationUnits,
        prefix.len() as u64 + 2 * core::mem::size_of::<NativeReader>() as u64,
    )
    .map_err(err)?;
    let readers = vec![
        NativeReader {
            operation: provider::operation(BuiltinReader::Name, r, b).map_err(err)?,
            read: provider::read,
        },
        NativeReader {
            operation: super::reader::signature(r, b).map_err(err)?.operation,
            read: super::reader::read,
        },
    ];
    let mut host = NativeHost::new(r, implementation, prefix.clone(), readers, b).map_err(err)?;
    let providers = host.providers().to_vec();
    let profile = ParseProfile {
        id: "sentence-source".into(),
        languages: vec![LanguageRegistration {
            alias: "Sentence".into(),
            package: compiled
                .package
                .check(r, b)
                .and_then(|p| p.semantic_identity(b))
                .map_err(err)?,
            default_category: compiled.package.root.clone(),
        }],
        schemas: crate::bootstrap::runtime::schemas(compiled, b).map_err(err)?,
        providers: providers
            .iter()
            .flat_map(|p| {
                p.operations.iter().map(|operation| ProviderRequirement {
                    provider: p.provider.clone(),
                    revision: p.revision,
                    implementation_digest: p.implementation_digest,
                    operation: operation.clone(),
                })
            })
            .collect(),
        allowlist: providers
            .iter()
            .flat_map(|p| p.operations.iter().cloned())
            .collect(),
        category_modes: vec![],
        head_providers: vec![],
        resources: vec![],
        limits: b.limits(),
    };
    let packages = [&compiled.package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &providers,
                resources: &[],
            },
            r,
            b,
        )
        .map_err(err)?;
    let tree = driver::parse(
        source,
        &resolved,
        "Sentence",
        &compiled.package.root,
        b,
        a,
        native,
        "sentence-parse",
        &prefix,
        &mut host,
    )?;
    let checked = tree.validate(&resolved, b, a).map_err(err)?;
    finish(&checked, &resolved, b, a)
}
