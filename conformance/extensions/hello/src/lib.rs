//! An independently owned syntax package using only public foundation APIs.
//! The host test adapter is intentionally std; production dependencies remain no_std.
use nepl3_core::{
    budget::{Budget, Limits},
    schema::*,
    value::KindRef,
};
use nepl3_engine::package::*;
use nepl3_reader::{builtin::BuiltinReader, plan::ReaderPlan, tokenizer::*};

pub fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 10_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
pub fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

/// `hello <name>` has exactly one name argument; no domain crate owns this form.
pub fn language() -> Result<(LanguagePackage, SchemaRegistry), String> {
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_reader::schema::descriptor(&mut b),
        nepl3_engine::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(error)?;
        let reference = descriptor.reference(&mut b).map_err(error)?;
        registry
            .register(reference, descriptor, &mut b)
            .map_err(error)?;
    }
    let descriptor = SchemaDescriptor {
        package: "org.example.hello".into(),
        revision: 1,
        operations: vec![],
        types: vec![
            NamedType {
                name: "Greeting".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "recipient".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "NodeRef".into(),
                        }),
                    }],
                },
            },
            NamedType {
                name: "Name".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "Word".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "payload".into(),
                        ty: TypeDescriptor::Text,
                    }],
                },
            },
        ],
    };
    let schema = descriptor.reference(&mut b).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut b)
        .map_err(error)?;
    registry.finalize(&mut b).map_err(error)?;
    let kind = |name: &str| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: schema.clone(),
            local_kind: registry.kind_id(&schema, name).map_err(error)?,
        })
    };
    let package = LanguagePackage {
        schema: schema.clone(),
        payload_schemas: vec![],
        root: "Greeting".into(),
        reader: ReaderPlan {
            schema: schema.clone(),
            state_type: TypeDescriptor::Unit,
            expressions: vec![],
            rules: vec![],
            providers: vec![],
        },
        modes: vec![ReaderMode {
            name: "Words".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Trivia),
            }],
            take: vec![TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: kind("Word")?,
            }],
        }],
        categories: vec![Category {
            name: "Greeting".into(),
            mode: "Words".into(),
        }],
        reads: vec![ReadSpec::Builtin {
            reader: BuiltinReader::Name,
            kind: kind("Name")?,
            token_kind: kind("Word")?,
        }],
        forms: vec![Form {
            category: "Greeting".into(),
            kind: kind("Greeting")?,
            spelling: "hello".into(),
            fields: vec![FieldSpec {
                name: "recipient".into(),
                read: ReadSpecId(0),
            }],
            binding: BindingId(0),
            selection_rules: vec![],
            styles: vec![],
        }],
        leaves: vec![],
        namespaces: vec![],
        bindings: vec![Binding::Group(vec![])],
        extensions: vec![],
        recovery: nepl3_engine::recovery::RecoveryPlan {
            default_unexpected: nepl3_engine::recovery::UnexpectedPolicy::PreserveRemainder,
            rules: vec![],
        },
        provenance: PackageProvenance {
            sources: vec![],
            origins: vec![],
            source_maps: vec![],
            declarations: vec![],
        },
    };
    package.check(&registry, &mut b).map_err(error)?;
    Ok((package, registry))
}

#[cfg(test)]
mod tests;
