//! Fallible arena copying for repeated display of prepared guest output.
use super::*;
use nepl3_core::budget::{Budget, Resource, StopReason};

fn array<T, U>(
    input: &[T],
    b: &mut Budget,
    mut item: impl FnMut(&T, &mut Budget) -> Result<U, StopReason>,
) -> Result<Vec<U>, StopReason> {
    let bytes = input
        .len()
        .checked_mul(core::mem::size_of::<U>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(input.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    for value in input {
        b.charge(Resource::Work, 1)?;
        result.push(item(value, b)?);
    }
    Ok(result)
}
fn text(input: &str, b: &mut Budget) -> Result<String, StopReason> {
    b.charge(Resource::Work, input.len() as u64)?;
    b.charge(Resource::AllocationUnits, input.len() as u64)?;
    let mut output = String::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    output.push_str(input);
    Ok(output)
}
fn optional(input: &Option<String>, b: &mut Budget) -> Result<Option<String>, StopReason> {
    input.as_deref().map(|value| text(value, b)).transpose()
}
fn href(input: &HtmlHref, b: &mut Budget) -> Result<HtmlHref, StopReason> {
    Ok(match input {
        HtmlHref::BetweenArtifacts {
            source,
            target,
            fragment,
        } => HtmlHref::BetweenArtifacts {
            source: text(source, b)?,
            target: text(target, b)?,
            fragment: optional(fragment, b)?,
        },
        HtmlHref::Fragment { id } => HtmlHref::Fragment { id: text(id, b)? },
        HtmlHref::Artifact { path, fragment } => HtmlHref::Artifact {
            path: text(path, b)?,
            fragment: optional(fragment, b)?,
        },
        HtmlHref::External { uri } => HtmlHref::External { uri: text(uri, b)? },
    })
}
fn attribute(input: &HtmlAttribute, b: &mut Budget) -> Result<HtmlAttribute, StopReason> {
    Ok(match input {
        HtmlAttribute::Id { value } => HtmlAttribute::Id {
            value: text(value, b)?,
        },
        HtmlAttribute::Class { values } => HtmlAttribute::Class {
            values: array(values, b, |s, b| text(s, b))?,
        },
        HtmlAttribute::Lang { value } => HtmlAttribute::Lang {
            value: text(value, b)?,
        },
        HtmlAttribute::Role { value } => HtmlAttribute::Role { value: *value },
        HtmlAttribute::AriaLabel { value } => HtmlAttribute::AriaLabel {
            value: text(value, b)?,
        },
        HtmlAttribute::AriaLevel { value } => HtmlAttribute::AriaLevel { value: *value },
        HtmlAttribute::DataId { value } => HtmlAttribute::DataId {
            value: text(value, b)?,
        },
        HtmlAttribute::DataGroup { value } => HtmlAttribute::DataGroup {
            value: text(value, b)?,
        },
        HtmlAttribute::Href { value } => HtmlAttribute::Href {
            value: href(value, b)?,
        },
        HtmlAttribute::Src { path } => HtmlAttribute::Src {
            path: text(path, b)?,
        },
        HtmlAttribute::Alt { value } => HtmlAttribute::Alt {
            value: text(value, b)?,
        },
        HtmlAttribute::Width { value } => HtmlAttribute::Width { value: *value },
        HtmlAttribute::Height { value } => HtmlAttribute::Height { value: *value },
        HtmlAttribute::Start { value } => HtmlAttribute::Start { value: *value },
        HtmlAttribute::Scope { value } => HtmlAttribute::Scope { value: *value },
    })
}
fn math(
    input: &crate::mathml::Attribute,
    b: &mut Budget,
) -> Result<crate::mathml::Attribute, StopReason> {
    use crate::mathml::Attribute as A;
    Ok(match input {
        A::Display(value) => A::Display(*value),
        A::NormalIdentifier => A::NormalIdentifier,
        A::Stretchy(value) => A::Stretchy(*value),
        A::Symmetric(value) => A::Symmetric(*value),
        A::LargeOperator(value) => A::LargeOperator(*value),
        A::MovableLimits(value) => A::MovableLimits(*value),
        A::Form(value) => A::Form(*value),
        A::Width(value) => A::Width(text(value, b)?),
        A::Height(value) => A::Height(text(value, b)?),
        A::Depth(value) => A::Depth(text(value, b)?),
    })
}
impl HtmlRequest {
    /// Copy the raw flat arena with charged, fallible allocations. Node indices,
    /// sharing, attributes and policy order are preserved. This operation grants
    /// no validation proof; the caller validates the completed destination.
    pub fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, StopReason> {
        b.poll()?;
        let nodes = array(&self.fragment.nodes, b, |node, b| {
            b.charge(Resource::Nodes, 1)?;
            Ok(match node {
                HtmlNode::Text { text: value } => HtmlNode::Text {
                    text: text(value, b)?,
                },
                HtmlNode::Element {
                    tag,
                    attributes,
                    children,
                } => HtmlNode::Element {
                    tag: *tag,
                    attributes: array(attributes, b, attribute)?,
                    children: array(children, b, |index, _| Ok(*index))?,
                },
                HtmlNode::MathElement {
                    tag,
                    attributes,
                    children,
                } => HtmlNode::MathElement {
                    tag: *tag,
                    attributes: array(attributes, b, math)?,
                    children: array(children, b, |index, _| Ok(*index))?,
                },
            })
        })?;
        let classes = array(&self.policy.classes, b, |s, b| text(s, b))?;
        b.poll()?;
        Ok(Self {
            fragment: HtmlFragment {
                root: self.fragment.root,
                nodes,
            },
            slot: self.slot,
            policy: HtmlPolicy { classes },
        })
    }
}
