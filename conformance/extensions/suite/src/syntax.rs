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
