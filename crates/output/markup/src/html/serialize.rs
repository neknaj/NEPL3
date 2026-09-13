use super::*;
use crate::output::Output;
use crate::text::{TextContext, TextError, escape};
use alloc::{format, vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
fn allocate(n: usize, b: &mut Budget) -> Result<(), HtmlError> {
    if n > isize::MAX as usize {
        return Err(b.stop(StopReason::AllocationLimit).into());
    }
    b.charge(Resource::Work, n as u64)?;
    b.charge(Resource::AllocationUnits, n as u64)?;
    Ok(())
}
// Preserve the decoded identifier exactly. Encoding every non-unreserved byte
// also prevents literal percent escapes or fragment directives being reinterpreted.
fn fragment_id(s: &str, b: &mut Budget) -> Result<String, HtmlError> {
    fn unreserved(c: u8) -> bool {
        c.is_ascii_alphanumeric() || b"-._~".contains(&c)
    }
    b.charge(Resource::Work, s.len() as u64)?;
    let mut len = 0usize;
    for c in s.bytes() {
        len = len
            .checked_add(if unreserved(c) { 1 } else { 3 })
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    }
    allocate(len, b)?;
    let mut result = String::with_capacity(len);
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for c in s.bytes() {
        if unreserved(c) {
            result.push(c as char);
        } else {
            result.push('%');
            result.push(HEX[(c >> 4) as usize] as char);
            result.push(HEX[(c & 15) as usize] as char);
        }
    }
    Ok(result)
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
        Href {
            value:
                HtmlHref::BetweenArtifacts {
                    source,
                    target,
                    fragment,
                },
        } => between_artifacts(source, target, fragment.as_deref(), b),
        AriaLevel { value } | Width { value } | Height { value } | Start { value } => {
            allocate(20, b)?;
            Ok(format!("{value}"))
        }
        Href {
            value: HtmlHref::Fragment { id },
        } => {
            let id = fragment_id(id, b)?;
            allocate(id.len() + 1, b)?;
            Ok(format!("#{id}"))
        }
        Href {
            value: HtmlHref::Artifact { path, fragment },
        } => {
            let fragment = fragment.as_deref().map(|s| fragment_id(s, b)).transpose()?;
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

/// Input paths have already passed the closed artifact-path grammar. Compare
/// complete directory segments, never filename prefixes. No URL decoding or
/// ambient base URI is involved; the host supplies both artifact-root paths.
fn between_artifacts(
    source: &str,
    target: &str,
    fragment: Option<&str>,
    b: &mut Budget,
) -> Result<String, HtmlError> {
    let encoded = fragment.map(|s| fragment_id(s, b)).transpose()?;
    let fragment = encoded.as_deref();
    let work = (source.len() as u64)
        .checked_add(target.len() as u64)
        .and_then(|n| n.checked_mul(3))
        .ok_or_else(|| b.stop(StopReason::WorkLimit))?;
    b.charge(Resource::Work, work)?;
    let parent = &source[..source.rfind('/').map_or(0, |i| i + 1)];
    let mut common = 0;
    for (left, right) in parent.split_inclusive('/').zip(target.split_inclusive('/')) {
        if left != right {
            break;
        }
        common += left.len();
    }
    let up = parent[common..].bytes().filter(|c| *c == b'/').count();
    let tail = &target[common..];
    let n = up
        .checked_mul(3)
        .and_then(|n| n.checked_add(tail.len()))
        .and_then(|n| n.checked_add(fragment.map_or(0, |s| s.len() + 1)))
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    allocate(n, b)?;
    let mut out = String::with_capacity(n);
    for _ in 0..up {
        out.push_str("../");
    }
    out.push_str(tail);
    if let Some(fragment) = fragment {
        out.push('#');
        out.push_str(fragment);
    }
    Ok(out)
}
impl Output {
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
        self.precharged(&escaped, b)?;
        Ok(())
    }
}
/// Deterministic complete HTML fragment. It adds no document shell, script,
/// CSS or external-resource loader. HTML attributes use ASCII name order;
/// MathML attributes retain the typed input order, as in the MathML serializer.
/// Output uses one growable buffer, with amortized O(emitted bytes) copy work;
/// escaped attribute/text temporaries and the explicit traversal frontier remain.
/// OutputBytes counts emitted bytes once, including escaping. AllocationUnits
/// measures logical allocation charges, not a claim about allocator peak RSS.
pub fn serialize(proof: &ValidatedHtml<'_>, b: &mut Budget) -> Result<String, HtmlError> {
    serialize_mode(proof, false, b)
}
/// Serialize the checked fragment for XML embedding. An element root declares
/// the XHTML namespace, void elements are self-closing, and `pre` retains exactly
/// its input whitespace (no HTML-parser first-newline compensation).
/// A text root emits escaped text without adding a wrapper or namespace.
/// This does not prove IDs/resources against an enclosing mixed-namespace tree.
pub fn serialize_xhtml(proof: &ValidatedHtml<'_>, b: &mut Budget) -> Result<String, HtmlError> {
    serialize_mode(proof, true, b)
}
fn serialize_mode(
    proof: &ValidatedHtml<'_>,
    xml: bool,
    b: &mut Budget,
) -> Result<String, HtmlError> {
    let mut out = Output::default();
    fragment_into(proof.fragment, xml, &mut out, b)?;
    Ok(out.finish())
}
/// Caller must own the composite structural and identity proof.
pub(crate) fn embedded(
    f: &HtmlFragment,
    out: &mut Output,
    b: &mut Budget,
) -> Result<(), HtmlError> {
    fragment_into(f, true, out, b)
}
fn fragment_into(
    f: &HtmlFragment,
    xml: bool,
    out: &mut Output,
    b: &mut Budget,
) -> Result<(), HtmlError> {
    b.poll()?;
    b.charge(Resource::AllocationUnits, 64)?;
    let mut stack = vec![(f.root, 1_u64, false, false)];
    while let Some((r, depth, exit, math_parent)) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        b.observe_depth(depth)?;
        let n = check::node(f, r)?;
        if exit {
            if let HtmlNode::Element { tag, .. } = n {
                out.literal("</", b)?;
                out.literal(tag.name(), b)?;
                out.literal(">", b)?;
            }
            if let HtmlNode::MathElement { tag, .. } = n {
                out.literal("</", b)?;
                out.literal(tag.name(), b)?;
                out.literal(">", b)?;
            }
            continue;
        }
        b.charge(Resource::Nodes, 1)?;
        match n {
            HtmlNode::Text { text } => out.text(text, TextContext::Content, r, b)?,
            HtmlNode::MathElement {
                tag,
                attributes,
                children,
            } => {
                out.literal("<", b)?;
                out.literal(tag.name(), b)?;
                if *tag == crate::mathml::Tag::Math {
                    out.literal(" xmlns=\"http://www.w3.org/1998/Math/MathML\"", b)?;
                }
                for a in attributes {
                    let (name, value) = crate::mathml::serialize::attr(a);
                    out.literal(" ", b)?;
                    out.literal(name, b)?;
                    out.literal("=\"", b)?;
                    out.literal(value, b)?;
                    out.literal("\"", b)?;
                }
                out.literal(">", b)?;
                b.charge(Resource::AllocationUnits, 64)?;
                stack.push((r, depth, true, math_parent));
                for c in children.iter().rev() {
                    b.charge(Resource::Work, 1)?;
                    b.charge(Resource::AllocationUnits, 64)?;
                    stack.push((*c, depth.saturating_add(1), false, true));
                }
            }
            HtmlNode::Element {
                tag,
                attributes,
                children,
            } => {
                out.literal("<", b)?;
                out.literal(tag.name(), b)?;
                if (xml && depth == 1) || math_parent {
                    out.literal(" xmlns=\"http://www.w3.org/1999/xhtml\"", b)?;
                }
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
                out.literal(if xml && tag.is_void() { " />" } else { ">" }, b)?;
                if !xml && *tag == HtmlTag::Pre {
                    out.literal("\n", b)?;
                }
                if !tag.is_void() {
                    b.charge(Resource::AllocationUnits, 64)?;
                    stack.push((r, depth, true, math_parent));
                    let next = depth
                        .checked_add(1)
                        .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
                    for c in children.iter().rev() {
                        b.charge(Resource::Work, 1)?;
                        b.charge(Resource::AllocationUnits, 64)?;
                        stack.push((*c, next, false, false));
                    }
                }
            }
        }
    }
    Ok(())
}
