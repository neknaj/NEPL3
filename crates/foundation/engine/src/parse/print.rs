//! Source-backed structural output for one complete expression.
//!
//! This traverses actual child fields and each token's retained lexeme/trivia.
//! It does not concatenate arbitrary tokens with invented whitespace, copy the
//! root source range, normalize semantic values, or append host-owned trailing trivia.
use crate::tree::ValidatedParseTree;
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::{SourceAdmission, SourceError, Span},
    syntax::{FieldValue, NodeRef, SyntaxBundle},
};
#[derive(Debug, Eq, PartialEq)]
pub enum PrintError {
    Recovered,
    Reference,
    Source(SourceError),
}
#[derive(Debug, Eq, PartialEq)]
pub enum PrintOutcome {
    Complete(String),
    Stopped { reason: StopReason, partial: String },
}
#[derive(Debug, Eq, PartialEq)]
pub struct PrintReply {
    pub outcome: PrintOutcome,
    pub report: Report,
}
struct Task<'a> {
    bundle: &'a SyntaxBundle,
    node: NodeRef,
    depth: u64,
}
fn push<'a>(
    tasks: &mut Vec<Task<'a>>,
    bundle: &'a SyntaxBundle,
    node: NodeRef,
    depth: u64,
    b: &mut Budget,
) -> Result<(), StopReason> {
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Task<'a>>() as u64,
    )?;
    tasks.push(Task {
        bundle,
        node,
        depth,
    });
    Ok(())
}
fn append(out: &mut String, text: &str, b: &mut Budget) -> Result<(), StopReason> {
    let len = text.len() as u64;
    b.charge(Resource::Work, len)?;
    b.charge(Resource::OutputBytes, len)?;
    b.charge(Resource::AllocationUnits, len)?;
    out.push_str(text);
    Ok(())
}
fn raw<'a>(bundle: &'a SyntaxBundle, span: &Span, b: &mut Budget) -> Result<&'a str, Failure> {
    b.charge(
        Resource::Work,
        bundle.sources.len() as u64 * (span.snapshot_ref().source.0.len() as u64 + 33),
    )?;
    let source = bundle
        .sources
        .iter()
        .find(|v| v.identity() == span.snapshot_ref())
        .ok_or(PrintError::Reference)?;
    source
        .slice(span)
        .map_err(|v| Failure::Invalid(PrintError::Source(v)))
}
enum Failure {
    Stopped(StopReason),
    Invalid(PrintError),
}
impl From<StopReason> for Failure {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<PrintError> for Failure {
    fn from(v: PrintError) -> Self {
        Self::Invalid(v)
    }
}
/// Print a source-backed validated static tree, preserving individual lexemes and
/// accepted leading trivia. Recovery nodes are rejected; domain semantic printers
/// and whole-file trailing-trivia output are separate operations.
pub fn source_tree(
    proof: &ValidatedParseTree<'_>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<PrintReply, PrintError> {
    if proof.is_recovered() {
        return Err(PrintError::Recovered);
    }
    let tree = proof.tree();
    let mut output = String::new();
    let mut tasks = Vec::new();
    let result = (|| -> Result<(), Failure> {
        push(&mut tasks, &tree.bundle, tree.bundle.root, 1, budget)?;
        while let Some(Task {
            bundle,
            node,
            depth,
        }) = tasks.pop()
        {
            budget.poll()?;
            budget.observe_depth(depth)?;
            budget.charge(Resource::Work, 1)?;
            for source in &bundle.sources {
                admission
                    .admit_existing(source, budget)
                    .map_err(|e| match e {
                        SourceError::Stopped(v) => Failure::Stopped(v),
                        v => Failure::Invalid(PrintError::Source(v)),
                    })?;
            }
            let node = usize::try_from(node.0)
                .ok()
                .and_then(|id| bundle.nodes.get(id))
                .ok_or(PrintError::Reference)?;
            let token = node
                .token
                .and_then(|id| usize::try_from(id.0).ok())
                .and_then(|id| bundle.tokens.get(id))
                .ok_or(PrintError::Reference)?;
            for trivia in &token.leading_trivia {
                append(&mut output, raw(bundle, &trivia.span, budget)?, budget)?;
            }
            append(&mut output, raw(bundle, &token.head, budget)?, budget)?;
            let child_depth = depth
                .checked_add(1)
                .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
            for field in node.fields.iter().rev() {
                match field {
                    FieldValue::Child(id) => push(&mut tasks, bundle, *id, child_depth, budget)?,
                    FieldValue::Children(ids) => {
                        for id in ids.iter().rev() {
                            push(&mut tasks, bundle, *id, child_depth, budget)?;
                        }
                    }
                    FieldValue::Foreign(value) => {
                        push(&mut tasks, &value.bundle, value.root, child_depth, budget)?
                    }
                    FieldValue::Atom(_) => return Err(PrintError::Reference.into()),
                }
            }
        }
        Ok(())
    })();
    let outcome = match result {
        Ok(()) => PrintOutcome::Complete(output),
        Err(Failure::Stopped(reason)) => PrintOutcome::Stopped {
            reason,
            partial: output,
        },
        Err(Failure::Invalid(error)) => return Err(error),
    };
    Ok(PrintReply {
        outcome,
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })
}
