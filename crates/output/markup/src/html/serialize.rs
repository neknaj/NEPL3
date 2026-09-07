use super::*;
use crate::text::{TextContext, TextError, escape};
use alloc::{format, vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
fn allocate(n: usize, b: &mut Budget) -> Result<(), HtmlError> {
    b.charge(Resource::Work, n as u64)?;
    b.charge(Resource::AllocationUnits, n as u64)?;
    Ok(())
}
fn value(a: &HtmlAttribute, b: &mut Budget) -> Result<String, HtmlError> {
    use HtmlAttribute::*;
    let borrowed = match a {
        Id { value }
        | Lang { value }
        | AriaLabel { value }
        | DataId { value }
        | DataGroup { value }
        | Alt { value } => Some(value.as_str()),
        Src { path } => Some(path.as_str()),
        Role { value } => Some(match value {
            HtmlRole::Heading => "heading",
            HtmlRole::Img => "img",
            HtmlRole::Group => "group",
            HtmlRole::Note => "note",
        }),
        Scope { value } => Some(match value {
            CellScope::Row => "row",
            CellScope::Col => "col",
        }),
        Href {
            value: HtmlHref::External { uri },
        } => Some(uri.as_str()),
        _ => None,
    };
    if let Some(s) = borrowed {
        allocate(s.len(), b)?;
        return Ok(s.into());
    }
    match a {
        AriaLevel { value } | Width { value } | Height { value } | Start { value } => {
            allocate(20, b)?;
            Ok(format!("{value}"))
        }
        Href {
            value: HtmlHref::Fragment { id },
        } => {
            allocate(id.len() + 1, b)?;
            Ok(format!("#{id}"))
        }
        Href {
            value: HtmlHref::Artifact { path, fragment },
        } => {
            let n = path
                .len()
                .checked_add(fragment.as_ref().map_or(0, |f| f.len() + 1))
                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
            allocate(n, b)?;
            Ok(if let Some(f) = fragment {
                format!("{path}#{f}")
            } else {
                path.clone()
            })
        }
        Class { values } => {
            let mut n = 0_usize;
            for v in values {
                b.charge(Resource::Work, 1)?;
                n = n
                    .checked_add(v.len() + 1)
                    .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
            }
            allocate(n, b)?;
            Ok(values.join(" "))
        }
        _ => Err(HtmlError::Policy),
    }
}
struct Output {
    pieces: Vec<String>,
    length: usize,
}
impl Output {
    fn store(&mut self, s: String, b: &mut Budget) -> Result<(), HtmlError> {
        self.length = self
            .length
            .checked_add(s.len())
            .ok_or_else(|| b.stop(StopReason::OutputLimit))?;
        b.charge(
            Resource::AllocationUnits,
            2 * core::mem::size_of::<String>() as u64,
        )?;
        self.pieces.push(s);
        Ok(())
    }
    fn literal(&mut self, s: &str, b: &mut Budget) -> Result<(), HtmlError> {
        b.charge(Resource::OutputBytes, s.len() as u64)?;
        allocate(s.len(), b)?;
        self.store(s.into(), b)
    }
    fn text(
        &mut self,
        s: &str,
        context: TextContext,
        r: u64,
        b: &mut Budget,
    ) -> Result<(), HtmlError> {
        let escaped = escape(s, context, b).map_err(|e| match e {
            TextError::Stopped(s) => HtmlError::Stopped(s),
            e => HtmlError::Text { node: r, error: e },
        })?;
        self.store(escaped, b)
    }
    fn finish(self, b: &mut Budget) -> Result<String, HtmlError> {
        if self.length > isize::MAX as usize {
            return Err(b.stop(StopReason::AllocationLimit).into());
        }
        allocate(self.length, b)?;
        let mut result = String::with_capacity(self.length);
        for piece in self.pieces {
            b.charge(Resource::Work, 1)?;
            result.push_str(&piece);
        }
        Ok(result)
    }
}
/// Deterministic complete HTML fragment. It adds no document shell, script,
/// CSS or external-resource loader. Attribute order is ASCII name order.
pub fn serialize(proof: &ValidatedHtml<'_>, b: &mut Budget) -> Result<String, HtmlError> {
    b.poll()?;
    let f = proof.fragment;
    b.charge(Resource::AllocationUnits, 64)?;
    let mut stack = vec![(f.root, 1_u64, false)];
    let mut out = Output {
        pieces: Vec::new(),
        length: 0,
    };
    while let Some((r, depth, exit)) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        b.observe_depth(depth)?;
        let n = check::node(f, r)?;
        if exit {
            if let HtmlNode::Element { tag, .. } = n {
                out.literal("</", b)?;
                out.literal(tag.name(), b)?;
                out.literal(">", b)?;
            }
            continue;
        }
        b.charge(Resource::Nodes, 1)?;
        match n {
            HtmlNode::Text { text } => out.text(text, TextContext::Content, r, b)?,
            HtmlNode::Element {
                tag,
                attributes,
                children,
            } => {
                out.literal("<", b)?;
                out.literal(tag.name(), b)?;
                b.charge(
                    Resource::AllocationUnits,
                    (attributes.len() as u64)
                        .saturating_mul(core::mem::size_of::<&HtmlAttribute>() as u64),
                )?;
                b.charge(
                    Resource::Work,
                    (attributes.len() as u64)
                        .saturating_mul(attributes.len() as u64)
                        .saturating_mul(16),
                )?;
                let mut sorted: Vec<_> = attributes.iter().collect();
                sorted.sort_unstable_by_key(|a| a.name());
                for a in sorted {
                    out.literal(" ", b)?;
                    out.literal(a.name(), b)?;
                    out.literal("=\"", b)?;
                    let text = value(a, b)?;
                    out.text(&text, TextContext::Attribute, r, b)?;
                    out.literal("\"", b)?;
                }
                out.literal(">", b)?;
                if *tag == HtmlTag::Pre {
                    out.literal("\n", b)?;
                }
                if !tag.is_void() {
                    b.charge(Resource::AllocationUnits, 64)?;
                    stack.push((r, depth, true));
                    let next = depth
                        .checked_add(1)
                        .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
                    for c in children.iter().rev() {
                        b.charge(Resource::Work, 1)?;
                        b.charge(Resource::AllocationUnits, 64)?;
                        stack.push((*c, next, false));
                    }
                }
            }
        }
    }
    out.finish(b)
}
