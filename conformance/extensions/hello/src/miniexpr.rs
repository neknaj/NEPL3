//! A recursive expression language owned by this external consumer.
//! `neg`, `add`, and `mul` describe syntax; this example defines no evaluation.
use super::*;
use nepl3_core::source::SourceAdmission;
use nepl3_engine::parse::{ParseOutcome, ParseReply, print};

/// Parsing and source-backed printing are separate observable results.
#[derive(Debug, Eq, PartialEq)]
pub struct Observation {
    pub parse: ParseReply,
    /// Present only after a complete tree passes the package's structural check.
    /// The printer preserves lexemes/trivia and reports its own resource stops.
    pub printed: Option<print::PrintReply>,
}

pub fn inspect(input: &str, final_input: bool) -> Result<Observation, String> {
    super::parse::with_language(
        input,
        final_input,
        false,
        language()?,
        ("MiniExpr", "miniexpr"),
        |parse, resolved, _, _| {
            let printed = if let ParseOutcome::Complete { tree, .. } = &parse.outcome {
                let proof = tree
                    .validate(resolved, &mut budget(), &mut SourceAdmission::default())
                    .map_err(error)?;
                Some(
                    print::source_tree(&proof, &mut budget(), &mut SourceAdmission::default())
                        .map_err(error)?,
                )
            } else {
                None
            };
            Ok(Observation { parse, printed })
        },
    )
}

/// Natural-number leaves and prefix forms with one or two Expr children.
pub fn language() -> Result<(LanguagePackage, SchemaRegistry), String> {
    definition(None)
}

/// Construct the composition variant with an explicitly selected Frame alias.
pub(crate) fn definition(
    frame_alias: Option<&str>,
) -> Result<(LanguagePackage, SchemaRegistry), String> {
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
    let field = |name: &str| FieldDescriptor {
        name: name.into(),
        ty: TypeDescriptor::Named(TypeRef {
            package: "nepl3.foundation".into(),
            revision: 1,
            name: "NodeRef".into(),
        }),
    };
    let record = |name: &str, fields| NamedType {
        name: name.into(),
        constraints: vec![],
        shape: TypeShape::Record { fields },
    };
    let mut descriptor = SchemaDescriptor {
        package: "org.example.miniexpr".into(),
        revision: 1,
        operations: vec![],
        types: vec![
            record("Neg", vec![field("value")]),
            record("Add", vec![field("left"), field("right")]),
            record("Mul", vec![field("left"), field("right")]),
            record("Natural", vec![]),
            record(
                "Word",
                vec![FieldDescriptor {
                    name: "payload".into(),
                    ty: TypeDescriptor::Text,
                }],
            ),
            record(
                "Number",
                vec![FieldDescriptor {
                    name: "payload".into(),
                    ty: TypeDescriptor::Integer,
                }],
            ),
        ],
    };
    if frame_alias.is_some() {
        descriptor.package = "org.example.miniexpr.framed".into();
        descriptor.types.push(record(
            "Framed",
            vec![FieldDescriptor {
                name: "value".into(),
                ty: TypeDescriptor::Named(TypeRef {
                    package: "nepl3.foundation".into(),
                    revision: 1,
                    name: "ForeignSyntax".into(),
                }),
            }],
        ));
    }
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
    let form = |spelling: &str, name: &str, fields: &[&str], binding| -> Result<Form, String> {
        Ok(Form {
            category: "Expr".into(),
            kind: kind(name)?,
            spelling: spelling.into(),
            fields: fields
                .iter()
                .map(|name| FieldSpec {
                    name: (*name).into(),
                    read: ReadSpecId(0),
                })
                .collect(),
            binding,
            selection_rules: vec![],
            styles: vec![],
        })
    };
    let mut package = LanguagePackage {
        schema: schema.clone(),
        payload_schemas: vec![],
        root: "Expr".into(),
        reader: ReaderPlan {
            schema: schema.clone(),
            state_type: TypeDescriptor::Unit,
            expressions: vec![],
            rules: vec![],
            providers: vec![],
        },
        modes: vec![ReaderMode {
            name: "Code".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Builtin(BuiltinReader::Trivia),
            }],
            take: vec![
                TakeRule {
                    reader: TokenReader::Builtin(BuiltinReader::Nat),
                    kind: kind("Number")?,
                },
                TakeRule {
                    reader: TokenReader::Builtin(BuiltinReader::Name),
                    kind: kind("Word")?,
                },
            ],
        }],
        categories: vec![Category {
            name: "Expr".into(),
            mode: "Code".into(),
        }],
        reads: vec![ReadSpec::Local {
            category: "Expr".into(),
        }],
        forms: vec![
            form("neg", "Neg", &["value"], BindingId(1))?,
            form("add", "Add", &["left", "right"], BindingId(4))?,
            form("mul", "Mul", &["left", "right"], BindingId(4))?,
        ],
        leaves: vec![Leaf {
            category: "Expr".into(),
            kind: kind("Natural")?,
            token_kind: kind("Number")?,
            payload: TypeDescriptor::Integer,
            binding: BindingId(0),
            selection_rules: vec![],
            styles: vec![],
        }],
        namespaces: vec![],
        bindings: vec![
            Binding::Group(vec![]),
            Binding::Visit("value".into()),
            Binding::Visit("left".into()),
            Binding::Visit("right".into()),
            Binding::Group(vec![BindingId(2), BindingId(3)]),
        ],
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
    if let Some(alias) = frame_alias {
        package.reads.push(ReadSpec::Foreign {
            alias: alias.into(),
            category: "Frame".into(),
        });
        let mut framed = form("framed", "Framed", &["value"], BindingId(1))?;
        framed.fields[0].read = ReadSpecId(1);
        package.forms.push(framed);
    }
    package.check(&registry, &mut b).map_err(error)?;
    Ok((package, registry))
}

#[cfg(test)]
mod tests;
