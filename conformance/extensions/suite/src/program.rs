//! Language-owned dependency plan over borrowed, validated syntax.
//! Child results precede consumers; foreign nodes remain explicit operations.
use crate::syntax::{Cursor, Error, Expression, Language};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::Span,
    value::Integer,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueId(pub usize);

#[derive(Debug)]
pub enum Instruction<'a> {
    Natural(&'a Integer),
    Neg(ValueId),
    Add(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Framed(ValueId),
    Frame(ValueId),
}

#[derive(Debug)]
pub struct Node<'a> {
    pub instruction: Instruction<'a>,
    pub language: Language,
    pub head: Option<&'a Span>,
}

/// Immutable plan. Numeric payloads and source spans borrow the original tree.
pub struct Program<'a> {
    nodes: Vec<Node<'a>>,
    root: ValueId,
}
impl<'a> Program<'a> {
    pub fn nodes(&self) -> &[Node<'a>] {
        &self.nodes
    }
    pub fn root(&self) -> ValueId {
        self.root
    }
}

#[derive(Clone, Copy)]
enum Operator {
    Neg,
    Add,
    Mul,
    Framed,
    Frame,
}
enum Task<'a> {
    Enter(Cursor<'a>, u64),
    Finish(Operator, Language, Option<&'a Span>),
}

fn push<T>(values: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), Error> {
    if values.len() == values.capacity() {
        let capacity = values
            .capacity()
            .max(1)
            .checked_mul(2)
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        let bytes = (capacity - values.capacity())
            .checked_mul(core::mem::size_of::<T>())
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        budget.charge(Resource::AllocationUnits, bytes as u64)?;
        budget.charge(Resource::Work, values.len() as u64)?;
        values
            .try_reserve_exact(capacity - values.len())
            .map_err(|_| Error::Stopped(budget.stop(StopReason::AllocationLimit)))?;
    }
    values.push(value);
    Ok(())
}

/// Compile each requested occurrence iteratively. Shared syntax reached through
/// two child positions produces two operations, each charged to this Budget.
/// No native recursion is used; logical nesting observes the Depth limit.
pub fn compile<'a>(root: Cursor<'a>, budget: &mut Budget) -> Result<Program<'a>, Error> {
    budget.poll()?;
    let mut tasks = Vec::new();
    let mut values = Vec::new();
    let mut nodes = Vec::new();
    push(&mut tasks, Task::Enter(root, 1), budget)?;
    while let Some(task) = tasks.pop() {
        budget.charge(Resource::Work, 1)?;
        let node = match task {
            Task::Enter(cursor, depth) => {
                budget.observe_depth(depth)?;
                let step = cursor.step(budget)?;
                let (operator, left, right) = match step.expression {
                    Expression::Natural(value) => {
                        budget.charge(Resource::Nodes, 1)?;
                        let id = ValueId(nodes.len());
                        push(
                            &mut nodes,
                            Node {
                                instruction: Instruction::Natural(value),
                                language: step.language,
                                head: step.head,
                            },
                            budget,
                        )?;
                        push(&mut values, id, budget)?;
                        continue;
                    }
                    Expression::Neg(value) => (Operator::Neg, value, None),
                    Expression::Add(left, right) => (Operator::Add, left, Some(right)),
                    Expression::Mul(left, right) => (Operator::Mul, left, Some(right)),
                    Expression::Framed(value) => (Operator::Framed, value, None),
                    Expression::Frame(value) => (Operator::Frame, value, None),
                };
                let next_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
                push(
                    &mut tasks,
                    Task::Finish(operator, step.language, step.head),
                    budget,
                )?;
                if let Some(right) = right {
                    push(&mut tasks, Task::Enter(right, next_depth), budget)?;
                }
                push(&mut tasks, Task::Enter(left, next_depth), budget)?;
                continue;
            }
            Task::Finish(operator, language, head) => {
                let last = values.pop().ok_or(Error::Shape)?;
                let instruction = match operator {
                    Operator::Neg => Instruction::Neg(last),
                    Operator::Add => Instruction::Add(values.pop().ok_or(Error::Shape)?, last),
                    Operator::Mul => Instruction::Mul(values.pop().ok_or(Error::Shape)?, last),
                    Operator::Framed => Instruction::Framed(last),
                    Operator::Frame => Instruction::Frame(last),
                };
                Node {
                    instruction,
                    language,
                    head,
                }
            }
        };
        budget.charge(Resource::Nodes, 1)?;
        let id = ValueId(nodes.len());
        push(&mut nodes, node, budget)?;
        push(&mut values, id, budget)?;
    }
    let [root] = values.as_slice() else {
        return Err(Error::Shape);
    };
    Ok(Program { nodes, root: *root })
}
