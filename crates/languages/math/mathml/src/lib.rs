#![no_std]
//! Structural display, never evaluation. Foreign annotations require separately
//! prepared phrasing content from an explicitly supplied host renderer.
extern crate alloc;
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_core::syntax::ForeignClosure;
use nepl3_markup::html::{self, HtmlRequest, HtmlSlot};
use nepl3_markup::mathml::{self, Attribute, Display, Fragment, Node, Tag};
use nepl3_math_core::{
    check::CheckedExpression,
    model::{MathKind as K, MathRoot},
    number,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Number(number::DecimalPrintError),
    Markup(mathml::Error),
    AnnotationRequiresPreparation(u64),
    Reference(u64),
    AnnotationSlot(u64),
}
/// Host failures retain their original type; resource stops remain terminal.
#[derive(Debug)]
pub enum AnnotationFailure<E> {
    Render(Error),
    Guest(E),
    Stopped(StopReason),
}
impl<E> From<Error> for AnnotationFailure<E> {
    fn from(e: Error) -> Self {
        match e {
            Error::Stopped(s) => Self::Stopped(s),
            e => Self::Render(e),
        }
    }
}
impl<E> From<StopReason> for AnnotationFailure<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<mathml::Error> for AnnotationFailure<E> {
    fn from(e: mathml::Error) -> Self {
        Error::from(e).into()
    }
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<mathml::Error> for Error {
    fn from(e: mathml::Error) -> Self {
        match e {
            mathml::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Markup(e),
        }
    }
}
/// Node roots correspond to the input arena indices. They preserve an explicit
/// route to input source/origin records; they are not a complete SourceMap proof.
pub struct Rendered {
    pub fragment: Fragment,
    pub node_roots: Vec<u64>,
}

fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    if v.len() == v.capacity() {
        let capacity = v
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(4))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(
            Resource::AllocationUnits,
            ((capacity - v.capacity()) as u64).saturating_mul(core::mem::size_of::<T>() as u64),
        )?;
        b.charge(Resource::Work, v.len() as u64)?;
        v.try_reserve_exact(capacity - v.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    v.push(item);
    Ok(())
}
struct Builder<'a> {
    nodes: Vec<Node>,
    b: &'a mut Budget,
}
impl Builder<'_> {
    fn add(&mut self, node: Node) -> Result<u64, Error> {
        self.b.charge(Resource::Nodes, 1)?;
        let id = self.nodes.len() as u64;
        push(&mut self.nodes, node, self.b)?;
        Ok(id)
    }
    fn element(&mut self, tag: Tag, children: &[u64]) -> Result<u64, Error> {
        let mut ids = Vec::new();
        for id in children {
            push(&mut ids, *id, self.b)?;
        }
        self.add(Node::Element {
            tag,
            attributes: Vec::new(),
            children: ids,
        })
    }
    fn token(&mut self, tag: Tag, text: &str) -> Result<u64, Error> {
        self.b.charge(Resource::Work, text.len() as u64)?;
        self.b
            .charge(Resource::AllocationUnits, text.len() as u64)?;
        let mut copy = String::new();
        copy.try_reserve_exact(text.len())
            .map_err(|_| self.b.stop(StopReason::AllocationLimit))?;
        copy.push_str(text);
        let text = self.add(Node::Text(copy))?;
        self.element(tag, &[text])
    }
    fn fence(&mut self, open: &str, close: &str, value: u64) -> Result<u64, Error> {
        let a = self.token(Tag::Operator, open)?;
        let z = self.token(Tag::Operator, close)?;
        self.element(Tag::Row, &[a, value, z])
    }
    fn grouped(&mut self, id: u64, child: &K, parent: u8, equal: bool) -> Result<u64, Error> {
        let script = matches!(
            child,
            K::Pow { .. }
                | K::Subscript { .. }
                | K::Superscript { .. }
                | K::Scripts { .. }
                | K::Transpose { .. }
        );
        if power(child) < parent
            || (equal && ((parent < 60 && power(child) == parent) || (parent >= 50 && script)))
        {
            self.fence("(", ")", id)
        } else {
            Ok(id)
        }
    }
}
fn power(k: &K) -> u8 {
    match k {
        K::Number { value, .. } if value.numerator().is_negative() => 40,
        K::Let { .. } | K::Sequence { .. } | K::Sum { .. } | K::Integral { .. } => 0,
        K::Equal { .. } | K::Lt { .. } | K::Le { .. } => 10,
        K::Add { .. } | K::Sub { .. } => 20,
        K::Mul { .. } => 30,
        K::Neg { .. } => 40,
        K::Pow { .. } => 50,
        _ => 60,
    }
}
fn root(map: &[u64], id: u64) -> Result<u64, Error> {
    usize::try_from(id)
        .ok()
        .and_then(|i| map.get(i))
        .copied()
        .filter(|v| *v != u64::MAX)
        .ok_or(Error::Reference(id))
}

