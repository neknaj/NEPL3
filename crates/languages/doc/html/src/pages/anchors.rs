use crate::build::push;
use alloc::vec::Vec;
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_markup::html::{HtmlAttribute, HtmlNode};

/// IDs borrowed from the final selected output. This is an operation-local
/// lookup table, not a validation proof or a cache across document revisions.
pub(super) struct OutputAnchors<'a>(Vec<&'a str>);

#[cfg(test)]
mod tests;

impl<'a> OutputAnchors<'a> {
    pub(super) fn collect(nodes: &'a [HtmlNode], b: &mut Budget) -> Result<Self, StopReason> {
        b.poll()?;
        let mut ids = Vec::new();
        for node in nodes {
            b.charge(Resource::Work, 1)?;
            if let HtmlNode::Element { attributes, .. } = node {
                for attribute in attributes {
                    b.charge(Resource::Work, 1)?;
                    if let HtmlAttribute::Id { value } = attribute {
                        push(&mut ids, value.as_str(), b)?;
                    }
                }
            }
        }
        Ok(Self(ids))
    }

    pub(super) fn contains(&self, id: &str, b: &mut Budget) -> Result<bool, StopReason> {
        b.poll()?;
        for value in &self.0 {
            b.charge(Resource::Work, (id.len() + value.len()) as u64 + 1)?;
            if id == *value {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
