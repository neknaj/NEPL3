//! Explicit recursive composition of two independently identified packages.
//! MiniExpr `framed` enters Frame; Frame `frame` returns to MiniExpr.
use super::*;
use nepl3_core::source::SourceAdmission;
use nepl3_engine::parse::{ParseOutcome, print};

/// Observe a composed expression with source-backed printing after validation.
pub fn inspect(input: &str, final_input: bool) -> Result<miniexpr::Observation, String> {
    super::parse::with_languages(
        input,
        final_input,
        false,
        languages("Expr", "Frame")?,
        ("Expr", "composition"),
        |parse, profile, _, _| {
            let printed = if let ParseOutcome::Complete { tree, .. } = &parse.outcome {
                let proof = tree
                    .validate(profile, &mut budget(), &mut SourceAdmission::default())
                    .map_err(error)?;
                Some(
                    print::source_tree(&proof, &mut budget(), &mut SourceAdmission::default())
                        .map_err(error)?,
                )
            } else {
                None
            };
            Ok(miniexpr::Observation { parse, printed })
        },
    )
}

/// The host supplies aliases; each package declares its foreign category reads.
pub(crate) fn languages<'a>(
    expr_alias: &'a str,
    frame_alias: &'a str,
) -> Result<super::parse::Languages<'a>, String> {
    let (expr, mut registry) = miniexpr::definition(Some(frame_alias))?;
    let mut b = budget();
    let descriptor = SchemaDescriptor {
        package: "org.example.frame".into(),
        revision: 1,
        operations: vec![],
        types: vec![
            NamedType {
                name: "Frame".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "expression".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "ForeignSyntax".into(),
                        }),
                    }],
                },
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
    let kind = |name| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: schema.clone(),
            local_kind: registry.kind_id(&schema, name).map_err(error)?,
        })
    };
    let frame = LanguagePackage {
        schema: schema.clone(),
        payload_schemas: vec![],
        root: "Frame".into(),
        reader: ReaderPlan {
            schema: schema.clone(),
            state_type: TypeDescriptor::Unit,
            expressions: vec![],
            rules: vec![],
            providers: vec![],
        },
        modes: vec![ReaderMode {
            name: "FrameCode".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Trivia),
            }],
            take: vec![TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: kind("Word")?,
            }],
        }],
        categories: vec![Category {
            name: "Frame".into(),
            mode: "FrameCode".into(),
        }],
        reads: vec![ReadSpec::Foreign {
            alias: expr_alias.into(),
            category: "Expr".into(),
        }],
        forms: vec![Form {
            category: "Frame".into(),
            kind: kind("Frame")?,
            spelling: "frame".into(),
            fields: vec![FieldSpec {
                name: "expression".into(),
                read: ReadSpecId(0),
            }],
            binding: BindingId(0),
            selection_rules: vec![],
            styles: vec![],
        }],
        leaves: vec![],
        namespaces: vec![],
        bindings: vec![Binding::Visit("expression".into())],
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
    frame.check(&registry, &mut b).map_err(error)?;
    Ok(super::parse::Languages {
        packages: vec![(expr_alias, expr), (frame_alias, frame)],
        registry,
    })
}

#[cfg(test)]
mod tests;
