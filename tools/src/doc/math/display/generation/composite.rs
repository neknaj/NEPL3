//! Local typed composition, not complete document/resource/accessibility admission.
use super::*;
use crate::doc::math::display::TexPreparation;
use crate::doc::math::{ProjectionError as MathProjectionError, RenderedHtmlMath};
use nepl3_markup::{html::*, mathml::Display};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Math(MathProjectionError),
    Html(HtmlError),
    Shape,
    ReservedClass,
    Preference,
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<MathProjectionError> for Error {
    fn from(e: MathProjectionError) -> Self {
        match e {
            MathProjectionError::Stopped(s) => Self::Stopped(s),
            e => Self::Math(e),
        }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Representation {
    MathML,
    Dual,
}
/// Explicit host selection, never inferred from a renderer failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionPolicy {
    /// Preserve all renderer failures as Deferred; not a cleanup certificate.
    Strict,
    OrdinaryMathmlFallback,
}
/// Retained ordinary renderer failure. This is not successful KaTeX execution.
pub enum FallbackReason {
    Unavailable(Option<reply::Cause>),
    RenderError,
}
/// Visual nodes and generated wrappers have a Doc occurrence owner, not forged
/// per-Math-node provenance. All original MathML indices stay below first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedRange {
    pub first: u64,
    pub elements: u64,
    pub visual_root: u64,
    pub accessible_root: u64,
}
#[must_use]
pub enum Composition<'c, 'r, E> {
    Ready(ComposedMath<'c, 'r>),
    Deferred(OwnedGeneration<'c, 'r, E>),
}
pub(in crate::doc::math) struct RendererSelection<'c> {
    pub(in crate::doc::math) config: Config<'c>,
    pub(in crate::doc::math) controls: Controls,
}
/// Owns the retained prepared Math result, either from a native generation
/// scope or explicit MathML-only completion. Doc occurrence association is
/// supplied separately by the opaque occurrence wrapper.
/// Mathematical fidelity, global CSS/resource scope and actual accessibility
/// require separate verification. Fields are immutable and cannot be paired
/// with an arbitrary replacement Math owner. A ready composition is not a
/// process-cleanup or artifact-success gate; inspect the retained termination flag.
/// ```compile_fail
/// fn replace(v: &mut nepl3_tools::doc::math::display::generation::composite::ComposedMath<'_, '_>) { let _ = &mut v.math; }
/// ```
/// ```compile_fail
/// fn mutate(v: &nepl3_tools::doc::math::display::generation::composite::ComposedMath<'_, '_>) { v.math().markup.fragment.nodes.clear(); }
/// ```
/// ```compile_fail
/// fn extract(v: nepl3_tools::doc::math::display::generation::composite::ComposedMath<'_, '_>) { v.into_parts(); }
/// ```
/// ```compile_fail
/// fn duplicate(v: nepl3_tools::doc::math::display::generation::composite::ComposedMath<'_, '_>) { let _ = v.clone(); }
/// ```
pub struct ComposedMath<'c, 'r> {
    math: RenderedHtmlMath,
    doc_node: u64,
    display: Display,
    tex: TexPreparation,
    representation: Representation,
    fallback: Option<FallbackReason>,
    generated: Option<GeneratedRange>,
    stylesheet: String,
    assets: Option<&'r PreparedAssets>,
    selection: Option<RendererSelection<'c>>,
    scope: String,
    observations: Option<Observations>,
    termination_failure: Option<bool>,
}
impl<'c> ComposedMath<'c, '_> {
    pub fn math(&self) -> &RenderedHtmlMath {
        &self.math
    }
    pub fn doc_node(&self) -> u64 {
        self.doc_node
    }
    pub fn display(&self) -> Display {
        self.display
    }
    pub fn tex(&self) -> &TexPreparation {
        &self.tex
    }
    pub fn representation(&self) -> Representation {
        self.representation
    }
    pub fn fallback(&self) -> Option<&FallbackReason> {
        self.fallback.as_ref()
    }
    pub fn generated(&self) -> Option<GeneratedRange> {
        self.generated
    }
    pub fn stylesheet(&self) -> &str {
        &self.stylesheet
    }
    pub fn assets(&self) -> Option<&PreparedAssets> {
        self.assets
    }
    /// None means no renderer was configured. Some does not prove execution.
    pub fn selected_config(&self) -> Option<Config<'c>> {
        self.selection.as_ref().map(|selected| selected.config)
    }
    pub fn selected_scope(&self) -> &str {
        &self.scope
    }
    pub fn controls(&self) -> Option<Controls> {
        self.selection.as_ref().map(|selected| selected.controls)
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.observations.as_ref()
    }
    pub fn termination_failure(&self) -> Option<bool> {
        self.termination_failure
    }
}
fn reserve<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(v)
}
fn copy(value: &str, b: &mut Budget) -> Result<String, Error> {
    b.charge(Resource::Work, value.len() as u64)?;
    b.charge(Resource::AllocationUnits, value.len() as u64)?;
    let mut s = String::new();
    s.try_reserve_exact(value.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    s.push_str(value);
    Ok(s)
}
fn css(css: &mut String, value: &str, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::OutputBytes, value.len() as u64)?;
    b.charge(Resource::Work, value.len() as u64)?;
    let total = css
        .len()
        .checked_add(value.len())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    if total > css.capacity() {
        b.charge(Resource::AllocationUnits, (total - css.capacity()) as u64)?;
        b.charge(Resource::Work, css.len() as u64)?;
        css.try_reserve_exact(total - css.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    css.push_str(value);
    Ok(())
}
fn wrapper(tag: HtmlTag, class: &str, children: &[u64], b: &mut Budget) -> Result<HtmlNode, Error> {
    b.charge(Resource::Nodes, 1)?;
    b.charge(Resource::Work, children.len() as u64 + 1)?;
    let mut values = reserve(1, b)?;
    values.push(copy(class, b)?);
    let mut attributes = reserve(1, b)?;
    attributes.push(HtmlAttribute::Class { values });
    let mut edges = reserve(children.len(), b)?;
    edges.extend_from_slice(children);
    Ok(HtmlNode::Element {
        tag,
        attributes,
        children: edges,
    })
}
fn check_visual(request: &HtmlRequest, scope: &str, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    let root = usize::try_from(request.fragment.root)
        .ok()
        .and_then(|i| request.fragment.nodes.get(i))
        .ok_or(Error::Shape)?;
    let HtmlNode::Element {
        tag: HtmlTag::Span,
        attributes,
        ..
    } = root
    else {
        return Err(Error::Shape);
    };
    if request.slot != HtmlSlot::Phrasing || attributes.len() != 2 {
        return Err(Error::Shape);
    }
    let mut class = false;
    let mut hidden = false;
    for attr in attributes {
        b.charge(Resource::Work, 1)?;
        match attr {
            HtmlAttribute::Class { values } if values.len() == 1 => {
                b.charge(Resource::Work, scope.len() as u64 + values[0].len() as u64)?;
                class = values[0] == scope;
            }
            HtmlAttribute::AriaHidden { value: true } => hidden = true,
            _ => return Err(Error::Shape),
        }
    }
    if class && hidden {
        Ok(())
    } else {
        Err(Error::Shape)
    }
}
fn check_math_classes(policy: &HtmlPolicy, b: &mut Budget) -> Result<(), Error> {
    for class in &policy.classes {
        b.charge(Resource::Work, class.len() as u64 + 1)?;
        if class.starts_with("nepl-math-") {
            return Err(Error::ReservedClass);
        }
    }
    Ok(())
}
fn check_accessible_class(
    policy: &HtmlPolicy,
    accessible: &str,
    b: &mut Budget,
) -> Result<(), Error> {
    for class in &policy.classes {
        b.charge(
            Resource::Work,
            (class.len().min(accessible.len()) + 1) as u64,
        )?;
        if class == accessible {
            return Err(Error::ReservedClass);
        }
    }
    Ok(())
}
fn join(
    math: &mut RenderedHtmlMath,
    visual: HtmlRequest,
    scope: &str,
    style: &mut String,
    display: Display,
    b: &mut Budget,
) -> Result<GeneratedRange, Error> {
    check_visual(&visual, scope, b)?;
    check_math_classes(&math.markup.policy, b)?;
    let mut accessible = copy(scope, b)?;
    let suffix = "-accessible";
    b.charge(Resource::AllocationUnits, suffix.len() as u64)?;
    b.charge(Resource::Work, (accessible.len() + suffix.len()) as u64)?;
    accessible
        .try_reserve_exact(suffix.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    accessible.push_str(suffix);
    check_accessible_class(&visual.policy, &accessible, b)?;
    let m = math.markup.fragment.nodes.len();
    let v = visual.fragment.nodes.len();
    let total = m
        .checked_add(v)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let mut nodes = reserve(total, b)?;
    for node in std::mem::take(&mut math.markup.fragment.nodes) {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        nodes.push(node);
    }
    let offset = m as u64;
    let add = |index: u64| offset.checked_add(index).ok_or(Error::Shape);
    let visual_root = add(visual.fragment.root)?;
    for mut node in visual.fragment.nodes {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        match &mut node {
            HtmlNode::Element { children, .. }
            | HtmlNode::MathElement { children, .. }
            | HtmlNode::SvgElement { children, .. } => {
                for child in children {
                    b.charge(Resource::Work, 1)?;
                    *child = add(*child)?;
                }
            }
            HtmlNode::Text { .. } => {}
        }
        nodes.push(node);
    }
    let tag = match display {
        Display::Inline => HtmlTag::Span,
        Display::Block => HtmlTag::Div,
    };
    let accessible_root = nodes.len() as u64;
    nodes.push(wrapper(tag, &accessible, &[math.markup.fragment.root], b)?);
    let root = nodes.len() as u64;
    nodes.push(wrapper(tag, scope, &[accessible_root, visual_root], b)?);
    let count = math
        .markup
        .policy
        .classes
        .len()
        .checked_add(visual.policy.classes.len())
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let mut classes: Vec<String> = reserve(count, b)?;
    for class in std::mem::take(&mut math.markup.policy.classes)
        .into_iter()
        .chain(visual.policy.classes)
    {
        b.charge(Resource::Work, 1)?;
        let mut found = false;
        for prior in &classes {
            b.charge(Resource::Work, (prior.len().min(class.len()) + 1) as u64)?;
            if prior == &class {
                found = true;
                break;
            }
        }
        if !found {
            classes.push(class);
        }
    }
    // Fixed clipping declarations mirror the reviewed pinned KaTeX MathML
    // hiding profile. This is trusted host CSS, not renderer-supplied style.
    for part in [
        ".",
        scope,
        " .",
        accessible.as_str(),
        "{border:0;-webkit-clip-path:inset(50%);clip-path:inset(50%);height:1px;overflow:hidden;padding:0;position:absolute;width:1px}\n",
    ] {
        css(style, part, b)?;
    }
    classes.push(accessible);
    math.markup.fragment = HtmlFragment { root, nodes };
    math.markup.policy = HtmlPolicy { classes };
    validate(
        &math.markup.fragment,
        math.markup.slot,
        &math.markup.policy,
        b,
    )?;
    Ok(GeneratedRange {
        first: offset,
        elements: (v + 2) as u64,
        visual_root,
        accessible_root,
    })
}
impl PreparedDisplay {
    /// Explicit MathML-only completion needs no Node, module files, KaTeX
    /// assets or fabricated renderer selection. Other preferences are rejected;
    /// this is not an implicit fallback from a failed/unavailable renderer.
    pub fn into_mathml_composite(
        self,
        scope: &str,
        b: &mut Budget,
    ) -> Result<ComposedMath<'static, 'static>, Error> {
        b.poll()?;
        if !matches!(&self.tex, TexPreparation::MathMLOnly) {
            return Err(Error::Preference);
        }
        let scope = own_scope(scope, b)?;
        let PreparedDisplay {
            doc_node,
            display,
            mathml,
            tex,
        } = self;
        let math = mathml.into_html(b)?;
        let expected = match display {
            Display::Inline => HtmlSlot::Phrasing,
            Display::Block => HtmlSlot::Block,
        };
        if math.markup.slot != expected {
            return Err(Error::Shape);
        }
        b.poll()?;
        Ok(ComposedMath {
            math,
            doc_node,
            display,
            tex,
            representation: Representation::MathML,
            fallback: None,
            generated: None,
            stylesheet: String::new(),
            assets: None,
            selection: None,
            scope,
            observations: None,
            termination_failure: None,
        })
    }
}
impl<'c, 'r, E> OwnedGeneration<'c, 'r, E> {
    /// No renderer failure is promoted to fallback here. Deferred retains all
    /// original attempt data for a separate policy decision. Composition charges
    /// new work/storage and only appended CSS bytes, never re-emitting prior CSS.
    pub fn into_composite(self, b: &mut Budget) -> Result<Composition<'c, 'r, E>, Error> {
        self.into_composite_with_policy(CompletionPolicy::Strict, b)
    }
    /// Complete only explicitly permitted ordinary failures using the original
    /// independent MathML. Disallowed fallback attempts remain Deferred; a
    /// stopped parent Budget returns Err. Existing Visual/NotRequested handling
    /// is unchanged, so Ready is never an execution or cleanup certificate.
    pub fn into_composite_with_policy(
        self,
        policy: CompletionPolicy,
        b: &mut Budget,
    ) -> Result<Composition<'c, 'r, E>, Error> {
        b.poll()?;
        let fallback = policy == CompletionPolicy::OrdinaryMathmlFallback
            && self.termination_failure == Some(false)
            && matches!(
                &self.attempt,
                Attempt::Other(Outcome::Unavailable(_) | Outcome::RenderError)
            );
        if !fallback && !matches!(&self.attempt, Attempt::Visual(_) | Attempt::NotRequested) {
            return Ok(Composition::Deferred(self));
        }
        if fallback {
            b.charge(Resource::Diagnostics, 1)?;
        }
        let Self {
            prepared,
            config,
            controls,
            scope,
            attempt,
            observations,
            termination_failure,
        } = self;
        let PreparedDisplay {
            doc_node,
            display,
            mathml,
            tex,
        } = prepared;
        let mut math = mathml.into_html(b)?;
        let expected = match display {
            Display::Inline => HtmlSlot::Phrasing,
            Display::Block => HtmlSlot::Block,
        };
        if math.markup.slot != expected {
            return Err(Error::Shape);
        }
        let (representation, generated, stylesheet, assets, fallback) = match attempt {
            Attempt::Visual(visual) => {
                let (visual, assets) = visual.into_parts();
                let (visual, mut stylesheet) = visual.into_parts();
                let generated = join(&mut math, visual, &scope, &mut stylesheet, display, b)?;
                (
                    Representation::Dual,
                    Some(generated),
                    stylesheet,
                    Some(assets),
                    None,
                )
            }
            Attempt::NotRequested => (Representation::MathML, None, String::new(), None, None),
            Attempt::Other(Outcome::Unavailable(cause)) if fallback => (
                Representation::MathML,
                None,
                String::new(),
                None,
                Some(FallbackReason::Unavailable(cause)),
            ),
            Attempt::Other(Outcome::RenderError) if fallback => (
                Representation::MathML,
                None,
                String::new(),
                None,
                Some(FallbackReason::RenderError),
            ),
            _ => return Err(Error::Shape),
        };
        b.poll()?;
        Ok(Composition::Ready(ComposedMath {
            math,
            doc_node,
            display,
            tex,
            representation,
            fallback,
            generated,
            stylesheet,
            assets,
            selection: Some(RendererSelection { config, controls }),
            scope,
            observations,
            termination_failure,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::budget::Limits;
    #[test]
    fn composite_class_collisions_are_rejected_before_join() -> Result<(), Error> {
        let policy = |name: &str| HtmlPolicy {
            classes: vec![name.into()],
        };
        let mut b = Budget::new(Limits {
            work: 10000,
            ..Limits::default()
        });
        check_math_classes(&policy("sentence"), &mut b)?;
        assert!(matches!(
            check_math_classes(&policy("nepl-math-owned"), &mut b),
            Err(Error::ReservedClass)
        ));
        check_accessible_class(
            &policy("nepl-math-owned-s0"),
            "nepl-math-owned-accessible",
            &mut b,
        )?;
        assert!(matches!(
            check_accessible_class(
                &policy("nepl-math-owned-accessible"),
                "nepl-math-owned-accessible",
                &mut b
            ),
            Err(Error::ReservedClass)
        ));
        let mut stopped = Budget::new(Limits {
            work: 0,
            ..Limits::default()
        });
        assert!(matches!(
            check_math_classes(&policy("nepl-math-owned"), &mut stopped),
            Err(Error::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        Ok(())
    }
}

pub mod serialize;

pub(in crate::doc::math) mod import;

#[cfg(test)]
mod fallback_tests;
