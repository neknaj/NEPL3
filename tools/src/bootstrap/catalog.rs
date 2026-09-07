//! Standard descriptor registrations used by first-seed and production-AST compilation.
//! A known facts signature is not an executable facts implementation.
use nepl3_core::{
    budget::Budget,
    schema::{SchemaRegistry, TypeDescriptor},
    source::SourceAdmission,
    value::OperationRef,
    view::{FallbackRole, PresentationClass},
};
use nepl3_engine::package::ExtensionRequirement;
use nepl3_grammar_core::{
    compile::{
        self, NamedClass, ReaderImport,
        package::{CompiledLanguage, PackageContext},
    },
    model::Document,
};
use nepl3_reader::builtin::{BuiltinReader, provider};
pub fn compile(
    document: &Document,
    package: &str,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<CompiledLanguage, String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(budget),
        nepl3_reader::schema::descriptor(budget),
        nepl3_engine::schema::descriptor(budget),
        nepl3_grammar_core::schema::descriptor(budget),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let reference = descriptor.reference(budget).map_err(|e| format!("{e:?}"))?;
        registry
            .register(reference, descriptor, budget)
            .map_err(|e| format!("{e:?}"))?;
    }
    registry.finalize(budget).map_err(|e| format!("{e:?}"))?;
    let mut imports = Vec::new();
    let mut extensions = Vec::new();
    for (name, kind) in [
        ("nepl3.reader/name-v1", BuiltinReader::Name),
        ("nepl3.reader/trivia-v1", BuiltinReader::Trivia),
        ("nepl3.reader/number-v1", BuiltinReader::Number),
    ] {
        let signature =
            provider::signature(kind, &registry, budget).map_err(|e| format!("{e:?}"))?;
        extensions.push(extension(
            &registry,
            name,
            "reader/v1",
            signature.operation.clone(),
        )?);
        imports.push(ReaderImport {
            provider: name.into(),
            signature,
        });
    }
    let engine = registry
        .selected("nepl3.engine", 1)
        .ok_or("engine schema")?
        .clone();
    extensions.push(extension(
        &registry,
        "grammar.facts/v1",
        "facts/v1",
        OperationRef {
            schema: engine,
            name: "bindingFacts".into(),
        },
    )?);
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation schema")?
        .clone();
    let classes = [
        ("content", FallbackRole::Content),
        ("marker", FallbackRole::Marker),
        ("delimiter", FallbackRole::Delimiter),
        ("name", FallbackRole::Name),
        ("quantity", FallbackRole::Quantity),
        ("annotation", FallbackRole::Annotation),
    ]
    .into_iter()
    .map(|(name, fallback)| NamedClass {
        name: name.into(),
        class: PresentationClass {
            schema: foundation.clone(),
            name: name.into(),
            fallback,
        },
    })
    .collect::<Vec<_>>();
    let checked = document
        .validate(budget, admission)
        .map_err(|e| format!("{e:?}"))?;
    compile::package::compile_with_admission(
        &checked,
        &PackageContext {
            package,
            state_type: &TypeDescriptor::Unit,
            reader_imports: &imports,
            extensions: &extensions,
            classes: &classes,
            views: &[],
        },
        registry,
        budget,
        admission,
    )
    .map_err(|e| format!("{e:?}"))
}
fn extension(
    registry: &SchemaRegistry,
    provider: &str,
    signature: &str,
    operation: OperationRef,
) -> Result<ExtensionRequirement, String> {
    let descriptor = registry
        .descriptor(&operation.schema)
        .and_then(|d| d.operations.iter().find(|v| v.name == operation.name))
        .ok_or("operation descriptor")?;
    Ok(ExtensionRequirement {
        alias: provider.into(),
        provider: provider.into(),
        signature: signature.into(),
        operation: operation.clone(),
        input: descriptor.input.clone(),
        output: descriptor.output.clone(),
        pure: descriptor.pure,
    })
}
