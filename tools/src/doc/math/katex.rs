//! Host JSON boundary for the finite visual tree emitted by tools/math/katex/parse.mjs.
//! The outer transport must cap bytes before JSON decoding and own identity,
//! cancellation and diagnostics. JSON is not the portable NEPL3 operation ABI.
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_markup::katex::fragment::{self, Aspect, Endpoint, Fragment, Node, Policy, Rendered};
use serde_json::{Map, Value};

#[derive(Debug)]
pub enum Error {
    Shape,
    Stopped(StopReason),
    Markup(fragment::Error),
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
type Fields = Map<String, Value>;
fn take(fields: &mut Fields, name: &str, b: &mut Budget) -> Result<Value, Error> {
    b.charge(Resource::Work, 1)?;
    fields.remove(name).ok_or(Error::Shape)
}
fn string(value: Value) -> Result<String, Error> {
    match value {
        Value::String(s) => Ok(s),
        _ => Err(Error::Shape),
    }
}
fn optional(value: Value) -> Result<Option<String>, Error> {
    match value {
        Value::Null => Ok(None),
        value => string(value).map(Some),
    }
}
fn fields(value: Value, maximum: usize) -> Result<Fields, Error> {
    match value {
        Value::Object(fields) if fields.len() <= maximum => Ok(fields),
        _ => Err(Error::Shape),
    }
}
fn reserve<T>(len: usize, b: &mut Budget) -> Result<Vec<T>, Error> {
    let bytes = len
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(out)
}
fn children(value: Value, b: &mut Budget) -> Result<Vec<u64>, Error> {
    let Value::Array(values) = value else {
        return Err(Error::Shape);
    };
    let mut out = reserve(values.len(), b)?;
    for value in values {
        b.charge(Resource::Work, 1)?;
        out.push(value.as_u64().ok_or(Error::Shape)?);
    }
    Ok(out)
}
fn endpoint(value: Value) -> Result<Endpoint, Error> {
    match string(value)?.as_str() {
        "0" => Ok(Endpoint::Zero),
        "100%" => Ok(Endpoint::Full),
        _ => Err(Error::Shape),
    }
}
fn node(value: Value, b: &mut Budget) -> Result<Node, Error> {
    let mut f = fields(value, 7)?;
    let kind = string(take(&mut f, "kind", b)?)?;
    let out = match kind.as_str() {
        "text" => Node::Text(string(take(&mut f, "text", b)?)?),
        "span" => Node::Span {
            classes: string(take(&mut f, "classes", b)?)?,
            style: string(take(&mut f, "style", b)?)?,
            aria_hidden: match take(&mut f, "aria_hidden", b)? {
                Value::Null => None,
                Value::Bool(hidden) => Some(hidden),
                _ => return Err(Error::Shape),
            },
            children: children(take(&mut f, "children", b)?, b)?,
        },
        "svg" => Node::Svg {
            width: string(take(&mut f, "width", b)?)?,
            height: string(take(&mut f, "height", b)?)?,
            view_box: optional(take(&mut f, "view_box", b)?)?,
            aspect: match optional(take(&mut f, "aspect", b)?)?.as_deref() {
                None => None,
                Some("none") => Some(Aspect::None),
                Some("xMinYMin slice") => Some(Aspect::MinSlice),
                Some("xMidYMin slice") => Some(Aspect::MidSlice),
                Some("xMaxYMin slice") => Some(Aspect::MaxSlice),
                _ => return Err(Error::Shape),
            },
            children: children(take(&mut f, "children", b)?, b)?,
        },
        "path" => Node::Path {
            data: string(take(&mut f, "data", b)?)?,
        },
        "line" => Node::Line {
            x1: endpoint(take(&mut f, "x1", b)?)?,
            y1: endpoint(take(&mut f, "y1", b)?)?,
            x2: endpoint(take(&mut f, "x2", b)?)?,
            y2: endpoint(take(&mut f, "y2", b)?)?,
            stroke_width: string(take(&mut f, "stroke_width", b)?)?,
        },
        _ => return Err(Error::Shape),
    };
    if !f.is_empty() {
        return Err(Error::Shape);
    }
    Ok(out)
}
/// Consume already byte-bounded host JSON, validate every field and then the
/// closed markup tree, and serialize the visual pair. Strings move without
/// copying; output vectors and subsequent markup operations are budgeted.
/// Unknown fields/kinds, malformed content and stops fail the entire operation.
/// Class/asset binding, source identity and accessible MathML remain host duties.
pub fn render(value: Value, policy: &Policy<'_>, b: &mut Budget) -> Result<Rendered, Error> {
    b.poll()?;
    let mut f = fields(value, 2)?;
    if string(take(&mut f, "kind", b)?)? != "parsed-unchecked" {
        return Err(Error::Shape);
    }
    let Value::Array(values) = take(&mut f, "nodes", b)? else {
        return Err(Error::Shape);
    };
    if !f.is_empty() {
        return Err(Error::Shape);
    }
    let mut nodes = reserve(values.len(), b)?;
    for value in values {
        nodes.push(node(value, b)?);
    }
    let fragment = Fragment { nodes };
    let map = |e| match e {
        fragment::Error::Stopped(reason) => Error::Stopped(reason),
        e => Error::Markup(e),
    };
    let checked = fragment::validate(&fragment, policy, b).map_err(map)?;
    fragment::serialize(&checked, b).map_err(map)
}
