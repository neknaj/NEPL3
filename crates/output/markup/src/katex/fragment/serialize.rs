use super::*;
use crate::{
    output::Output,
    text::{TextContext, TextError, escape},
};

/// A visual-only pair. Bind the fixed KaTeX CSS/fonts and independent accessible
/// MathML before document admission; HTML alone is not a complete Math artifact.
pub struct Rendered {
    html: String,
    stylesheet: String,
}
impl Rendered {
    pub fn html(&self) -> &str {
        &self.html
    }
    pub fn stylesheet(&self) -> &str {
        &self.stylesheet
    }
    pub fn into_parts(self) -> (String, String) {
        (self.html, self.stylesheet)
    }
}
fn text(out: &mut Output, value: &str, context: TextContext, b: &mut Budget) -> Result<(), Error> {
    let escaped = escape(value, context, b).map_err(|e| match e {
        TextError::Stopped(reason) => Error::Stopped(reason),
        // Checked borrows the same immutable input, whose characters passed
        // validation. Keep an explicit error even for this unreachable case.
        TextError::InvalidCharacter { byte, .. } => Error::Text { node: 0, byte },
    })?;
    out.precharged(&escaped, b)?;
    Ok(())
}
fn attr(out: &mut Output, name: &str, value: &str, b: &mut Budget) -> Result<(), Error> {
    out.literal(" ", b)?;
    out.literal(name, b)?;
    out.literal("=\"", b)?;
    text(out, value, TextContext::Attribute, b)?;
    out.literal("\"", b)?;
    Ok(())
}
fn class(out: &mut Output, scope: &str, id: usize, b: &mut Budget) -> Result<(), Error> {
    out.literal(scope, b)?;
    out.literal("-n", b)?;
    let mut value = id as u64;
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let number = core::str::from_utf8(&digits[start..]).map_err(|_| Error::Policy)?;
    out.literal(number, b)?;
    Ok(())
}
/// Serialize without style attributes, script, URLs or reparsing raw HTML.
/// Each nonempty computed style gets an occurrence-specific class; declaration
/// order/values remain intact. Generated declarations use author !important to
/// retain precedence over the bound KaTeX stylesheet's normal declarations.
/// The host must verify that fixed CSS has no important declaration for these
/// properties, exclude competing author rules for this scope, and allocate a
/// distinct scope per insertion. This does not certify arbitrary CSS cascades.
///
/// O(N + E + output bytes), O(N + output bytes) storage. No partial pair escapes
/// on failure; the caller's stopped budget is never recovered into a result.
pub fn serialize(checked: &Checked<'_>, b: &mut Budget) -> Result<Rendered, Error> {
    b.poll()?;
    let nodes = &checked.fragment.nodes;
    let count = nodes
        .len()
        .checked_mul(2)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    let mut pending = scratch(count, (0_usize, false, 0_u64), b)?;
    pending.clear();
    let mut html = Output::default();
    let mut css = Output::default();
    b.observe_depth(1)?;
    b.charge(Resource::Nodes, 1)?;
    html.literal("<span", b)?;
    attr(&mut html, "class", checked.scope, b)?;
    html.literal(" aria-hidden=\"true\">", b)?;
    pending.push((nodes.len() - 1, false, 2));
    while let Some((id, close, depth)) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        let node = &nodes[id];
        if close {
            html.literal(
                if matches!(node, Node::Svg { .. }) {
                    "</svg>"
                } else {
                    "</span>"
                },
                b,
            )?;
            continue;
        }
        b.observe_depth(depth)?;
        b.charge(Resource::Nodes, 1)?;
        let children = match node {
            Node::Text(value) => {
                text(&mut html, value, TextContext::Content, b)?;
                None
            }
            Node::Span {
                classes,
                style,
                aria_hidden,
                children,
            } => {
                html.literal("<span class=\"", b)?;
                html.literal(classes, b)?;
                if !style.is_empty() {
                    if !classes.is_empty() {
                        html.literal(" ", b)?;
                    }
                    class(&mut html, checked.scope, id, b)?;
                    css.literal(".", b)?;
                    css.literal(checked.scope, b)?;
                    css.literal(" .", b)?;
                    class(&mut css, checked.scope, id, b)?;
                    css.literal("{", b)?;
                    for declaration in style.split_terminator(';') {
                        css.literal(declaration, b)?;
                        css.literal("!important;", b)?;
                    }
                    css.literal("}\n", b)?;
                }
                html.literal("\"", b)?;
                if let Some(hidden) = aria_hidden {
                    attr(
                        &mut html,
                        "aria-hidden",
                        if *hidden { "true" } else { "false" },
                        b,
                    )?;
                }
                html.literal(">", b)?;
                Some(children)
            }
            Node::Svg {
                width,
                height,
                view_box,
                aspect,
                children,
            } => {
                html.literal("<svg xmlns=\"http://www.w3.org/2000/svg\"", b)?;
                attr(&mut html, "width", width, b)?;
                attr(&mut html, "height", height, b)?;
                if let Some(value) = view_box {
                    attr(&mut html, "viewBox", value, b)?;
                }
                if let Some(value) = aspect {
                    attr(&mut html, "preserveAspectRatio", value.value(), b)?;
                }
                html.literal(">", b)?;
                Some(children)
            }
            Node::Path { data } => {
                html.literal("<path", b)?;
                attr(&mut html, "d", data, b)?;
                html.literal("></path>", b)?;
                None
            }
            Node::Line {
                x1,
                y1,
                x2,
                y2,
                stroke_width,
            } => {
                html.literal("<line", b)?;
                for (name, value) in [
                    ("x1", x1.value()),
                    ("y1", y1.value()),
                    ("x2", x2.value()),
                    ("y2", y2.value()),
                    ("stroke-width", stroke_width.as_str()),
                ] {
                    attr(&mut html, name, value, b)?;
                }
                html.literal("></line>", b)?;
                None
            }
        };
        if let Some(children) = children {
            // At most one open/close event per input node fits the precharged
            // capacity. Validation excludes shared and unreachable nodes.
            pending.push((id, true, depth));
            let child_depth = depth
                .checked_add(1)
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            for child in children.iter().rev() {
                pending.push((*child as usize, false, child_depth));
            }
        }
    }
    html.literal("</span>", b)?;
    b.poll()?;
    Ok(Rendered {
        html: html.finish(),
        stylesheet: css.finish(),
    })
}
