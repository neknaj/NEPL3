//! Prefix source generation, independent of evaluation and Doc semantics.
mod output;
pub mod request;
use crate::{
    check::{ValidatedMathShape, edges},
    model::*,
    number,
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    syntax::ForeignClosure,
};
use output::Output;

/// The selected host prints the exact borrowed Doc Sentence closure. The host
/// must reparse/check its returned source against that closure before treating
/// the combined artifact as a roundtrip proof. Retained source is not a fallback.
pub trait GuestPrinter {
    type Error;
    fn print(&mut self, guest: &ForeignClosure, budget: &mut Budget)
    -> Result<String, Self::Error>;
}
#[derive(Debug, Eq, PartialEq)]
pub enum PrintError<E> {
    Stopped(StopReason),
    UnprintableName { node: u64 },
    Guest { embed: EmbedRef, error: E },
    EmptyGuest { embed: EmbedRef },
    InvalidState,
}
impl<E> From<StopReason> for PrintError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// No invented source identity: the host assigns a snapshot when saving/parsing.
pub use crate::model::MathSourceArtifact as SourceArtifact;
enum Task {
    Node(u64, u64),
    Children(u64, usize, u64),
}
fn push<E>(stack: &mut Vec<Task>, task: Task, b: &mut Budget) -> Result<(), PrintError<E>> {
    b.charge(Resource::Work, 1)?;
    if stack.len() == stack.capacity() {
        let capacity = stack
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(4))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(
            Resource::AllocationUnits,
            ((capacity - stack.capacity()) as u64)
                .saturating_mul(core::mem::size_of::<Task>() as u64),
        )?;
        b.charge(Resource::Work, stack.len() as u64)?;
        stack
            .try_reserve_exact(capacity - stack.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    stack.push(task);
    Ok(())
}
/// Emit every occurrence in field order using O(depth) explicit traversal
/// storage. Shared nodes are expanded, not memoized. Work is proportional to
/// occurrences plus text/number/guest conversion costs; budget bounds expansion.
/// This preserves notation and never invokes Math or Doc evaluation.
pub fn prefix<G: GuestPrinter>(
    input: &ValidatedMathShape<'_>,
    guests: &mut G,
    b: &mut Budget,
) -> Result<SourceArtifact, PrintError<G::Error>> {
    b.poll()?;
    let value = input.value();
    let (root, entry) = edges::root(value.root);
    let base = b.current_depth();
    let mut stack = Vec::new();
    let mut out = Output::default();
    push(&mut stack, Task::Node(root, 1), b)?;
    while let Some(task) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        let (id, depth) = match task {
            Task::Node(id, d) | Task::Children(id, _, d) => (id, d),
        };
        let node = usize::try_from(id)
            .ok()
            .and_then(|i| value.nodes.get(i))
            .ok_or(PrintError::InvalidState)?;
        b.observe_depth(depth)?;
        if let Task::Children(_, next, _) = task {
            let list = matches!(
                node.kind,
                MathKind::Sequence { .. }
                    | MathKind::Vector { .. }
                    | MathKind::Matrix { .. }
                    | MathKind::Row { .. }
            );
            let call = matches!(node.kind, MathKind::Call { .. });
            if let Some((child, _)) = edges::edge(&node.kind, next) {
                if list || (call && next > 0) {
                    out.atom("cons", b)?;
                }
                push(&mut stack, Task::Children(id, next + 1, depth), b)?;
                push(
                    &mut stack,
                    Task::Node(
                        child,
                        depth
                            .checked_add(1)
                            .ok_or_else(|| b.stop(StopReason::DepthLimit))?,
                    ),
                    b,
                )?;
            } else if list || call {
                out.atom("nil", b)?;
            }
            continue;
        }
        b.charge(Resource::Nodes, 1)?;
        use MathKind::*;
        match &node.kind {
            Number { value, .. } => {
                out.start(b)?;
                let text = number::print_decimal(value, b).map_err(|e| match e {
                    number::DecimalPrintError::Stopped(r) => PrintError::Stopped(r),
                    _ => PrintError::InvalidState,
                })?;
                out.precharged(&text, b)?;
                continue;
            }
            Symbol { name } => {
                out.atom("symbol", b)?;
                out.quoted(name, b)?;
                continue;
            }
            Text { text } => {
                out.atom("text", b)?;
                out.quoted(text, b)?;
                continue;
            }
            DocGuest { syntax } => {
                let guest = usize::try_from(syntax.0)
                    .ok()
                    .and_then(|i| value.embeds.get(i))
                    .ok_or(PrintError::InvalidState)?;
                out.atom("Doc", b)?;
                let absolute_depth = base
                    .checked_add(depth)
                    .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
                let text = b.with_depth_at_least(absolute_depth, |b| {
                    let result = guests.print(guest, b);
                    b.poll()?;
                    result.map_err(|error| PrintError::Guest {
                        embed: *syntax,
                        error,
                    })
                })?;
                b.charge(Resource::Work, text.len() as u64)?;
                if text.trim().is_empty() {
                    return Err(PrintError::EmptyGuest { embed: *syntax });
                }
                out.atom(&text, b)?;
                continue;
            }
            _ => {}
        }
        let head = match &node.kind {
            Add { .. } => "add",
            Sub { .. } => "sub",
            Mul { .. } => "mul",
            Frac { .. } => "frac",
            Pow { .. } => "pow",
            Equal { .. } => "equal",
            Lt { .. } => "lt",
            Le { .. } => "le",
            Subscript { .. } => "subscript",
            Superscript { .. } => "superscript",
            Neg { .. } => "neg",
            Sqrt { .. } => "sqrt",
            Transpose { .. } => "transpose",
            Det { .. } => "det",
            Root { .. } => "root",
            Scripts { .. } => "scripts",
            Fence { .. } => "fence",
            Sequence { .. } => "sequence",
            Vector { .. } => "vector",
            Matrix { .. } => "matrix",
            Let { .. } => "let",
            Sum { .. } => "sum",
            Integral { .. } => "integral",
            Call { .. } => "call",
            Label { .. } => "label",
            Row { .. } => "row",
            _ => return Err(PrintError::InvalidState),
        };
        out.atom(head, b)?;
        match &node.kind {
            Fence { open, close, .. } => {
                out.quoted(open, b)?;
                out.quoted(close, b)?;
            }
            Let { name, .. } | Sum { index: name, .. } | Integral { index: name, .. } => {
                if !nepl3_core::lexical::name(name, b)? {
                    return Err(PrintError::UnprintableName { node: id });
                }
                out.atom(name, b)?;
            }
            _ => {}
        }
        push(&mut stack, Task::Children(id, 0, depth), b)?;
    }
    Ok(SourceArtifact {
        text: out.text,
        entry,
    })
}