/// Render an exact checked expression without evaluation or source-text guessing.
/// O(input nodes + edges + copied text) plus numeric printing and final markup
/// validation; O(input + output nodes) storage. Shared children stay shared.
/// The caller's budget includes construction and validation; no partial result
/// is returned on a stop or an unprepared annotation.
pub fn render(
    input: &CheckedExpression<'_>,
    display: Display,
    b: &mut Budget,
) -> Result<Rendered, Error> {
    type NoRenderer =
        fn(&ForeignClosure, &mut Budget) -> Result<HtmlRequest, core::convert::Infallible>;
    build::<core::convert::Infallible, NoRenderer>(input, display, None, b).map_err(|e| match e {
        AnnotationFailure::Render(e) => e,
        AnnotationFailure::Stopped(s) => Error::Stopped(s),
        AnnotationFailure::Guest(never) => match never {},
    })
}
/// The host selects the annotation operation and receives the exact retained
/// ForeignClosure. It must lower/check/render that closure; Math never evaluates
/// it or guesses text. Returned markup must be phrasing and is revalidated here.
/// The callback uses the same sticky budget; its policy grants only CSS class
/// names, never raw HTML, scripts or unrestricted attributes. Final validation
/// checks identities across every expanded occurrence of all annotations.
/// In addition to ordinary rendering and callback costs, class policy merging
/// takes O(C² * L) work for C class entries of maximum length L. Mixed-markup
/// validation visits expanded occurrences under the same finite budget.
pub fn render_with_annotations<E, F>(
    input: &CheckedExpression<'_>,
    display: Display,
    renderer: &mut F,
    b: &mut Budget,
) -> Result<Rendered, AnnotationFailure<E>>
where
    F: FnMut(&ForeignClosure, &mut Budget) -> Result<HtmlRequest, E>,
{
    build(input, display, Some(renderer), b)
}
fn build<E, F>(
    input: &CheckedExpression<'_>,
    display: Display,
    mut renderer: Option<&mut F>,
    b: &mut Budget,
) -> Result<Rendered, AnnotationFailure<E>>
where
    F: FnMut(&ForeignClosure, &mut Budget) -> Result<HtmlRequest, E>,
{
    b.poll()?;
    let mut classes = Vec::new();
    let value = input.value();
    b.charge(Resource::Work, value.nodes.len() as u64)?;
    b.charge(
        Resource::AllocationUnits,
        (value.nodes.len() as u64).saturating_mul(8),
    )?;
    let mut map = Vec::new();
    map.try_reserve_exact(value.nodes.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    map.resize(value.nodes.len(), u64::MAX);
    let mut out = Builder {
        nodes: Vec::new(),
        b,
    };
    for &i in input.shape().postorder() {
        out.b.charge(Resource::Work, 1)?;
        let k = &value.nodes[i].kind;
        let id = match k {
            K::Number { value, .. } => {
                let text = number::print_decimal(value, out.b).map_err(|e| match e {
                    number::DecimalPrintError::Stopped(s) => Error::Stopped(s),
                    e => Error::Number(e),
                })?;
                let text = out.add(Node::Text(text))?;
                out.element(Tag::Number, &[text])?
            }
            K::Symbol { name } => out.token(Tag::Identifier, name)?,
            K::Text { text } => out.token(Tag::Text, text)?,
            K::Add { left, right }
            | K::Sub { left, right }
            | K::Mul { left, right }
            | K::Equal { left, right }
            | K::Lt { left, right }
            | K::Le { left, right } => {
                let p = power(k);
                let comparison = p == 10;
                let left_id = out.grouped(
                    root(&map, left.0)?,
                    &value.nodes[left.0 as usize].kind,
                    p,
                    comparison,
                )?;
                let right_id = out.grouped(
                    root(&map, right.0)?,
                    &value.nodes[right.0 as usize].kind,
                    p,
                    comparison || matches!(k, K::Sub { .. }),
                )?;
                let op = out.token(
                    Tag::Operator,
                    match k {
                        K::Add { .. } => "+",
                        K::Sub { .. } => "−",
                        K::Mul { .. } => "·",
                        K::Equal { .. } => "=",
                        K::Lt { .. } => "<",
                        _ => "≤",
                    },
                )?;
                out.element(Tag::Row, &[left_id, op, right_id])?
            }
            K::Frac { left, right } => {
                out.element(Tag::Fraction, &[root(&map, left.0)?, root(&map, right.0)?])?
            }
            K::Pow { left, right }
            | K::Subscript { left, right }
            | K::Superscript { left, right } => {
                let p = if matches!(k, K::Pow { .. }) { 50 } else { 60 };
                let base = out.grouped(
                    root(&map, left.0)?,
                    &value.nodes[left.0 as usize].kind,
                    p,
                    true,
                )?;
                out.element(
                    if matches!(k, K::Subscript { .. }) {
                        Tag::Sub
                    } else {
                        Tag::Sup
                    },
                    &[base, root(&map, right.0)?],
                )?
            }
            K::Neg { value: v } => {
                let op = out.token(Tag::Operator, "−")?;
                let child =
                    out.grouped(root(&map, v.0)?, &value.nodes[v.0 as usize].kind, 40, true)?;
                out.element(Tag::Row, &[op, child])?
            }
            K::Sqrt { value: v } => out.element(Tag::Sqrt, &[root(&map, v.0)?])?,
            K::Root { degree, radicand } => {
                out.element(Tag::Root, &[root(&map, radicand.0)?, root(&map, degree.0)?])?
            }
            K::Scripts { base, sub, sup } => {
                let base = out.grouped(
                    root(&map, base.0)?,
                    &value.nodes[base.0 as usize].kind,
                    60,
                    true,
                )?;
                out.element(Tag::SubSup, &[base, root(&map, sub.0)?, root(&map, sup.0)?])?
            }
            K::Fence {
                open,
                close,
                value: v,
            } => out.fence(open, close, root(&map, v.0)?)?,
            K::Transpose { value: v } => {
                let base =
                    out.grouped(root(&map, v.0)?, &value.nodes[v.0 as usize].kind, 60, true)?;
                let t = out.token(Tag::Identifier, "T")?;
                out.element(Tag::Sup, &[base, t])?
            }
            K::Det { value: v } => {
                let name = out.token(Tag::Identifier, "det")?;
                let child = out.fence("(", ")", root(&map, v.0)?)?;
                out.element(Tag::Row, &[name, child])?
            }
            K::Sequence { values } | K::Row { values } | K::Vector { values } => {
                let mut children = Vec::new();
                for v in values {
                    let mut child = root(&map, v.0)?;
                    if !matches!(k, K::Sequence { .. }) {
                        child = out.element(Tag::Cell, &[child])?;
                    }
                    if matches!(k, K::Vector { .. }) {
                        child = out.element(Tag::TableRow, &[child])?;
                    }
                    push(&mut children, child, out.b)?;
                }
                let tag = match k {
                    K::Row { .. } => Tag::TableRow,
                    K::Vector { .. } => Tag::Table,
                    _ => Tag::Row,
                };
                let n = out.element(tag, &children)?;
                if matches!(k, K::Vector { .. }) {
                    out.fence("(", ")", n)?
                } else {
                    n
                }
            }
            K::Matrix { rows } => {
                let mut children = Vec::new();
                for row in rows {
                    push(&mut children, root(&map, row.0)?, out.b)?;
                }
                let n = out.element(Tag::Table, &children)?;
                out.fence("(", ")", n)?
            }
            K::Let { name, init, body } => {
                let n = out.token(Tag::Identifier, name)?;
                let eq = out.token(Tag::Operator, ":=")?;
                let semi = out.token(Tag::Operator, ";")?;
                out.element(
                    Tag::Row,
                    &[n, eq, root(&map, init.0)?, semi, root(&map, body.0)?],
                )?
            }
            K::Sum {
                index,
                lower,
                upper,
                body,
            }
            | K::Integral {
                index,
                lower,
                upper,
                body,
            } => {
                let integral = matches!(k, K::Integral { .. });
                let op = out.token(Tag::Operator, if integral { "∫" } else { "∑" })?;
                let mut lower = root(&map, lower.0)?;
                if !integral {
                    let index = out.token(Tag::Identifier, index)?;
                    let eq = out.token(Tag::Operator, "=")?;
                    lower = out.element(Tag::Row, &[index, eq, lower])?;
                }
                let op = out.element(
                    if display == Display::Block {
                        Tag::UnderOver
                    } else {
                        Tag::SubSup
                    },
                    &[op, lower, root(&map, upper.0)?],
                )?;
                let body = out.grouped(
                    root(&map, body.0)?,
                    &value.nodes[body.0 as usize].kind,
                    30,
                    false,
                )?;
                if integral {
                    let d = out.token(Tag::Identifier, "d")?;
                    let index = out.token(Tag::Identifier, index)?;
                    out.element(Tag::Row, &[op, body, d, index])?
                } else {
                    out.element(Tag::Row, &[op, body])?
                }
            }
            K::Call {
                function,
                arguments,
            } => {
                let f = out.grouped(
                    root(&map, function.0)?,
                    &value.nodes[function.0 as usize].kind,
                    60,
                    false,
                )?;
                let mut args = Vec::new();
                for (j, a) in arguments.iter().enumerate() {
                    if j > 0 {
                        let comma = out.token(Tag::Operator, ",")?;
                        push(&mut args, comma, out.b)?;
                    }
                    push(&mut args, root(&map, a.0)?, out.b)?;
                }
                let args = out.element(Tag::Row, &args)?;
                let args = out.fence("(", ")", args)?;
                out.element(Tag::Row, &[f, args])?
            }
            K::DocGuest { syntax } => {
                let render = renderer
                    .as_mut()
                    .ok_or(Error::AnnotationRequiresPreparation(i as u64))?;
                let guest = value
                    .embeds
                    .get(syntax.0 as usize)
                    .ok_or(Error::Reference(syntax.0))?;
                let result = render(guest, out.b);
                out.b.poll()?;
                let request = result.map_err(AnnotationFailure::Guest)?;
                if request.slot != HtmlSlot::Phrasing {
                    return Err(Error::AnnotationSlot(i as u64).into());
                }
                html::validate(&request.fragment, request.slot, &request.policy, out.b).map_err(
                    |e| match e {
                        html::HtmlError::Stopped(s) => mathml::Error::Stopped(s),
                        e => mathml::Error::Html(e),
                    },
                )?;
                for class in request.policy.classes {
                    let mut present = false;
                    for prior in &classes {
                        out.b.charge(
                            Resource::Work,
                            (class.len() as u64)
                                .saturating_add(String::len(prior) as u64)
                                .saturating_add(1),
                        )?;
                        if prior == &class {
                            present = true;
                            break;
                        }
                    }
                    if !present {
                        push(&mut classes, class, out.b)?;
                    }
                }
                let html = out.add(Node::Html {
                    fragment: request.fragment,
                })?;
                out.element(Tag::Text, &[html])?
            }
            K::Label { value, annotation } => {
                let base = out.grouped(
                    root(&map, value.0)?,
                    &input.value().nodes[value.0 as usize].kind,
                    60,
                    false,
                )?;
                out.element(Tag::Under, &[base, root(&map, annotation.0)?])?
            }
        };
        map[i] = id;
    }
    let MathRoot::Expr(expr) = value.root else {
        return Err(Error::Reference(u64::MAX).into());
    };
    let root = out.element(Tag::Math, &[root(&map, expr.0)?])?;
    if let Node::Element { attributes, .. } = &mut out.nodes[root as usize] {
        push(attributes, Attribute::Display(display), out.b)?;
    }
    let fragment = Fragment {
        html_policy: nepl3_markup::html::HtmlPolicy { classes },
        nodes: out.nodes,
        root,
    };
    mathml::validate(&fragment, out.b)?;
    Ok(Rendered {
        fragment,
        node_roots: map,
    })
}
