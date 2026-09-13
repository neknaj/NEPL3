//! Move a checked MathML arena into the flat mixed HTML representation.
use super::*;
use crate::html::{self, HtmlFragment, HtmlNode, HtmlRequest, HtmlSlot};

/// Owned structural conversion with mappings to its exact immutable result.
/// This is a local conversion proof, not a transferable provider assertion.
pub struct HtmlProjection {
    request: HtmlRequest,
    math_nodes: Vec<u64>,
    html_ranges: Vec<Option<(u64, u64)>>,
}
impl HtmlProjection {
    pub fn request(&self) -> &HtmlRequest {
        &self.request
    }
    /// Flat output node corresponding to the given original MathML arena node.
    pub fn math_node(&self, node: u64) -> Option<u64> {
        self.math_nodes.get(usize::try_from(node).ok()?).copied()
    }
    /// Original HTML leaf node, scoped by its owning MathML node. A Doc origin
    /// map must retain this owner; local HTML indices are not globally unique.
    pub fn html_node(&self, owner: u64, node: u64) -> Option<u64> {
        let (start, count) = self
            .html_ranges
            .get(usize::try_from(owner).ok()?)?
            .as_ref()?;
        if node < *count {
            start.checked_add(node)
        } else {
            None
        }
    }
    /// Consume the mapping proof after any source/origin references are remapped.
    pub fn into_request(self) -> HtmlRequest {
        self.request
    }
}
fn reserve<T>(items: usize, b: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = items
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(items)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(result)
}
/// Validate and move a MathML fragment into HTML's flat mixed arena. No markup
/// is parsed from strings. Owned text/attributes/edge buffers move without copies.
/// Conversion is O(nodes + edges), with O(nodes) new indexing/storage in addition
/// to the owning validators' work (which can expand shared occurrences).
/// Failure consumes the input, returns no partial projection and preserves stops.
pub fn into_html(fragment: Fragment, b: &mut Budget) -> Result<HtmlProjection, Error> {
    validate(&fragment, b)?;
    let mut math_nodes = reserve(fragment.nodes.len(), b)?;
    let mut html_ranges = reserve(fragment.nodes.len(), b)?;
    let mut count = 0usize;
    for node in &fragment.nodes {
        b.charge(Resource::Work, 1)?;
        let (size, root, range) = match node {
            Node::Html { fragment } => (
                fragment.nodes.len(),
                fragment.root,
                Some((count as u64, fragment.nodes.len() as u64)),
            ),
            _ => (1, 0, None),
        };
        math_nodes.push(
            (count as u64)
                .checked_add(root)
                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?,
        );
        html_ranges.push(range);
        count = count
            .checked_add(size)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    }
    let root = *math_nodes
        .get(index(fragment.root, math_nodes.len())?)
        .ok_or(Error::Reference(fragment.root))?;
    let slot = match &fragment.nodes[index(fragment.root, fragment.nodes.len())?] {
        Node::Element { attributes, .. } => {
            b.charge(Resource::Work, attributes.len() as u64)?;
            if attributes
                .iter()
                .any(|a| matches!(a, Attribute::Display(Display::Block)))
            {
                HtmlSlot::Block
            } else {
                HtmlSlot::Phrasing
            }
        }
        _ => return Err(Error::Content(fragment.root)),
    };
    let mut nodes = reserve(count, b)?;
    for node in fragment.nodes {
        b.charge(Resource::Work, 1)?;
        match node {
            Node::Text(text) => nodes.push(HtmlNode::Text { text }),
            Node::Element {
                tag,
                attributes,
                mut children,
            } => {
                for child in &mut children {
                    b.charge(Resource::Work, 1)?;
                    *child = math_nodes[index(*child, math_nodes.len())?];
                }
                nodes.push(HtmlNode::MathElement {
                    tag,
                    attributes,
                    children,
                });
            }
            Node::Html { fragment } => {
                let base = nodes.len() as u64;
                for mut node in fragment.nodes {
                    b.charge(Resource::Work, 1)?;
                    if let HtmlNode::Element { children, .. }
                    | HtmlNode::MathElement { children, .. } = &mut node
                    {
                        for child in children {
                            b.charge(Resource::Work, 1)?;
                            *child = base
                                .checked_add(*child)
                                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
                        }
                    }
                    nodes.push(node);
                }
            }
        }
    }
    let request = HtmlRequest {
        fragment: HtmlFragment { nodes, root },
        slot,
        policy: fragment.html_policy,
    };
    html::validate(&request.fragment, request.slot, &request.policy, b)?;
    Ok(HtmlProjection {
        request,
        math_nodes,
        html_ranges,
    })
}
