//! Closed visual tree for generated Math, separate from author-controlled HTML.
//! This is a native validation boundary, not a renderer authenticity, fidelity,
//! resource-availability or portable transport proof.
use super::{computed_style, length, path_data, view_box};
use crate::text::is_xml_character;
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
mod serialize;
pub use serialize::{Rendered, serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Aspect {
    None,
    MinSlice,
    MidSlice,
    MaxSlice,
}
impl Aspect {
    fn value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MinSlice => "xMinYMin slice",
            Self::MidSlice => "xMidYMin slice",
            Self::MaxSlice => "xMaxYMin slice",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Text(String),
    Span {
        classes: String,
        style: String,
        aria_hidden: Option<bool>,
        children: Vec<u64>,
    },
    Svg {
        width: String,
        height: String,
        view_box: Option<String>,
        aspect: Option<Aspect>,
        children: Vec<u64>,
    },
    Path {
        data: String,
    },
    Line {
        x1: Endpoint,
        y1: Endpoint,
        x2: Endpoint,
        y2: Endpoint,
        stroke_width: String,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Zero,
    Full,
}
impl Endpoint {
    fn value(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::Full => "100%",
        }
    }
}
/// A postorder tree: the last node is the root, every other node has exactly
/// one parent, and children precede parents. Shared nodes must be expanded by
/// the producer; this representation makes occurrence identity unambiguous.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fragment {
    pub nodes: Vec<Node>,
}
/// Host-owned class inventory for the bound renderer stylesheet. Sorted unique
/// names are required. The host must bind these to the actual fixed asset bytes;
/// accepting an inventory here does not authenticate a stylesheet.
pub struct Policy<'a> {
    pub classes: &'a [&'a str],
    /// Unique within the containing artifact, starting with `nepl-math-`.
    pub scope: &'a str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Policy,
    Root,
    Reference { node: u64, child: u64 },
    Parent { node: u64 },
    Content { node: u64 },
    Attribute { node: u64 },
    Text { node: u64, byte: u64 },
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct Checked<'a> {
    fragment: &'a Fragment,
    scope: &'a str,
}
impl Checked<'_> {
    pub fn fragment(&self) -> &Fragment {
        self.fragment
    }
}
fn identifier(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}
fn scratch<T: Clone>(n: usize, value: T, budget: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = n
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, bytes as u64)?;
    budget.charge(Resource::Work, n as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(n)
        .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    values.resize(n, value);
    Ok(values)
}
/// Validate the exact immutable visual tree and stylesheet class inventory.
/// Time O(N + E + B log(C+1)), space O(N), where B is attribute/text bytes and
/// C registered classes. Depth includes a generated outer span. All scans and
/// scratch allocations consume the caller's sticky budget before use.
pub fn validate<'a>(
    fragment: &'a Fragment,
    policy: &Policy<'a>,
    budget: &mut Budget,
) -> Result<Checked<'a>, Error> {
    budget.poll()?;
    budget.charge(Resource::Work, policy.scope.len() as u64 + 1)?;
    if !identifier(policy.scope)
        || !policy
            .scope
            .strip_prefix("nepl-math-")
            .is_some_and(|s| !s.is_empty())
    {
        return Err(Error::Policy);
    }
    let mut previous: Option<&str> = None;
    for class in policy.classes {
        budget.charge(Resource::Work, class.len() as u64 * 2 + 1)?;
        if !identifier(class)
            || class.starts_with("nepl-math-")
            || previous.is_some_and(|p| p >= *class)
        {
            return Err(Error::Policy);
        }
        previous = Some(class);
    }
    if !matches!(fragment.nodes.last(), Some(Node::Span { .. })) {
        return Err(Error::Root);
    }
    let n = fragment.nodes.len();
    let mut parents = scratch(n, false, budget)?;
    let mut heights = scratch(n, 1_u64, budget)?;
    for (index, node) in fragment.nodes.iter().enumerate() {
        budget.charge(Resource::Nodes, 1)?;
        budget.charge(Resource::Work, 1)?;
        let id = index as u64;
        let mut children = None;
        let valid = match node {
            Node::Text(text) => {
                budget.charge(Resource::Work, text.len() as u64)?;
                for (byte, scalar) in text.char_indices() {
                    if !is_xml_character(scalar) {
                        return Err(Error::Text {
                            node: id,
                            byte: byte as u64,
                        });
                    }
                }
                true
            }
            Node::Span {
                classes,
                style,
                children: edges,
                ..
            } => {
                let factor = u64::from(usize::BITS - policy.classes.len().leading_zeros()) + 2;
                let work = (classes.len() as u64)
                    .checked_mul(factor)
                    .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
                budget.charge(Resource::Work, work)?;
                children = Some(edges);
                (classes.is_empty()
                    || classes
                        .split(' ')
                        .all(|c| policy.classes.binary_search(&c).is_ok()))
                    && computed_style(style, budget)?
            }
            Node::Svg {
                width,
                height,
                view_box: viewport,
                aspect,
                children: edges,
            } => {
                budget.charge(Resource::Work, (width.len() + height.len()) as u64)?;
                children = Some(edges);
                (width == "100%" || length(width, false))
                    && length(height, false)
                    && match viewport {
                        Some(value) => view_box(value, budget)?,
                        None => aspect.is_none(),
                    }
            }
            Node::Path { data } => path_data(data, budget)?,
            Node::Line { stroke_width, .. } => {
                budget.charge(Resource::Work, stroke_width.len() as u64)?;
                length(stroke_width, false)
            }
        };
        if !valid {
            return Err(Error::Attribute { node: id });
        }
        if let Some(children) = children {
            for child in children {
                budget.charge(Resource::Work, 1)?;
                let child = usize::try_from(*child).ok().filter(|c| *c < index).ok_or(
                    Error::Reference {
                        node: id,
                        child: *child,
                    },
                )?;
                if parents[child] {
                    return Err(Error::Parent { node: child as u64 });
                }
                let content = match node {
                    Node::Span { .. } => matches!(
                        fragment.nodes[child],
                        Node::Span { .. } | Node::Text(_) | Node::Svg { .. }
                    ),
                    Node::Svg { .. } => {
                        matches!(fragment.nodes[child], Node::Path { .. } | Node::Line { .. })
                    }
                    _ => false,
                };
                if !content {
                    return Err(Error::Content { node: id });
                }
                parents[child] = true;
                heights[index] = heights[index].max(
                    heights[child]
                        .checked_add(1)
                        .ok_or_else(|| budget.stop(StopReason::DepthLimit))?,
                );
            }
        }
        let depth = heights[index]
            .checked_add(1)
            .ok_or_else(|| budget.stop(StopReason::DepthLimit))?;
        budget.observe_depth(depth)?;
    }
    for (index, used) in parents.iter().enumerate().take(n - 1) {
        budget.charge(Resource::Work, 1)?;
        if !used {
            return Err(Error::Parent { node: index as u64 });
        }
    }
    Ok(Checked {
        fragment,
        scope: policy.scope,
    })
}
