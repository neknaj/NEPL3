//! Borrowed semantic views over the existing validated syntax graph.
//! No alternate parser, source slicing or copied expression tree is involved.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::Span,
    syntax::{FieldValue, NodeRef, SyntaxBundle},
    value::{Integer, NdfValue, SchemaRef},
};
use nepl3_engine::tree::ValidatedParseTree;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    MiniExpr,
    Frame,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Recovered,
    Schema,
    Shape,
    Reference,
    Stopped(StopReason),
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// A cursor can only originate from a validated tree or one of its child fields.
#[derive(Clone, Copy)]
pub struct Cursor<'a> {
    bundle: &'a SyntaxBundle,
    node: NodeRef,
    language: Language,
    expr_schema: &'a SchemaRef,
    frame_schema: &'a SchemaRef,
}

pub enum Expression<'a> {
    Natural(&'a Integer),
    Neg(Cursor<'a>),
    Add(Cursor<'a>, Cursor<'a>),
    Mul(Cursor<'a>, Cursor<'a>),
    /// MiniExpr delegates a foreign value to Frame's operation.
    Framed(Cursor<'a>),
    /// Frame delegates its expression to MiniExpr's operation.
    Frame(Cursor<'a>),
}

pub struct Step<'a> {
    pub expression: Expression<'a>,
    pub head: Option<&'a Span>,
    pub language: Language,
}

impl<'a> Cursor<'a> {
    pub fn root(
        proof: &'a ValidatedParseTree<'a>,
        expr_schema: &'a SchemaRef,
        frame_schema: &'a SchemaRef,
        budget: &mut Budget,
    ) -> Result<Self, Error> {
        budget.charge(Resource::Work, proof.tree().recovery.len() as u64 + 1)?;
        if proof.is_recovered() {
            return Err(Error::Recovered);
        }
        let bundle = &proof.tree().bundle;
        Ok(Self {
            bundle,
            node: bundle.root,
            language: Language::MiniExpr,
            expr_schema,
            frame_schema,
        })
    }

    /// Classify a single language-owned node without evaluating foreign syntax.
    /// Host-selected complete schema identities bind these semantics to packages.
    pub fn step(self, budget: &mut Budget) -> Result<Step<'a>, Error> {
        budget.poll()?;
        let node = usize::try_from(self.node.0)
            .ok()
            .and_then(|i| self.bundle.nodes.get(i))
            .ok_or(Error::Reference)?;
        let expected = match self.language {
            Language::MiniExpr => self.expr_schema,
            Language::Frame => self.frame_schema,
        };
        budget.charge(
            Resource::Work,
            (node.schema.package.len() + expected.package.len() + node.kind.len()) as u64 + 40,
        )?;
        if &node.schema != expected {
            return Err(Error::Schema);
        }
        let child = |node| Self { node, ..self };
        let expression = match (self.language, node.kind.as_str(), node.fields.as_slice()) {
            (Language::MiniExpr, "Natural", []) => {
                let token = node
                    .token
                    .and_then(|t| usize::try_from(t.0).ok())
                    .and_then(|i| self.bundle.tokens.get(i))
                    .ok_or(Error::Reference)?;
                let NdfValue::Integer(value) = &token.payload else {
                    return Err(Error::Shape);
                };
                if value.is_negative() {
                    return Err(Error::Shape);
                }
                Expression::Natural(value)
            }
            (Language::MiniExpr, "Neg", [FieldValue::Child(value)]) => {
                Expression::Neg(child(*value))
            }
            (Language::MiniExpr, "Add", [FieldValue::Child(left), FieldValue::Child(right)]) => {
                Expression::Add(child(*left), child(*right))
            }
            (Language::MiniExpr, "Mul", [FieldValue::Child(left), FieldValue::Child(right)]) => {
                Expression::Mul(child(*left), child(*right))
            }
            (Language::MiniExpr, "Framed", [FieldValue::Foreign(value)]) => {
                Expression::Framed(Self {
                    bundle: &value.bundle,
                    node: value.root,
                    language: Language::Frame,
                    ..self
                })
            }
            (Language::Frame, "Frame", [FieldValue::Foreign(value)]) => Expression::Frame(Self {
                bundle: &value.bundle,
                node: value.root,
                language: Language::MiniExpr,
                ..self
            }),
            _ => return Err(Error::Shape),
        };
        Ok(Step {
            expression,
            head: node.head.as_ref(),
            language: self.language,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use external_hello_language::{budget, composition, error};
    use nepl3_core::source::{Digest, SourceAdmission};
    use nepl3_engine::{parse::ParseOutcome, profile::RuntimeCatalog};

    #[test]
    fn borrowed_views_follow_language_boundaries_and_keep_inner_span() -> Result<(), String> {
        let languages = composition::languages("Expr", "Frame")?;
        let profile = languages.profile("composition")?;
        let packages = languages
            .packages
            .iter()
            .map(|(_, p)| p)
            .collect::<Vec<_>>();
        let resolved = profile
            .resolve(
                &RuntimeCatalog {
                    packages: &packages,
                    providers: &[],
                    resources: &[],
                },
                &languages.registry,
                &mut budget(),
            )
            .map_err(error)?;
        let observation = composition::inspect("add framed frame neg 7 2", true)?;
        let ParseOutcome::Complete { tree, .. } = observation.parse.outcome else {
            return Err("complete".into());
        };
        let proof = tree
            .validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .map_err(error)?;
        let root = Cursor::root(
            &proof,
            &packages[0].schema,
            &packages[1].schema,
            &mut budget(),
        )
        .map_err(error)?;
        use crate::program::{self, Instruction, ValueId};
        let plan = program::compile(root, &mut budget()).map_err(error)?;
        assert_eq!(plan.nodes().len(), 6);
        assert_eq!(plan.root(), ValueId(5));
        assert!(matches!(
            plan.nodes()[1].instruction,
            Instruction::Neg(ValueId(0))
        ));
        assert!(matches!(
            plan.nodes()[2].instruction,
            Instruction::Frame(ValueId(1))
        ));
        assert!(matches!(
            plan.nodes()[3].instruction,
            Instruction::Framed(ValueId(2))
        ));
        assert!(matches!(
            plan.nodes()[5].instruction,
            Instruction::Add(ValueId(3), ValueId(4))
        ));
        assert_eq!(plan.nodes()[2].language, Language::Frame);
        let origin = plan.nodes()[0].head.ok_or("planned leaf span")?;
        assert_eq!((origin.start(), origin.end()), (21, 22));
        {
            use crate::program::transfer;
            use nepl3_core::{schema::SchemaRegistry, value::TypedValue};
            let descriptor = transfer::descriptor(&mut budget()).map_err(error)?;
            let identity = descriptor.reference(&mut budget()).map_err(error)?;
            let mut registry = SchemaRegistry::default();
            registry
                .register(identity.clone(), descriptor, &mut budget())
                .map_err(error)?;
            registry.finalize(&mut budget()).map_err(error)?;
            let encoded = transfer::encode(&plan, &identity, &mut budget()).map_err(error)?;
            let checked =
                transfer::validate(&encoded, &identity, &registry, &mut budget()).map_err(error)?;
            assert_eq!(checked.nodes().len(), 6);
            let NdfValue::Variant(leaf) = &checked.nodes()[0] else {
                return Err("transferred leaf".into());
            };
            assert_eq!(leaf.variant, "Natural");
            assert_eq!(leaf.fields, vec![NdfValue::Integer(Integer::from(7_i64))]);
            // Cycles, duplicate occurrence IDs, foreign-language confusion and
            // disconnected subtrees remain structurally valid schema values.
            // The domain validator must reject all four independently.
            for (node_index, child_index, child) in [(1, 0, 1), (5, 1, 3), (3, 0, 1), (5, 0, 0)] {
                let mut changed = encoded.clone_with_budget(&mut budget()).map_err(error)?;
                let TypedValue::Record(record) = &mut changed else {
                    return Err("plan record".into());
                };
                let [NdfValue::List(nodes)] = record.fields.as_mut_slice() else {
                    return Err("plan list".into());
                };
                let NdfValue::Variant(node) = &mut nodes[node_index] else {
                    return Err("plan node".into());
                };
                node.fields[child_index] = NdfValue::U64(child);
                registry
                    .validate_typed(&changed, &mut budget())
                    .map_err(error)?;
                assert!(matches!(
                    transfer::validate(&changed, &identity, &registry, &mut budget()),
                    Err(transfer::Error::Reference)
                ));
            }
            let mut wrong_identity = identity.clone();
            wrong_identity.digest = Digest::of(b"wrong plan schema");
            assert!(matches!(
                transfer::validate(&encoded, &wrong_identity, &registry, &mut budget()),
                Err(transfer::Error::Shape)
            ));
            let mut limits = budget().limits();
            limits.allocation_units = 0;
            assert!(matches!(
                transfer::encode(&plan, &identity, &mut Budget::new(limits)),
                Err(transfer::Error::Stopped(StopReason::AllocationLimit))
            ));
            let mut stopped = budget();
            stopped.stop(StopReason::Cancelled);
            assert!(matches!(
                transfer::validate(&encoded, &identity, &registry, &mut stopped),
                Err(transfer::Error::Stopped(StopReason::Cancelled))
            ));
        }
        {
            use crate::execution::{self, Runtime};
            use nepl3_core::{
                diagnostic::OperationResult, schema::SchemaRegistry, source::SourceStore,
                value::TypedValue,
            };
            let mut registry = SchemaRegistry::default();
            let runtime = Runtime::register(
                &mut registry,
                [
                    Digest::of(b"MiniExpr evaluator v1"),
                    Digest::of(b"Frame evaluator v1"),
                ],
                &mut budget(),
            )
            .map_err(error)?;
            registry.finalize(&mut budget()).map_err(error)?;
            let mut sources = SourceStore::default();
            for source in &tree.bundle.sources {
                sources
                    .insert_ref_with_budget(source, &mut budget())
                    .map_err(error)?;
            }
            let mut awaits = Vec::new();
            let mut cancellations = Vec::new();
            let result = runtime
                .run(
                    &plan,
                    &sources,
                    &registry,
                    &mut budget(),
                    &mut budget(),
                    |id, _| awaits.push(id),
                    |id| cancellations.push(id),
                )
                .map_err(error)?;
            let OperationResult::Complete {
                value: TypedValue::Record(result),
                ..
            } = result
            else {
                return Err("completed composition evaluation".into());
            };
            // MiniExpr Add -> Framed -> Frame -> MiniExpr Neg, followed by 2:
            // (-7) + 2 = -5. Every non-leaf traverses an Await/Resume boundary.
            assert_eq!(
                result.fields,
                vec![NdfValue::Integer(Integer::from(-5_i64))]
            );
            assert_eq!(awaits, vec![6, 4, 3, 2]);
            assert!(cancellations.is_empty());
            assert!(matches!(
                runtime.run(
                    &plan,
                    &SourceStore::default(),
                    &registry,
                    &mut budget(),
                    &mut budget(),
                    |_, _| {},
                    |_| {}
                ),
                Err(execution::Error::Source(_))
            ));
            let mut stopped = budget();
            stopped.stop(StopReason::Cancelled);
            assert!(matches!(
                runtime.run(
                    &plan,
                    &sources,
                    &registry,
                    &mut stopped,
                    &mut budget(),
                    |_, _| {},
                    |_| {}
                ),
                Err(execution::Error::Execution(_))
            ));
        }
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::NodeLimit,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::DepthLimit => limits.depth = 2,
                StopReason::NodeLimit => limits.nodes = 0,
                _ => return Err("unexpected test resource".into()),
            }
            assert!(
                matches!(program::compile(root, &mut Budget::new(limits)), Err(Error::Stopped(actual)) if actual == reason)
            );
        }
        let Expression::Add(left, right) = root.step(&mut budget()).map_err(error)?.expression
        else {
            return Err("add".into());
        };
        let Expression::Natural(value) = right.step(&mut budget()).map_err(error)?.expression
        else {
            return Err("right leaf".into());
        };
        assert_eq!(*value, Integer::from(2_i64));
        let Expression::Framed(frame) = left.step(&mut budget()).map_err(error)?.expression else {
            return Err("framed".into());
        };
        let step = frame.step(&mut budget()).map_err(error)?;
        assert_eq!(step.language, Language::Frame);
        let Expression::Frame(expr) = step.expression else {
            return Err("frame".into());
        };
        let Expression::Neg(leaf) = expr.step(&mut budget()).map_err(error)?.expression else {
            return Err("neg".into());
        };
        let step = leaf.step(&mut budget()).map_err(error)?;
        let Expression::Natural(value) = step.expression else {
            return Err("inner leaf".into());
        };
        assert_eq!(*value, Integer::from(7_i64));
        let span = step.head.ok_or("inner span")?;
        assert_eq!((span.start(), span.end()), (21, 22));
        let mut wrong = packages[0].schema.clone();
        wrong.digest = Digest::of(b"another schema");
        let forged =
            Cursor::root(&proof, &wrong, &packages[1].schema, &mut budget()).map_err(error)?;
        assert!(matches!(forged.step(&mut budget()), Err(Error::Schema)));
        let mut stopped = budget();
        stopped.stop(StopReason::Cancelled);
        assert!(matches!(
            root.step(&mut stopped),
            Err(Error::Stopped(StopReason::Cancelled))
        ));
        let recovered = composition::inspect("framed frame", true)?;
        let ParseOutcome::Recovered { tree, .. } = recovered.parse.outcome else {
            return Err("expected recovered missing guest expression".into());
        };
        let proof = tree
            .validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .map_err(error)?;
        assert!(matches!(
            Cursor::root(
                &proof,
                &packages[0].schema,
                &packages[1].schema,
                &mut budget()
            ),
            Err(Error::Recovered)
        ));
        Ok(())
    }
}
