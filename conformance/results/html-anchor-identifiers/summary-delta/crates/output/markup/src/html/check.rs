use super::*;
use crate::text::{TextError, is_xml_character};
use alloc::{collections::BTreeSet, vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HtmlError {
    Stopped(StopReason),
    Text { node: u64, error: TextError },
    Reference(u64),
    Cycle(u64),
    Unreachable(u64),
    Content(u64),
    Attribute { node: u64, index: u64 },
    DuplicateId(u64),
    MissingFragment(u64),
    Policy,
}
impl From<StopReason> for HtmlError {
    fn from(r: StopReason) -> Self {
        Self::Stopped(r)
    }
}
pub struct ValidatedHtml<'a> {
    pub(super) fragment: &'a HtmlFragment,
}
impl ValidatedHtml<'_> {
    pub fn fragment(&self) -> &HtmlFragment {
        self.fragment
    }
}
pub(super) fn node(f: &HtmlFragment, r: u64) -> Result<&HtmlNode, HtmlError> {
    usize::try_from(r)
        .ok()
        .and_then(|i| f.nodes.get(i))
        .ok_or(HtmlError::Reference(r))
}
fn text(s: &str, r: u64, b: &mut Budget) -> Result<(), HtmlError> {
    b.charge(Resource::Work, s.len() as u64)?;
    for (byte, c) in s.char_indices() {
        if !is_xml_character(c) {
            return Err(HtmlError::Text {
                node: r,
                error: TextError::InvalidCharacter {
                    byte: byte as u64,
                    scalar: c as u64,
                },
            });
        }
    }
    Ok(())
}
fn attr(
    a: &HtmlAttribute,
    t: HtmlTag,
    p: &HtmlPolicy,
    r: u64,
    index: u64,
    b: &mut Budget,
) -> Result<(), HtmlError> {
    use HtmlAttribute::*;
    let ok = match a {
        Id { value } => {
            text(value, r, b)?;
            uri::anchor_id(value)
        }
        DataId { value } | DataGroup { value } => {
            text(value, r, b)?;
            uri::id(value)
        }
        AriaLabel { value } => {
            text(value, r, b)?;
            true
        }
        Lang { value } => nepl3_core::lexical::language::well_formed(value, b)?,
        Class { values } => {
            let mut valid = !values.is_empty();
            for (i, v) in values.iter().enumerate() {
                text(v, r, b)?;
                for prior in &values[..i] {
                    b.charge(Resource::Work, prior.len() as u64 + 1)?;
                    if prior == v {
                        valid = false
                    }
                }
                let mut found = false;
                for class in &p.classes {
                    b.charge(Resource::Work, class.len() as u64 + 1)?;
                    if class == v {
                        found = true
                    }
                }
                valid &= found;
            }
            valid
        }
        Role { .. } => true,
        AriaLevel { value } => *value > 0,
        Href { value } => {
            t == HtmlTag::A
                && match value {
                    HtmlHref::BetweenArtifacts {
                        source,
                        target,
                        fragment,
                    } => {
                        text(source, r, b)?;
                        text(target, r, b)?;
                        if let Some(f) = fragment {
                            text(f, r, b)?;
                        }
                        uri::path(source)
                            && uri::path(target)
                            && fragment.as_ref().is_none_or(|s| uri::anchor_id(s))
                    }
                    HtmlHref::Fragment { id } => {
                        text(id, r, b)?;
                        uri::anchor_id(id)
                    }
                    HtmlHref::Artifact { path, fragment } => {
                        text(path, r, b)?;
                        if let Some(f) = fragment {
                            text(f, r, b)?
                        };
                        uri::path(path) && fragment.as_ref().is_none_or(|s| uri::anchor_id(s))
                    }
                    HtmlHref::External { uri: s } => {
                        text(s, r, b)?;
                        super::external_uri(s, b)?
                    }
                }
        }
        Src { path } => {
            text(path, r, b)?;
            t == HtmlTag::Img && uri::path(path)
        }
        Alt { value } => {
            text(value, r, b)?;
            t == HtmlTag::Img
        }
        Width { value } | Height { value } => t == HtmlTag::Img && *value > 0,
        Start { value } => t == HtmlTag::Ol && *value <= i32::MAX as u64,
        Scope { .. } => t == HtmlTag::Th,
    };
    if ok {
        Ok(())
    } else {
        Err(HtmlError::Attribute { node: r, index })
    }
}
fn accepts(parent: HtmlTag, child: &HtmlNode) -> bool {
    use HtmlTag::*;
    let tag = match child {
        HtmlNode::Text { .. } => None,
        HtmlNode::Element { tag, .. } => Some(*tag),
    };
    match parent {
        Br | Img => false,
        Table => matches!(tag, Some(Caption | Thead | Tbody)),
        Thead | Tbody => tag == Some(Tr),
        Tr => matches!(tag, Some(Th | Td)),
        Ul | Ol => tag == Some(Li),
        Rp => tag.is_none(),
        P | Span | H1 | H2 | H3 | H4 | H5 | H6 | Rt | Em | Strong | Pre | Code | A => {
            tag.is_none_or(HtmlTag::is_phrasing)
        }
        Ruby => tag.is_none_or(|t| t.is_phrasing() || matches!(t, Rt | Rp)),
        Figure => tag.is_none_or(|t| t.is_flow() || t == Figcaption),
        Article | Section | Div | Figcaption | Li | Caption | Th | Td => {
            tag.is_none_or(HtmlTag::is_flow)
        }
    }
}
fn sequence(
    f: &HtmlFragment,
    r: u64,
    t: HtmlTag,
    children: &[u64],
    b: &mut Budget,
) -> Result<(), HtmlError> {
    if t == HtmlTag::Ruby {
        ruby::sequence(f, r, children, b)?;
    }
    let mut stage = 0;
    let mut captions = 0;
    let mut width = None;
    for (i, c) in children.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let n = node(f, *c)?;
        if !accepts(t, n) {
            return Err(HtmlError::Content(r));
        }
        if let HtmlNode::Element { tag, .. } = n {
            if t == HtmlTag::Table {
                let next = match tag {
                    HtmlTag::Caption => 1,
                    HtmlTag::Thead => 2,
                    _ => 3,
                };
                if next < stage || (next == stage && next != 3) {
                    return Err(HtmlError::Content(r));
                }
                stage = next;
                if matches!(tag, HtmlTag::Thead | HtmlTag::Tbody)
                    && let HtmlNode::Element { children: rows, .. } = n
                {
                    for row in rows {
                        b.charge(Resource::Work, 1)?;
                        let HtmlNode::Element {
                            tag: HtmlTag::Tr,
                            children: cells,
                            ..
                        } = node(f, *row)?
                        else {
                            return Err(HtmlError::Content(*c));
                        };
                        if width.is_some_and(|n| n != cells.len()) {
                            return Err(HtmlError::Content(r));
                        }
                        width = Some(cells.len());
                    }
                }
            }
            if t == HtmlTag::Figure && *tag == HtmlTag::Figcaption {
                captions += 1;
                if captions > 1 || (i != 0 && i + 1 != children.len()) {
                    return Err(HtmlError::Content(r));
                }
            }
        }
    }
    Ok(())
}
/// Validate each expanded occurrence, so shared DAG nodes cannot duplicate IDs
/// or bypass nested-anchor restrictions. Stops preserve the caller's Budget.
pub fn validate<'a>(
    f: &'a HtmlFragment,
    slot: HtmlSlot,
    p: &HtmlPolicy,
    b: &mut Budget,
) -> Result<ValidatedHtml<'a>, HtmlError> {
    b.poll()?;
    for c in &p.classes {
        text(c, f.root, b)?;
        if !uri::id(c) {
            return Err(HtmlError::Policy);
        }
    }
    let root = node(f, f.root)?;
    if let HtmlNode::Element { tag, .. } = root
        && !(match slot {
            HtmlSlot::Block => tag.is_flow(),
            HtmlSlot::Phrasing => tag.is_phrasing(),
        })
    {
        return Err(HtmlError::Content(f.root));
    }
    b.charge(Resource::Work, f.nodes.len() as u64)?;
    b.charge(Resource::AllocationUnits, f.nodes.len() as u64)?;
    let mut state = vec![0_u8; f.nodes.len()];
    b.charge(Resource::AllocationUnits, 64)?;
    let mut stack = vec![(f.root, 1_u64, false, 0_u8, false)];
    let mut ids = BTreeSet::new();
    let mut links = Vec::new();
    while let Some((r, depth, anchor, forbidden, exit)) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        let n = node(f, r)?;
        let i = usize::try_from(r).map_err(|_| HtmlError::Reference(r))?;
        if exit {
            state[i] = 2;
            continue;
        }
        if state[i] == 1 {
            return Err(HtmlError::Cycle(r));
        }
        b.observe_depth(depth)?;
        b.charge(Resource::Nodes, 1)?;
        state[i] = 1;
        b.charge(Resource::AllocationUnits, 64)?;
        stack.push((r, depth, anchor, forbidden, true));
        match n {
            HtmlNode::Text { text: s } => text(s, r, b)?,
            HtmlNode::Element {
                tag,
                attributes,
                children,
            } => {
                if anchor && *tag == HtmlTag::A {
                    return Err(HtmlError::Content(r));
                }
                if (forbidden & 1 != 0 && *tag == HtmlTag::Table)
                    || (forbidden & 4 != 0 && *tag == HtmlTag::Ruby)
                    || (forbidden & 2 != 0
                        && matches!(
                            tag,
                            HtmlTag::Article
                                | HtmlTag::Section
                                | HtmlTag::H1
                                | HtmlTag::H2
                                | HtmlTag::H3
                                | HtmlTag::H4
                                | HtmlTag::H5
                                | HtmlTag::H6
                        ))
                {
                    return Err(HtmlError::Content(r));
                }
                for (j, a) in attributes.iter().enumerate() {
                    b.charge(Resource::Work, 1)?;
                    for prior in &attributes[..j] {
                        b.charge(Resource::Work, 1)?;
                        if prior.name() == a.name() {
                            return Err(HtmlError::Attribute {
                                node: r,
                                index: j as u64,
                            });
                        }
                    }
                    attr(a, *tag, p, r, j as u64, b)?;
                    if let HtmlAttribute::Id { value } = a {
                        b.charge(Resource::AllocationUnits, value.len() as u64 + 128)?;
                        b.charge(
                            Resource::Work,
                            (value.len() as u64 + 1).saturating_mul(ids.len() as u64 + 1),
                        )?;
                        if !ids.insert(value.as_str()) {
                            return Err(HtmlError::DuplicateId(r));
                        }
                    }
                    if let HtmlAttribute::Href {
                        value: HtmlHref::Fragment { id },
                    } = a
                    {
                        b.charge(Resource::AllocationUnits, 64)?;
                        links.push((r, id.as_str()));
                    }
                }
                if *tag == HtmlTag::Img
                    && (!attributes
                        .iter()
                        .any(|a| matches!(a, HtmlAttribute::Src { .. }))
                        || !attributes
                            .iter()
                            .any(|a| matches!(a, HtmlAttribute::Alt { .. })))
                {
                    return Err(HtmlError::Content(r));
                }
                sequence(f, r, *tag, children, b)?;
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
                for child in children.iter().rev() {
                    b.charge(Resource::Work, 1)?;
                    b.charge(Resource::AllocationUnits, 64)?;
                    let child_ruby = matches!(
                        node(f, *child)?,
                        HtmlNode::Element {
                            tag: HtmlTag::Ruby,
                            ..
                        }
                    );
                    let child_annotation = matches!(
                        node(f, *child)?,
                        HtmlNode::Element {
                            tag: HtmlTag::Rt | HtmlTag::Rp,
                            ..
                        }
                    );
                    let next_forbidden = (forbidden & !8)
                        | if forbidden & 8 != 0 { 4 } else { 0 }
                        | if *tag == HtmlTag::Ruby && !child_annotation {
                            if child_ruby { 8 } else { 4 }
                        } else {
                            0
                        }
                        | if *tag == HtmlTag::Caption { 1 } else { 0 }
                        | if *tag == HtmlTag::Th { 2 } else { 0 };
                    stack.push((
                        *child,
                        child_depth,
                        anchor || *tag == HtmlTag::A,
                        next_forbidden,
                        false,
                    ));
                }
            }
        }
    }
    for (i, s) in state.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if *s == 0 {
            return Err(HtmlError::Unreachable(i as u64));
        }
    }
    for (r, id) in links {
        b.charge(
            Resource::Work,
            (id.len() as u64 + 1).saturating_mul(ids.len() as u64 + 1),
        )?;
        if !ids.contains(id) {
            return Err(HtmlError::MissingFragment(r));
        }
    }
    Ok(ValidatedHtml { fragment: f })
}
