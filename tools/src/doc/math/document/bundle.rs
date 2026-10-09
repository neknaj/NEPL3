//! Metered local emission, not portable artifact admission or browser qualification.
use super::LocalDocument;
use crate::doc::math::assets::{self, Asset};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_markup::html::{self, HtmlAttribute, HtmlError, HtmlHref, HtmlNode};

pub const HTML_PATH: &str = "index.html";
pub const DOC_CSS_PATH: &str = "assets/doc.css";
pub const MATH_CSS_PATH: &str = "assets/math.css";
pub const KATEX_DIRECTORY: &str = "assets/katex";
const HEAD: &str = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'self'; style-src-attr 'none'; font-src 'self'; base-uri 'none'; form-action 'none'\"><title>NEPL3 Doc</title><link rel=\"stylesheet\" href=\"assets/doc.css\">";
const MATH_LINKS: &str = "<link rel=\"stylesheet\" href=\"assets/katex/katex.min.css\"><link rel=\"stylesheet\" href=\"assets/math.css\">";
const BODY: &str = "</head><body>";
const TAIL: &str = "</body></html>";

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Html(HtmlError),
    Assets(assets::Error),
    UnresolvedImage { element: u64 },
    UnresolvedLink { element: u64 },
    OutputDepth { element: u64, shell_depth: u64 },
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<HtmlError> for Error {
    fn from(e: HtmlError) -> Self {
        match e {
            HtmlError::Stopped(s) => Self::Stopped(s),
            e => Self::Html(e),
        }
    }
}
impl From<assets::Error> for Error {
    fn from(e: assets::Error) -> Self {
        match e {
            assets::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Assets(e),
        }
    }
}
/// Owned bytes retain an immutable association with the exact imported document.
/// Paths above are relative to a single artifact root. KaTeX files go below
/// KATEX_DIRECTORY without changing their package-relative paths.
/// No file I/O or publication occurs. A nonempty Math stylesheet implies the
/// fixed resource inventory is present. Empty MathML-only documents need neither.
/// Retained process observations, including failed cleanup, remain on source;
/// this object does not certify execution, fidelity, isolation or accessibility.
/// ```compile_fail
/// fn mutate(b: &mut nepl3_tools::doc::math::document::bundle::LocalBundle<'_, '_, '_, '_, '_>) { b.html.clear(); }
/// ```
/// ```compile_fail
/// fn duplicate(b: nepl3_tools::doc::math::document::bundle::LocalBundle<'_, '_, '_, '_, '_>) { b.clone(); }
/// ```
/// ```compile_fail
/// fn detach(b: nepl3_tools::doc::math::document::bundle::LocalBundle<'_, '_, '_, '_, '_>) { b.into_parts(); }
/// ```
#[must_use]
pub struct LocalBundle<'a, 'p, 'd, 'c, 'r> {
    source: &'a LocalDocument<'p, 'd, 'c, 'r>,
    html: String,
    doc_css: String,
    math_css: String,
    resources: Vec<Asset>,
}
impl<'a, 'p, 'd, 'c, 'r> LocalBundle<'a, 'p, 'd, 'c, 'r> {
    pub fn source(&self) -> &'a LocalDocument<'p, 'd, 'c, 'r> {
        self.source
    }
    pub fn html(&self) -> &str {
        &self.html
    }
    pub fn doc_stylesheet(&self) -> &str {
        &self.doc_css
    }
    pub fn math_stylesheet(&self) -> &str {
        &self.math_css
    }
    pub fn katex_assets(&self) -> &[Asset] {
        &self.resources
    }
}
fn reserve(len: usize, b: &mut Budget) -> Result<String, Error> {
    if len > isize::MAX as usize {
        return Err(b.stop(StopReason::AllocationLimit).into());
    }
    b.charge(Resource::AllocationUnits, len as u64)?;
    let mut s = String::new();
    s.try_reserve_exact(len)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(s)
}
fn append(target: &mut String, source: &str, output: bool, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, source.len() as u64)?;
    if output {
        b.charge(Resource::OutputBytes, source.len() as u64)?;
    }
    target.push_str(source);
    Ok(())
}
fn sum(a: usize, c: usize, b: &mut Budget) -> Result<usize, Error> {
    a.checked_add(c)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit).into())
}
fn push_pending(
    pending: &mut Vec<(u64, u64)>,
    item: (u64, u64),
    b: &mut Budget,
) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    if pending.len() == pending.capacity() {
        let capacity = pending
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(1))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        let bytes = capacity
            .checked_mul(core::mem::size_of::<(u64, u64)>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        b.charge(
            Resource::Work,
            (pending.len() * core::mem::size_of::<(u64, u64)>()) as u64,
        )?;
        pending
            .try_reserve_exact(capacity - pending.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    pending.push(item);
    Ok(())
}
// Input has already passed the closed document import. The browser's relative
// envelope is independent of the caller's current logical Budget depth.
fn check_viewer_depth(fragment: &html::HtmlFragment, b: &mut Budget) -> Result<(), Error> {
    let mut pending = Vec::new();
    push_pending(&mut pending, (fragment.root, 3), b)?;
    while let Some((element, shell_depth)) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        if shell_depth > 256 {
            return Err(Error::OutputDepth {
                element,
                shell_depth,
            });
        }
        b.observe_depth(shell_depth)?;
        match &fragment.nodes[element as usize] {
            HtmlNode::Element { children, .. }
            | HtmlNode::MathElement { children, .. }
            | HtmlNode::SvgElement { children, .. } => {
                for child in children.iter().rev() {
                    b.charge(Resource::Work, 1)?;
                    push_pending(&mut pending, (*child, shell_depth + 1), b)?;
                }
            }
            HtmlNode::Text { .. } => {}
        }
    }
    Ok(())
}
impl<'p, 'd, 'c, 'r> LocalDocument<'p, 'd, 'c, 'r> {
    /// Emit a standalone external-CSS bundle with no viewer scripts/network
    /// fonts. This selected plain-Article stage rejects image attributes instead
    /// of silently emitting resources it does not own. Cross-artifact links and
    /// missing local targets are rejected; ordinary external navigation links
    /// remain links, without permission to fetch network subresources.
    /// Klee One remains an
    /// optional locally installed family; the bundled Doc CSS has system fallbacks.
    /// Caller depth and all output/copy costs apply again on each invocation.
    pub fn materialize_bundle<'a>(
        &'a self,
        b: &mut Budget,
    ) -> Result<LocalBundle<'a, 'p, 'd, 'c, 'r>, Error> {
        b.poll()?;
        let mut selected = None;
        let mut css_len = 0;
        for math in self.math() {
            b.charge(Resource::Work, 1)?;
            css_len = sum(css_len, math.stylesheet().len(), b)?;
            if let Some(assets) = math.assets() {
                selected = Some(assets);
            }
        }
        let request = &self.rendered().fragment.markup;
        for (i, node) in request.fragment.nodes.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if let HtmlNode::Element { attributes, .. } = node {
                for attr in attributes {
                    b.charge(Resource::Work, 1)?;
                    if let HtmlAttribute::Href {
                        value: HtmlHref::Artifact { .. } | HtmlHref::BetweenArtifacts { .. },
                    } = attr
                    {
                        return Err(Error::UnresolvedLink { element: i as u64 });
                    }
                    if matches!(
                        attr,
                        HtmlAttribute::Src { .. } | HtmlAttribute::EmbeddedSvg { .. }
                    ) {
                        return Err(Error::UnresolvedImage { element: i as u64 });
                    }
                }
            }
        }
        check_viewer_depth(&request.fragment, b)?;
        // Fixed shell: html/head/body, three metas, title and its text, one or
        // three links. Doctype is lexical output, not a markup arena node.
        b.charge(Resource::Nodes, if selected.is_some() { 11 } else { 9 })?;
        let depth = b.current_depth();
        b.observe_depth(4)?;
        let wrapper_depth = depth
            .checked_add(2)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        let fragment = b.with_depth_at_least(wrapper_depth, |b| -> Result<String, Error> {
            let checked = html::validate(&request.fragment, request.slot, &request.policy, b)?;
            Ok(html::serialize(&checked, b)?)
        })?;
        let links = if selected.is_some() { MATH_LINKS } else { "" };
        let mut size = fragment.len();
        for piece in [HEAD, links, BODY, TAIL] {
            size = sum(size, piece.len(), b)?;
        }
        let mut emitted = reserve(size, b)?;
        for piece in [HEAD, links, BODY] {
            append(&mut emitted, piece, true, b)?;
        }
        append(&mut emitted, &fragment, false, b)?;
        append(&mut emitted, TAIL, true, b)?;
        let mut doc_css = reserve(nepl3_doc_html::STYLESHEET.len(), b)?;
        append(&mut doc_css, nepl3_doc_html::STYLESHEET, true, b)?;
        let mut math_css = reserve(css_len, b)?;
        for math in self.math() {
            b.charge(Resource::Work, 1)?;
            append(&mut math_css, math.stylesheet(), true, b)?;
        }
        let resources = match selected {
            Some(assets) => assets.materialize(b)?,
            None => Vec::new(),
        };
        b.poll()?;
        Ok(LocalBundle {
            source: self,
            html: emitted,
            doc_css,
            math_css,
            resources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chain(depth: usize) -> html::HtmlFragment {
        html::HtmlFragment {
            root: 0,
            nodes: (0..depth)
                .map(|i| {
                    if i + 1 == depth {
                        HtmlNode::Text { text: "x".into() }
                    } else {
                        HtmlNode::Element {
                            tag: html::HtmlTag::Div,
                            attributes: vec![],
                            children: vec![i as u64 + 1],
                        }
                    }
                })
                .collect(),
        }
    }
    #[test]
    fn shell_envelope_is_relative_and_distinct_from_caller_budget() -> Result<(), Error> {
        for depth in [254, 255, 256] {
            let fragment = chain(depth);
            let mut b = crate::doc::source::budget();
            html::validate(
                &fragment,
                html::HtmlSlot::Block,
                &html::HtmlPolicy { classes: vec![] },
                &mut b,
            )?;
            for caller in [0, 50] {
                let result = b.with_depth_at_least(caller, |b| check_viewer_depth(&fragment, b));
                if depth == 254 {
                    result?;
                } else {
                    assert!(matches!(
                        result,
                        Err(Error::OutputDepth {
                            shell_depth: 257,
                            ..
                        })
                    ));
                }
            }
        }
        Ok(())
    }
    #[test]
    fn repeated_child_frontier_growth_is_metered() -> Result<(), Error> {
        let fragment = html::HtmlFragment {
            root: 0,
            nodes: vec![
                HtmlNode::Element {
                    tag: html::HtmlTag::Div,
                    attributes: vec![],
                    children: vec![1; 100],
                },
                HtmlNode::Text { text: "x".into() },
            ],
        };
        html::validate(
            &fragment,
            html::HtmlSlot::Block,
            &html::HtmlPolicy { classes: vec![] },
            &mut crate::doc::source::budget(),
        )?;
        let mut b = crate::doc::source::budget();
        check_viewer_depth(&fragment, &mut b)?;
        let usage = b.usage();
        assert!(usage.allocation_units >= 100 * core::mem::size_of::<(u64, u64)>() as u64);
        for short in [false, true] {
            let mut limits = b.limits();
            limits.allocation_units = usage.allocation_units - u64::from(short);
            let result = check_viewer_depth(&fragment, &mut Budget::new(limits));
            if short {
                assert!(matches!(
                    result,
                    Err(Error::Stopped(StopReason::AllocationLimit))
                ));
            } else {
                result?;
            }
        }
        Ok(())
    }
}
