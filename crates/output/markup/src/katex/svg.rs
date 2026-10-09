//! Attribute vocabulary shared by generated-Math SVG projections.
//! This validates attributes only: tree admission, namespace transitions,
//! resource identity, fidelity and serialization remain separate obligations.
use super::{length, path_data, view_box};
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Aspect {
    None,
    MinSlice,
    MidSlice,
    MaxSlice,
}
impl Aspect {
    pub(crate) fn value(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MinSlice => "xMinYMin slice",
            Self::MidSlice => "xMidYMin slice",
            Self::MaxSlice => "xMaxYMin slice",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint {
    Zero,
    Full,
}
impl Endpoint {
    pub(crate) fn value(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::Full => "100%",
        }
    }
}
/// Borrowed attributes, without children or arbitrary names. Existing visual
/// nodes and future typed projections can use the same rules without cloning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Element<'a> {
    Svg {
        width: &'a str,
        height: &'a str,
        view_box: Option<&'a str>,
        aspect: Option<Aspect>,
    },
    Path {
        data: &'a str,
    },
    Line {
        x1: Endpoint,
        y1: Endpoint,
        x2: Endpoint,
        y2: Endpoint,
        stroke_width: &'a str,
    },
}
/// Check the closed attribute profile without allocating, producing output or
/// charging tree Nodes/Depth. Every call polls the sticky parent budget and
/// charges Work before scanning. Invalid attributes return false unchanged;
/// callers must reject them rather than strip or repair them silently.
///
/// Width permits 100% or a nonnegative bounded em length; height/stroke width
/// permit only the latter (including unitless zero). Aspect requires viewBox.
/// The path and viewport use the existing generated-Math lexical profiles.
pub fn validate_attributes(element: Element<'_>, budget: &mut Budget) -> Result<bool, StopReason> {
    budget.charge(Resource::Work, 1)?;
    match element {
        Element::Svg {
            width,
            height,
            view_box: viewport,
            aspect,
        } => {
            let work = (width.len() as u64)
                .checked_add(height.len() as u64)
                .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
            budget.charge(Resource::Work, work)?;
            Ok((width == "100%" || length(width, false))
                && length(height, false)
                && match viewport {
                    Some(value) => view_box(value, budget)?,
                    None => aspect.is_none(),
                })
        }
        Element::Path { data } => path_data(data, budget),
        Element::Line { stroke_width, .. } => {
            budget.charge(Resource::Work, stroke_width.len() as u64)?;
            Ok(length(stroke_width, false))
        }
    }
}
