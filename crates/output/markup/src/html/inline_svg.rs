use super::{HtmlError, HtmlSvgAspect, HtmlSvgEndpoint};
use crate::{katex::svg::Element, output::Output, text::TextContext};
use alloc::string::String;
use nepl3_core::budget::Budget;

/// Closed inline generated-Math SVG vocabulary. The containing HTML arena owns
/// and validates every child edge; this type alone grants no serialization proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HtmlSvgElement {
    Svg {
        width: String,
        height: String,
        view_box: Option<String>,
        aspect: Option<HtmlSvgAspect>,
    },
    Path {
        data: String,
    },
    Line {
        x1: HtmlSvgEndpoint,
        y1: HtmlSvgEndpoint,
        x2: HtmlSvgEndpoint,
        y2: HtmlSvgEndpoint,
        stroke_width: String,
    },
}
impl HtmlSvgElement {
    pub(crate) fn attributes(&self) -> Element<'_> {
        match self {
            Self::Svg {
                width,
                height,
                view_box,
                aspect,
            } => Element::Svg {
                width,
                height,
                view_box: view_box.as_deref(),
                aspect: *aspect,
            },
            Self::Path { data } => Element::Path { data },
            Self::Line {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
            } => Element::Line {
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
                stroke_width,
            },
        }
    }
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Svg { .. } => "svg",
            Self::Path { .. } => "path",
            Self::Line { .. } => "line",
        }
    }
}
fn attr(
    out: &mut Output,
    name: &str,
    value: &str,
    node: u64,
    b: &mut Budget,
) -> Result<(), HtmlError> {
    out.literal(" ", b)?;
    out.literal(name, b)?;
    out.literal("=\"", b)?;
    out.text(value, TextContext::Attribute, node, b)?;
    out.literal("\"", b)?;
    Ok(())
}
pub(super) fn open(
    element: &HtmlSvgElement,
    out: &mut Output,
    node: u64,
    b: &mut Budget,
) -> Result<(), HtmlError> {
    out.literal("<", b)?;
    out.literal(element.name(), b)?;
    match element {
        HtmlSvgElement::Svg {
            width,
            height,
            view_box,
            aspect,
        } => {
            attr(out, "xmlns", "http://www.w3.org/2000/svg", node, b)?;
            attr(out, "width", width, node, b)?;
            attr(out, "height", height, node, b)?;
            if let Some(value) = view_box {
                attr(out, "viewBox", value, node, b)?;
            }
            if let Some(value) = aspect {
                attr(out, "preserveAspectRatio", value.value(), node, b)?;
            }
        }
        HtmlSvgElement::Path { data } => attr(out, "d", data, node, b)?,
        HtmlSvgElement::Line {
            x1,
            y1,
            x2,
            y2,
            stroke_width,
        } => {
            for (name, value) in [("x1", x1), ("y1", y1), ("x2", x2), ("y2", y2)] {
                attr(out, name, value.value(), node, b)?;
            }
            attr(out, "stroke-width", stroke_width, node, b)?;
        }
    }
    out.literal(">", b)?;
    Ok(())
}
