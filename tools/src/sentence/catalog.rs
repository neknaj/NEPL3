//! Independent Sentence surface assembly; no Doc/Math or annotation registry is needed.
//! A known facts signature is not an executable facts implementation.
#[cfg(test)]
mod tests;
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
/// Compile the standard Sentence source with the production Grammar reader.
/// `implementation` identifies the explicitly selected bootstrap host readers.
pub fn standard(
    implementation: nepl3_core::source::Digest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<CompiledLanguage, String> {
    standard_with_foreign_forms(implementation, &[], budget, admission)
}
/// Assemble the selected host surface with explicit typed foreign declarations.
pub fn standard_with_foreign_forms(
    implementation: nepl3_core::source::Digest,
    forms: &[compile::package::ForeignForm<'_>],
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<CompiledLanguage, String> {
    let document = standard_document(implementation, budget, admission)?;
    compile_with_foreign_forms(&document, "nepl3.syntax.sentence", forms, budget, admission)
}
fn standard_document(
    implementation: nepl3_core::source::Digest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Document, String> {
    use nepl3_core::source::{SourceId, SourceSnapshot};
    let err = |error| format!("{error:?}");
    let seed = crate::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/doc/grammar.json"),
        budget,
        admission,
    )
    .map_err(err)?;
    let grammar =
        crate::bootstrap::catalog::compile(&seed, "nepl3.syntax.grammar", budget, admission)?;
    let source = SourceSnapshot::new(
        SourceId("sentence-grammar".into()),
        1,
        "repository:languages/Sentence/syntax.neplg".into(),
        include_bytes!("../../../languages/Sentence/syntax.neplg").to_vec(),
        budget,
    )
    .map_err(|e| format!("{e:?}"))?;
    let document =
        crate::bootstrap::runtime::parse(&source, &grammar, implementation, budget, admission)
            .map_err(|e| format!("{e:?}"))?;
    Ok(document)
}

pub fn compile(
    document: &Document,
    package: &str,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<CompiledLanguage, String> {
    compile_with_foreign_forms(document, package, &[], budget, admission)
}
fn compile_with_foreign_forms(
    document: &Document,
    package: &str,
    forms: &[compile::package::ForeignForm<'_>],
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<CompiledLanguage, String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(budget),
        nepl3_reader::schema::descriptor(budget),
        nepl3_engine::schema::descriptor(budget),
        nepl3_grammar_core::schema::descriptor(budget),
        nepl3_sentence_core::schema::descriptor(budget),
        super::reader::descriptor(budget),
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
    let word = provider::signature(BuiltinReader::Name, &registry, budget)
        .map_err(|e| format!("{e:?}"))?;
    let literal = super::reader::signature(&registry, budget).map_err(|e| format!("{e:?}"))?;
    for (name, signature) in [
        ("nepl3.reader/name-v1", word),
        ("sentence.reader/literal-v1", literal),
    ] {
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
    compile::package::compile_with_foreign_forms(
        &checked,
        &PackageContext {
            package,
            state_type: &TypeDescriptor::Unit,
            reader_imports: &imports,
            extensions: &extensions,
            classes: &classes,
            views: &[],
        },
        forms,
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
