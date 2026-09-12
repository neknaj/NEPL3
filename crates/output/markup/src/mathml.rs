//! Structural MathML profile from design/markup.json. This proof does not
//! certify layout, resource availability, or the meaning of a Math expression.
//! Mixed HTML inside mtext uses checked phrasing nodes, never raw markup.
//! The neutral structure codec is `portable::mathml`; it revalidates received
//! trees, including output-occurrence identities across HTML leaves.
//! These Rust types are not a portable ABI or a completed artifact contract.
//!
//! Validation borrows the exact immutable fragment. Serialization expands shared
//! nodes per appearance and returns no partial string on failure. Both operations
//! consume the caller's sticky budget; a separate serialization budget is only
//! appropriate for a separate invocation, not recovery from an exhausted one.
pub(crate) mod serialize;
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
pub use serialize::serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tag {
    Math,
    Row,
    Identifier,
    Number,
    Operator,
    Text,
    Fraction,
    Sqrt,
    Root,
    Sub,
    Sup,
    SubSup,
    Under,
    Over,
    UnderOver,
    Table,
    TableRow,
    Cell,
    Space,
}
impl Tag {
    pub fn name(self) -> &'static str {
        match self {
            Self::Math => "math",
            Self::Row => "mrow",
            Self::Identifier => "mi",
            Self::Number => "mn",
            Self::Operator => "mo",
            Self::Text => "mtext",
            Self::Fraction => "mfrac",
            Self::Sqrt => "msqrt",
            Self::Root => "mroot",
            Self::Sub => "msub",
            Self::Sup => "msup",
            Self::SubSup => "msubsup",
            Self::Under => "munder",
            Self::Over => "mover",
            Self::UnderOver => "munderover",
            Self::Table => "mtable",
            Self::TableRow => "mtr",
            Self::Cell => "mtd",
            Self::Space => "mspace",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Display {
    Inline,
    Block,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorForm {
    Prefix,
    Infix,
    Postfix,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Attribute {
    Display(Display),
    NormalIdentifier,
    Stretchy(bool),
    Symmetric(bool),
    LargeOperator(bool),
    MovableLimits(bool),
    Form(OperatorForm),
    Width(String),
    Height(String),
    Depth(String),
}
impl Attribute {
    pub(crate) fn key(&self) -> u8 {
        match self {
            Self::Display(_) => 0,
            Self::NormalIdentifier => 1,
            Self::Stretchy(_) => 2,
            Self::Symmetric(_) => 3,
            Self::LargeOperator(_) => 4,
            Self::MovableLimits(_) => 5,
            Self::Form(_) => 6,
            Self::Width(_) => 7,
            Self::Height(_) => 8,
            Self::Depth(_) => 9,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Node {
    Text(String),
    /// Only admitted immediately inside mtext, with the containing policy.
    Html {
        fragment: crate::html::HtmlFragment,
    },
    Element {
        tag: Tag,
        attributes: Vec<Attribute>,
        children: Vec<u64>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fragment {
    pub nodes: Vec<Node>,
    pub root: u64,
    pub html_policy: crate::html::HtmlPolicy,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Reference(u64),
    Cycle(u64),
    Unreachable(u64),
    Content(u64),
    Attribute { node: u64, index: u64 },
    Text { node: u64, byte: u64 },
    Html(crate::html::HtmlError),
}
impl From<crate::html::HtmlError> for Error {
    fn from(e: crate::html::HtmlError) -> Self {
        match e {
            crate::html::HtmlError::Stopped(s) => Self::Stopped(s),
            e => Self::Html(e),
        }
    }
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct Validated<'a> {
    fragment: &'a Fragment,
}
impl<'a> Validated<'a> {
    pub fn fragment(&self) -> &'a Fragment {
        self.fragment
    }
}

fn index(id: u64, count: usize) -> Result<usize, Error> {
    usize::try_from(id)
        .ok()
        .filter(|i| *i < count)
        .ok_or(Error::Reference(id))
}
fn length(s: &str) -> bool {
    let Some(n) = s.strip_suffix("em") else {
        return false;
    };
    let (whole, fraction) = match n.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (n, None),
    };
    !whole.is_empty()
        && whole.bytes().all(|b| b.is_ascii_digit())
        && (whole.len() == 1 || !whole.starts_with('0'))
        && fraction.is_none_or(|f| {
            !f.is_empty() && !f.ends_with('0') && f.bytes().all(|b| b.is_ascii_digit())
        })
}
pub(crate) fn attribute_allowed(
    tag: Tag,
    a: &Attribute,
    b: &mut Budget,
) -> Result<bool, StopReason> {
    Ok(match a {
        Attribute::Display(_) => tag == Tag::Math,
        Attribute::NormalIdentifier => tag == Tag::Identifier,
        Attribute::Width(s) | Attribute::Height(s) | Attribute::Depth(s) => {
            b.charge(Resource::Work, s.len() as u64)?;
            tag == Tag::Space && length(s)
        }
        _ => tag == Tag::Operator,
    })
}
pub(crate) fn arity(tag: Tag) -> Option<usize> {
    match tag {
        Tag::Fraction | Tag::Root | Tag::Sub | Tag::Sup | Tag::Under | Tag::Over => Some(2),
        Tag::SubSup | Tag::UnderOver => Some(3),
        Tag::Space => Some(0),
        _ => None,
    }
}
pub(crate) enum Child {
    Text,
    Html,
    Element(Tag),
}
pub(crate) fn accepts(tag: Tag, child: Child) -> bool {
    match tag {
        Tag::Text => matches!(child, Child::Text | Child::Html),
        Tag::Identifier | Tag::Number | Tag::Operator => matches!(child, Child::Text),
        Tag::Table => matches!(child, Child::Element(Tag::TableRow)),
        Tag::TableRow => matches!(child, Child::Element(Tag::Cell)),
        _ => {
            matches!(child, Child::Element(t) if !matches!(t, Tag::Math | Tag::TableRow | Tag::Cell))
        }
    }
}
fn local(fragment: &Fragment, id: usize, b: &mut Budget) -> Result<(), Error> {
    let Node::Element {
        tag,
        attributes,
        children,
    } = &fragment.nodes[id]
    else {
        if let Node::Text(text) = &fragment.nodes[id] {
            b.charge(Resource::Work, text.len() as u64)?;
            for (byte, c) in text.char_indices() {
                if !crate::text::is_xml_character(c) {
                    return Err(Error::Text {
                        node: id as u64,
                        byte: byte as u64,
                    });
                }
            }
        }
        return Ok(());
    };
    let mut keys = 0u16;
    for (i, a) in attributes.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let bit = 1u16 << a.key();
        let allowed = attribute_allowed(*tag, a, b)?;
        if keys & bit != 0 || !allowed {
            return Err(Error::Attribute {
                node: id as u64,
                index: i as u64,
            });
        }
        keys |= bit;
    }
    if arity(*tag).is_some_and(|n| children.len() != n) {
        return Err(Error::Content(id as u64));
    }
    for child in children {
        b.charge(Resource::Work, 1)?;
        let node = &fragment.nodes[index(*child, fragment.nodes.len())?];
        let good = accepts(
            *tag,
            match node {
                Node::Text(_) => Child::Text,
                Node::Html { .. } => Child::Html,
                Node::Element { tag, .. } => Child::Element(*tag),
            },
        );
        if !good {
            return Err(Error::Content(id as u64));
        }
    }
    Ok(())
}

/// Validate all reachable nodes, arities, attributes and shared-DAG depth.
/// Math-only validation is O(nodes + edges + text/attribute bytes). With HTML
/// leaves, additionally visit expanded occurrences and run HTML validation under
/// the same budget; shared DAGs may have exponentially many appearances.
/// O(nodes) Math traversal storage plus the HTML validator's identity/storage cost.
/// Only a root `math` establishes the namespace; no RawHtml is admitted here.
pub fn validate<'a>(fragment: &'a Fragment, b: &mut Budget) -> Result<Validated<'a>, Error> {
    b.poll()?;
    crate::html::check::check_policy(&fragment.html_policy, fragment.root, b)?;
    let count = fragment.nodes.len();
    let root = index(fragment.root, count)?;
    if !matches!(fragment.nodes[root], Node::Element { tag: Tag::Math, .. }) {
        return Err(Error::Content(fragment.root));
    }
    b.charge(Resource::Nodes, count as u64)?;
    b.charge(Resource::Work, count as u64)?;
    b.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(9 + core::mem::size_of::<(usize, usize)>() as u64),
    )?;
    let mut colors = Vec::new();
    let mut heights = Vec::new();
    let mut stack = Vec::new();
    colors
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    heights
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    stack
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    colors.resize(count, 0u8);
    heights.resize(count, 1u64);
    local(fragment, root, b)?;
    colors[root] = 1;
    stack.push((root, 0usize));
    while let Some((node, next)) = stack.last().copied() {
        b.charge(Resource::Work, 1)?;
        b.observe_depth(stack.len() as u64)?;
        let child = match &fragment.nodes[node] {
            Node::Text(_) | Node::Html { .. } => None,
            Node::Element { children, .. } => children.get(next).copied(),
        };
        if let Some(child) = child {
            let child = index(child, count)?;
            let frame = stack.last_mut().ok_or(Error::Reference(node as u64))?;
            frame.1 += 1;
            if colors[child] == 1 {
                return Err(Error::Cycle(child as u64));
            }
            if colors[child] == 0 {
                local(fragment, child, b)?;
                colors[child] = 1;
                stack.push((child, 0));
            } else {
                heights[node] = heights[node].max(heights[child].saturating_add(1));
                b.observe_depth((stack.len() as u64).saturating_add(heights[child]))?;
            }
        } else {
            colors[node] = 2;
            stack.pop();
            if let Some((parent, _)) = stack.last() {
                heights[*parent] = heights[*parent].max(heights[node].saturating_add(1));
            }
        }
    }
    for (i, color) in colors.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if *color != 2 {
            return Err(Error::Unreachable(i as u64));
        }
    }
    b.charge(Resource::Work, count as u64)?;
    if !fragment
        .nodes
        .iter()
        .any(|node| matches!(node, Node::Html { .. }))
    {
        return Ok(Validated { fragment });
    }
    // Reuse the bounded DFS stack for output occurrences: shared HTML carrying
    // an ID must not be accepted twice. Resolve links only after all leaves.
    let base = b.current_depth();
    let mut identity = crate::html::check::Identity::default();
    stack.push((root, 0));
    while let Some((node, next)) = stack.last().copied() {
        b.charge(Resource::Work, 1)?;
        match &fragment.nodes[node] {
            Node::Html { fragment: html } => {
                if matches!(
                    html.nodes.get(index(html.root, html.nodes.len())?),
                    Some(crate::html::HtmlNode::MathElement { .. })
                ) {
                    return Err(Error::Content(node as u64));
                }
                let depth = base.saturating_add(stack.len() as u64 - 1);
                b.with_depth_at_least(depth, |b| {
                    crate::html::check::validate_into(
                        html,
                        crate::html::HtmlSlot::Phrasing,
                        &fragment.html_policy,
                        &mut identity,
                        b,
                    )
                })?;
                stack.pop();
            }
            Node::Text(_) => {
                stack.pop();
            }
            Node::Element { children, .. } => {
                if let Some(child) = children.get(next) {
                    let frame = stack.last_mut().ok_or(Error::Reference(node as u64))?;
                    frame.1 += 1;
                    stack.push((index(*child, count)?, 0));
                } else {
                    stack.pop();
                }
            }
        }
    }
    identity.finish(b)?;
    Ok(Validated { fragment })
}
