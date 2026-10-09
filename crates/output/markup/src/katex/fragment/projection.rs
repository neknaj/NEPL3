//! Consuming native visual projection; not complete Math artifact admission.
use super::{
    Checked, Error, Fragment, Node,
    owned::{copy, reserve},
};
use crate::{html::*, output::Output};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    Stopped(StopReason),
    Visual(Error),
    Html(HtmlError),
}
impl From<StopReason> for ProjectionError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl From<Error> for ProjectionError {
    fn from(value: Error) -> Self {
        match value {
            Error::Stopped(s) => Self::Stopped(s),
            value => Self::Visual(value),
        }
    }
}
impl From<HtmlError> for ProjectionError {
    fn from(value: HtmlError) -> Self {
        match value {
            HtmlError::Stopped(s) => Self::Stopped(s),
            value => Self::Html(value),
        }
    }
}

/// Typed visual-only HTML and its exact generated stylesheet. Resources,
/// document-wide unique scopes, accessible MathML and fidelity are not proved.
/// Extracting parts consumes this association; later mutation/insertion requires
/// the destination's complete markup and resource validation.
/// ```compile_fail
/// fn change(p: &nepl3_markup::katex::fragment::ProjectedVisual) {
///     p.request().fragment.nodes.clear();
/// }
/// ```
/// ```compile_fail
/// fn copy(p: &nepl3_markup::katex::fragment::ProjectedVisual) -> nepl3_markup::katex::fragment::ProjectedVisual { p.clone() }
/// ```
pub struct ProjectedVisual {
    request: HtmlRequest,
    stylesheet: String,
}
impl ProjectedVisual {
    pub fn request(&self) -> &HtmlRequest {
        &self.request
    }
    pub fn stylesheet(&self) -> &str {
        &self.stylesheet
    }
    pub fn into_parts(self) -> (HtmlRequest, String) {
        (self.request, self.stylesheet)
    }
}
// Original serializer visits root first, then children in their displayed order.
// Arena order is postorder and cannot substitute for that CSS rule order.
pub(super) fn stylesheet(checked: &Checked<'_>, b: &mut Budget) -> Result<String, ProjectionError> {
    let nodes = &checked.fragment.nodes;
    let mut stack = reserve(nodes.len(), b)?;
    let root = nodes.len().checked_sub(1).ok_or(Error::Root)?;
    stack.push(root);
    let mut css = Output::default();
    while let Some(index) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        let children = match &nodes[index] {
            Node::Span {
                style, children, ..
            } => {
                if !style.is_empty() {
                    super::serialize::style_rule(&mut css, checked.scope, index, style, b)?;
                }
                Some(children)
            }
            Node::Svg { children, .. } => Some(children),
            _ => None,
        };
        if let Some(children) = children {
            for child in children.iter().rev() {
                b.charge(Resource::Work, 1)?;
                stack.push(*child as usize);
            }
        }
    }
    Ok(css.finish())
}
fn generated_class(scope: &str, index: usize, b: &mut Budget) -> Result<String, ProjectionError> {
    let capacity = scope
        .len()
        .checked_add(22)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, capacity as u64)?;
    b.charge(Resource::Work, capacity as u64)?;
    let mut name = String::new();
    name.try_reserve_exact(capacity)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    write!(&mut name, "{scope}-n{index}").map_err(|_| Error::Policy)?;
    Ok(name)
}
fn class_tokens(value: &str, styled: bool, b: &mut Budget) -> Result<Vec<String>, ProjectionError> {
    b.charge(Resource::Work, value.len() as u64 + 1)?;
    let count = if value.is_empty() {
        0
    } else {
        value.split(' ').count()
    };
    let capacity = count
        .checked_add(usize::from(styled))
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let mut tokens: Vec<String> = reserve(capacity, b)?;
    if !value.is_empty() {
        b.charge(Resource::Work, value.len() as u64)?;
        for token in value.split(' ') {
            b.charge(Resource::Work, 1)?;
            let mut duplicate = false;
            for previous in &tokens {
                b.charge(Resource::Work, previous.len().min(token.len()) as u64 + 1)?;
                if previous == token {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                tokens.push(copy(token, b)?);
            }
        }
    }
    Ok(tokens)
}
pub(super) fn project(
    fragment: Fragment,
    classes: Vec<String>,
    scope: String,
    stylesheet: String,
    b: &mut Budget,
) -> Result<ProjectedVisual, ProjectionError> {
    b.poll()?;
    let count = fragment.nodes.len();
    let total = count
        .checked_add(1)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let policy_capacity = classes
        .len()
        .checked_add(total)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let mut policy = reserve(policy_capacity, b)?;
    for class in classes {
        b.charge(Resource::Work, 1)?;
        policy.push(class);
    }
    policy.push(copy(&scope, b)?);
    let mut nodes = reserve(total, b)?;
    for (index, node) in fragment.nodes.into_iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        nodes.push(match node {
            Node::Text(text) => HtmlNode::Text { text },
            Node::Span {
                classes,
                style,
                aria_hidden,
                children,
            } => {
                let mut values = class_tokens(&classes, !style.is_empty(), b)?;
                if !style.is_empty() {
                    let name = generated_class(&scope, index, b)?;
                    policy.push(copy(&name, b)?);
                    values.push(name);
                }
                let mut attributes = reserve(2, b)?;
                if !values.is_empty() {
                    attributes.push(HtmlAttribute::Class { values });
                }
                if let Some(value) = aria_hidden {
                    attributes.push(HtmlAttribute::AriaHidden { value });
                }
                HtmlNode::Element {
                    tag: HtmlTag::Span,
                    attributes,
                    children,
                }
            }
            Node::Svg {
                width,
                height,
                view_box,
                aspect,
                children,
            } => HtmlNode::SvgElement {
                element: HtmlSvgElement::Svg {
                    width,
                    height,
                    view_box,
                    aspect,
                },
                children,
            },
            Node::Path { data } => HtmlNode::SvgElement {
                element: HtmlSvgElement::Path { data },
                children: Vec::new(),
            },
            Node::Line {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
            } => HtmlNode::SvgElement {
                element: HtmlSvgElement::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke_width,
                },
                children: Vec::new(),
            },
        });
    }
    b.charge(Resource::Nodes, 1)?;
    let mut values = reserve(1, b)?;
    values.push(scope);
    let mut attributes = reserve(2, b)?;
    attributes.push(HtmlAttribute::Class { values });
    attributes.push(HtmlAttribute::AriaHidden { value: true });
    let mut children = reserve(1, b)?;
    children.push(count.checked_sub(1).ok_or(Error::Root)? as u64);
    nodes.push(HtmlNode::Element {
        tag: HtmlTag::Span,
        attributes,
        children,
    });
    let request = HtmlRequest {
        fragment: HtmlFragment {
            root: count as u64,
            nodes,
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: policy },
    };
    validate(&request.fragment, request.slot, &request.policy, b)?;
    Ok(ProjectedVisual {
        request,
        stylesheet,
    })
}
