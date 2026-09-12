use super::*;
use crate::text::{TextContext, TextError, escape};
enum Action {
    Node(u64, u64),
    Children(u64, usize, u64),
}
fn push(stack: &mut Vec<Action>, action: Action, b: &mut Budget) -> Result<(), Error> {
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
                .saturating_mul(core::mem::size_of::<Action>() as u64),
        )?;
        b.charge(Resource::Work, stack.len() as u64)?;
        stack
            .try_reserve_exact(capacity - stack.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    stack.push(action);
    Ok(())
}
fn append(out: &mut String, text: &str, precharged: bool, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, text.len() as u64)?;
    if !precharged {
        b.charge(Resource::OutputBytes, text.len() as u64)?;
    }
    let len = out
        .len()
        .checked_add(text.len())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    if len > out.capacity() {
        let capacity = out.capacity().saturating_mul(2).max(len).max(32);
        b.charge(
            Resource::AllocationUnits,
            (capacity - out.capacity()) as u64,
        )?;
        b.charge(Resource::Work, out.len() as u64)?;
        out.try_reserve_exact(capacity - out.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    out.push_str(text);
    Ok(())
}
fn attr(a: &Attribute) -> (&'static str, &str) {
    fn boolean(v: bool) -> &'static str {
        if v { "true" } else { "false" }
    }
    match a {
        Attribute::Display(Display::Inline) => ("display", "inline"),
        Attribute::Display(Display::Block) => ("display", "block"),
        Attribute::NormalIdentifier => ("mathvariant", "normal"),
        Attribute::Stretchy(v) => ("stretchy", boolean(*v)),
        Attribute::Symmetric(v) => ("symmetric", boolean(*v)),
        Attribute::LargeOperator(v) => ("largeop", boolean(*v)),
        Attribute::MovableLimits(v) => ("movablelimits", boolean(*v)),
        Attribute::Form(v) => (
            "form",
            match v {
                OperatorForm::Prefix => "prefix",
                OperatorForm::Infix => "infix",
                OperatorForm::Postfix => "postfix",
            },
        ),
        Attribute::Width(v) => ("width", v),
        Attribute::Height(v) => ("height", v),
        Attribute::Depth(v) => ("depth", v),
    }
}
/// Serialize an immutable checked MathML tree with explicit namespace and
/// escaped text. O(emitted occurrences + bytes), O(depth) traversal storage.
/// Shared DAG nodes expand per occurrence; stopped output is never returned.
pub fn serialize(input: &Validated<'_>, b: &mut Budget) -> Result<String, Error> {
    b.poll()?;
    let f = input.fragment();
    let mut out = String::new();
    let base = b.current_depth();
    let mut stack = Vec::new();
    push(&mut stack, Action::Node(f.root, 1), b)?;
    while let Some(action) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        match action {
            Action::Node(id, depth) => {
                b.observe_depth(depth)?;
                if !matches!(f.nodes[index(id, f.nodes.len())?], Node::Html { .. }) {
                    b.charge(Resource::Nodes, 1)?;
                }
                match &f.nodes[index(id, f.nodes.len())?] {
                    Node::Html { fragment } => {
                        let html = b.with_depth_at_least(base.saturating_add(depth - 1), |b| {
                            crate::html::serialize::embedded(fragment, b)
                        })?;
                        append(&mut out, &html, true, b)?;
                    }
                    Node::Text(text) => {
                        let escaped =
                            escape(text, TextContext::Content, b).map_err(|e| match e {
                                TextError::Stopped(s) => Error::Stopped(s),
                                TextError::InvalidCharacter { byte, .. } => {
                                    Error::Text { node: id, byte }
                                }
                            })?;
                        append(&mut out, &escaped, true, b)?;
                    }
                    Node::Element {
                        tag, attributes, ..
                    } => {
                        append(&mut out, "<", false, b)?;
                        append(&mut out, tag.name(), false, b)?;
                        if *tag == Tag::Math {
                            append(
                                &mut out,
                                " xmlns=\"http://www.w3.org/1998/Math/MathML\"",
                                false,
                                b,
                            )?;
                        }
                        for a in attributes {
                            let (name, value) = attr(a);
                            append(&mut out, " ", false, b)?;
                            append(&mut out, name, false, b)?;
                            append(&mut out, "=\"", false, b)?;
                            append(&mut out, value, false, b)?;
                            append(&mut out, "\"", false, b)?;
                        }
                        append(&mut out, ">", false, b)?;
                        push(&mut stack, Action::Children(id, 0, depth), b)?;
                    }
                }
            }
            Action::Children(id, next, depth) => {
                let Node::Element { tag, children, .. } = &f.nodes[index(id, f.nodes.len())?]
                else {
                    return Err(Error::Content(id));
                };
                if let Some(child) = children.get(next) {
                    push(&mut stack, Action::Children(id, next + 1, depth), b)?;
                    push(&mut stack, Action::Node(*child, depth.saturating_add(1)), b)?;
                } else {
                    append(&mut out, "</", false, b)?;
                    append(&mut out, tag.name(), false, b)?;
                    append(&mut out, ">", false, b)?;
                }
            }
        }
    }
    Ok(out)
}
