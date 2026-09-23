//! Shared development-host standard Doc source pipeline.
//! Packages are compiled from checked bootstrap fixtures; these are not claimed
//! to be a completed generated LanguagePackage distribution or suite runtime.
use nepl3_core::{budget::*, source::*};
use nepl3_engine::{profile::*, tree::ValidatedParseTree};
use nepl3_grammar_core::compile::package::CompiledLanguage;
#[cfg(test)]
mod tests;
pub fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
pub fn host_identity() -> Digest {
    Digest::of(
        concat!(
            include_str!("source.rs"),
            include_str!("host.rs"),
            include_str!("../source/host.rs"),
            include_str!("../source/driver.rs"),
            include_str!("reader.rs"),
            include_str!("../sentence/reader.rs"),
            include_str!("../sentence/reader/descriptor.rs"),
            include_str!("../sentence/catalog.rs"),
            include_str!("../bootstrap/runtime.rs"),
            include_str!("../bootstrap/runtime/host.rs"),
            include_str!("../../../crates/integration/suite/src/adapters/sentence/document.rs"),
            include_str!("../../../crates/foundation/reader/src/builtin/provider.rs")
        )
        .as_bytes(),
    )
}
pub fn with_input<T>(
    compiled: &Compiled,
    input: &str,
    category: &str,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_input_route(false, compiled, input, category, finish)
}
pub fn with_input_route<T>(
    native: bool,
    compiled: &Compiled,
    input: &str,
    category: &str,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_named_input(native, compiled, input, "doc-input", category, finish)
}
/// Assign distinct source identities when compiling several pages into one set.
pub fn with_named_input<T>(
    native: bool,
    compiled: &Compiled,
    input: &str,
    source_name: &str,
    category: &str,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_named_input_limits(
        native,
        compiled,
        input,
        source_name,
        category,
        budget().limits(),
        finish,
    )
}
/// Configuration-only entry: select limits before creating this parse operation.
/// The selected ceiling is also bound into its resolved ParseProfile.
#[allow(clippy::too_many_arguments)]
pub fn with_named_input_limits<T>(
    native: bool,
    compiled: &Compiled,
    input: &str,
    source_name: &str,
    category: &str,
    parse_limits: Limits,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let r = &compiled.doc.registry;
    let packages: Vec<_> = std::iter::once(&compiled.doc.package)
        .chain(compiled.others.iter())
        .collect();
    // Fixture implementation identity binds all code selected by this host.
    let implementation_for = |_: &nepl3_core::value::OperationRef| host_identity();
    let mut operations = Vec::new();
    for operation in packages
        .iter()
        .flat_map(|p| p.reader.providers.iter().map(|v| &v.operation))
    {
        if !operations.contains(operation) {
            operations.push(operation.clone());
        }
    }
    let providers: Vec<_> = operations
        .iter()
        .map(|operation| ProviderImplementation {
            provider: operation.name.clone(),
            revision: 1,
            implementation_digest: implementation_for(operation),
            operations: vec![operation.clone()],
        })
        .collect();
    let requirements = operations
        .iter()
        .map(|operation| ProviderRequirement {
            provider: operation.name.clone(),
            revision: 1,
            implementation_digest: implementation_for(operation),
            operation: operation.clone(),
        })
        .collect();
    let mut schemas = packages
        .iter()
        .map(|p| p.schema.clone())
        .collect::<Vec<_>>();
    for schema in packages
        .iter()
        .flat_map(|p| p.payload_schemas.iter())
        .chain(
            [
                "nepl3.foundation",
                "nepl3.reader",
                "nepl3.engine",
                "nepl3.doc",
                "nepl3.doc.reader",
                "nepl3.sentence.reader",
                "nepl3.grammar",
            ]
            .iter()
            .filter_map(|name| r.selected(name, 1)),
        )
    {
        if !schemas.contains(schema) {
            schemas.push(schema.clone());
        }
    }
    let profile = ParseProfile {
        id: "doc-prefix".into(),
        languages: packages
            .iter()
            .zip([
                ("Doc", "Article"),
                ("Math", "Expr"),
                ("Circuit", "Design"),
                ("Grammar", "Root"),
                ("Sentence", "Sentence"),
            ])
            .map(|(p, (alias, category))| {
                Ok(LanguageRegistration {
                    alias: alias.into(),
                    package: p
                        .check(r, &mut budget())
                        .and_then(|c| c.semantic_identity(&mut budget()))
                        .map_err(err)?,
                    default_category: category.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        schemas,
        head_providers: vec![],
        category_modes: vec![],
        providers: requirements,
        allowlist: operations,
        resources: vec![],
        limits: parse_limits,
    };
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &providers,
                resources: &[],
            },
            r,
            &mut budget(),
        )
        .map_err(err)?;
    let mut b = Budget::new(parse_limits);
    let mut a = SourceAdmission::default();
    let source = SourceSnapshot::new(
        SourceId(source_name.into()),
        0,
        format!("memory:{source_name}"),
        input.as_bytes().to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let tree = parse_source_route(&source, &resolved, "Doc", category, &mut b, &mut a, native)?;
    let checked = tree
        .validate(&resolved, &mut b, &mut a)
        .map_err(|e| format!("tree validation: {e:?}; usage={:?}", b.usage()))?;
    finish(&checked, &resolved, &mut b, &mut a)
}

pub fn parse_source_as(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<nepl3_engine::recovery::ParseTree, String> {
    parse_source_route(source, resolved, alias, category, b, a, false)
}
#[allow(clippy::too_many_arguments)]
pub fn parse_source_route(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
    native: bool,
) -> Result<nepl3_engine::recovery::ParseTree, String> {
    let prefix = crate::source::driver::reservation_prefix(source, b)?;
    b.charge(Resource::AllocationUnits, prefix.len() as u64)
        .map_err(err)?;
    let mut host =
        crate::doc::host::native(resolved.registry(), host_identity(), prefix.clone(), b)
            .map_err(err)?;
    crate::source::driver::parse(
        source,
        resolved,
        alias,
        category,
        b,
        a,
        native,
        "doc-parse",
        &prefix,
        &mut host,
    )
}

/// Desktop development-host allowance. `nodes` includes cumulative typed-value
/// validation visits, not just the number of syntax nodes in the final tree.
/// Every operation still keeps its own fixed, sticky Work/Allocation limits.
pub fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 10_000_000,
        allocation_units: 500_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
pub struct Compiled {
    pub doc: CompiledLanguage,
    pub others: Vec<nepl3_engine::package::LanguagePackage>,
}
pub fn compiled() -> Result<Compiled, String> {
    compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
        kind: "InlineMath",
        category: "Inline",
        spelling: "math",
        field: "syntax",
        alias: "Math",
        guest_category: "Expr",
        origin_reason: "Doc host selects Math expressions in Sentence Inline",
    }])
}
/// Compile the parsing profile with the host's explicit Sentence extensions.
/// Rendering and document namespace resolution require corresponding adapters;
/// registering a surface form alone does not supply those operations.
pub fn compiled_with_sentence_forms(
    forms: &[nepl3_grammar_core::compile::package::ForeignForm<'_>],
) -> Result<Compiled, String> {
    let document = crate::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/doc/syntax.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut doc = crate::doc::catalog::compile(
        &document,
        "standard.doc",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut others = Vec::new();
    for (name, seed) in [
        (
            "math",
            include_bytes!("../../../conformance/fixtures/doc/math.json").as_slice(),
        ),
        (
            "circuit",
            include_bytes!("../../../conformance/fixtures/doc/circuit.json").as_slice(),
        ),
        (
            "grammar",
            include_bytes!("../../../conformance/fixtures/doc/grammar.json").as_slice(),
        ),
    ] {
        let document = crate::bootstrap::load(seed, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let other = crate::doc::catalog::compile(
            &document,
            &format!("standard.{name}"),
            &mut budget(),
            &mut SourceAdmission::default(),
        )?;
        let schema = other.package.schema.clone();
        let descriptor = other
            .registry
            .descriptor(&schema)
            .ok_or("surface descriptor")?
            .clone();
        doc.registry
            .register(schema, descriptor, &mut budget())
            .map_err(err)?;
        others.push(other.package);
    }
    let sentence = crate::sentence::catalog::standard_with_foreign_forms(
        host_identity(),
        forms,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    for schema in [
        &sentence.package.schema,
        sentence
            .registry
            .selected("nepl3.sentence.reader", 1)
            .ok_or("Sentence reader schema")?,
    ] {
        let descriptor = sentence
            .registry
            .descriptor(schema)
            .ok_or("Sentence descriptor")?
            .clone();
        doc.registry
            .register(schema.clone(), descriptor, &mut budget())
            .map_err(err)?;
    }
    others.push(sentence.package);
    doc.registry.finalize(&mut budget()).map_err(err)?;
    Ok(Compiled { doc, others })
}
