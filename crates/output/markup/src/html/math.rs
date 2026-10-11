//! Native presentation composition. These bindings are not portable HtmlRequest
//! data; the original arena and its provenance remain independently available.
use super::*;
use crate::{katex::fragment, output::Output};
use nepl3_core::budget::{Budget, Resource, StopReason};

pub struct MathBinding<'a> {
    /// The actual outer MathML root, occurring exactly once in expanded output.
    pub root: u64,
    pub visual: &'a fragment::Checked<'a>,
}
pub struct MathHtml {
    pub html: String,
    /// Required generated style rules, including accessible MathML hiding.
    pub stylesheet: String,
}
const ACCESSIBLE: &str = ".nepl-math-accessible{position:absolute!important;width:1px!important;height:1px!important;padding:0!important;margin:-1px!important;overflow:hidden!important;clip:rect(0,0,0,0)!important;white-space:nowrap!important;border:0!important}";

pub(super) struct Composition<'a> {
    bindings: &'a [MathBinding<'a>],
    fragment: &'a HtmlFragment,
    seen: Vec<bool>,
    css: Output,
}
fn visual_error(error: fragment::Error) -> HtmlError {
    match error {
        fragment::Error::Stopped(reason) => HtmlError::Stopped(reason),
        _ => HtmlError::Policy,
    }
}
impl Composition<'_> {
    fn blocks(&self, root: u64, b: &mut Budget) -> Result<bool, HtmlError> {
        if let HtmlNode::MathElement { attributes, .. } = &self.fragment.nodes[root as usize] {
            b.charge(Resource::Work, attributes.len() as u64)?;
        }
        Ok(
            matches!(&self.fragment.nodes[root as usize], HtmlNode::MathElement { attributes, .. }
            if attributes.iter().any(|a| matches!(a, crate::mathml::Attribute::Display(crate::mathml::Display::Block)))),
        )
    }
    fn index(&self, root: u64, b: &mut Budget) -> Result<Option<usize>, HtmlError> {
        for (index, binding) in self.bindings.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if binding.root == root {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
    pub(super) fn open(
        &mut self,
        root: u64,
        depth: u64,
        out: &mut Output,
        b: &mut Budget,
    ) -> Result<u64, HtmlError> {
        let Some(index) = self.index(root, b)? else {
            return Ok(depth);
        };
        if self.seen[index] {
            return Err(HtmlError::Policy);
        }
        self.seen[index] = true;
        let next = depth
            .checked_add(2)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        b.observe_depth(next)?;
        b.charge(Resource::Nodes, 2)?;
        // Div wrappers preserve block Math's flow insertion contract. Inline
        // wrappers remain spans. Selection is recorded during validation below.
        let block = self.blocks(root, b)?;
        out.literal(
            if block {
                "<div class=\"nepl-math-presentation\">"
            } else {
                "<span class=\"nepl-math-presentation\">"
            },
            b,
        )?;
        let pair = fragment::serialize_at_depth(self.bindings[index].visual, depth + 1, b)
            .map_err(visual_error)?;
        out.precharged(pair.html(), b)?;
        self.css.precharged(pair.stylesheet(), b)?;
        out.literal(
            if block {
                "<div class=\"nepl-math-accessible\">"
            } else {
                "<span class=\"nepl-math-accessible\">"
            },
            b,
        )?;
        Ok(next)
    }
    pub(super) fn close(
        &mut self,
        root: u64,
        out: &mut Output,
        b: &mut Budget,
    ) -> Result<(), HtmlError> {
        if self.index(root, b)?.is_some() {
            out.literal(
                if self.blocks(root, b)? {
                    "</div></div>"
                } else {
                    "</span></span>"
                },
                b,
            )?;
        }
        Ok(())
    }
}

/// Compose only separately checked visual trees; no raw HTML is accepted.
/// The host must bind each visual to this formula/display/source and actual
/// fixed CSS/fonts, exclude conflicting styles, and enforce its shell-depth cap.
/// `start_depth` includes enclosing document ancestors. OutputBytes counts HTML
/// and required generated CSS once. Stops never become partial/fallback output.
/// O(expanded HTML nodes × bindings + visual bytes + policy comparisons).
pub fn serialize_with_math(
    proof: &ValidatedHtml<'_>,
    bindings: &[MathBinding<'_>],
    start_depth: u64,
    b: &mut Budget,
) -> Result<MathHtml, HtmlError> {
    b.poll()?;
    if start_depth == 0 {
        return Err(HtmlError::Policy);
    }
    for (i, binding) in bindings.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if !matches!(
            super::check::node(proof.fragment, binding.root)?,
            HtmlNode::MathElement {
                tag: crate::mathml::Tag::Math,
                ..
            }
        ) {
            return Err(HtmlError::Policy);
        }
        let scope = binding.visual.scope();
        if scope == "nepl-math-accessible" || scope == "nepl-math-presentation" {
            return Err(HtmlError::Policy);
        }
        for prior in &bindings[..i] {
            let other = prior.visual.scope();
            b.charge(Resource::Work, (scope.len() + other.len()) as u64 + 1)?;
            if prior.root == binding.root
                || scope == other
                || scope
                    .strip_prefix(other)
                    .is_some_and(|tail| tail.starts_with("-n"))
                || other
                    .strip_prefix(scope)
                    .is_some_and(|tail| tail.starts_with("-n"))
            {
                return Err(HtmlError::Policy);
            }
        }
    }
    // General HTML policies do not reserve this generated namespace.
    for node in &proof.fragment.nodes {
        b.charge(Resource::Work, 1)?;
        if let HtmlNode::Element { attributes, .. } = node {
            for attribute in attributes {
                b.charge(Resource::Work, 1)?;
                if let HtmlAttribute::Class { values } = attribute {
                    for value in values {
                        b.charge(Resource::Work, value.len() as u64 + 1)?;
                        if value.starts_with("nepl-math-") {
                            return Err(HtmlError::Policy);
                        }
                    }
                }
            }
        }
    }
    b.charge(Resource::AllocationUnits, bindings.len() as u64)?;
    b.charge(Resource::Work, bindings.len() as u64)?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(bindings.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    seen.resize(bindings.len(), false);
    let mut composition = Composition {
        bindings,
        seen,
        css: Output::default(),
        fragment: proof.fragment,
    };
    if !bindings.is_empty() {
        composition.css.literal(ACCESSIBLE, b)?;
    }
    let mut out = Output::default();
    super::serialize::fragment_into(
        proof.fragment,
        false,
        start_depth,
        Some(&mut composition),
        &mut out,
        b,
    )?;
    b.charge(Resource::Work, composition.seen.len() as u64)?;
    if composition.seen.iter().any(|seen| !seen) {
        return Err(HtmlError::Policy);
    }
    Ok(MathHtml {
        html: out.finish(),
        stylesheet: composition.css.finish(),
    })
}
