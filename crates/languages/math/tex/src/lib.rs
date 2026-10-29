#![no_std]
//! Generation-side TeX preparation. This backend never evaluates expressions,
//! runs KaTeX, or claims that fonts, markup validation, or source admission passed.
extern crate alloc;
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_math_core::{
    check::CheckedExpression,
    model::{MathKind as K, MathRoot, MathValue},
    number,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unsupported {
    ForeignAnnotation,
    Delimiter,
    ControlCharacter,
    LiteralQuote,
}
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Number(number::DecimalPrintError),
    Unsupported { node: u64, reason: Unsupported },
    Reference(u64),
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
/// UTF-8 byte range for one emitted occurrence, not a global source-map proof.
#[derive(Debug, Eq, PartialEq)]
pub struct Occurrence {
    pub node: u64,
    pub start: u64,
    pub end: u64,
}
pub struct Rendered<'a> {
    source: &'a MathValue,
    tex: String,
    occurrences: Vec<Occurrence>,
}
impl<'a> Rendered<'a> {
    pub fn source(&self) -> &'a MathValue {
        self.source
    }
    pub fn tex(&self) -> &str {
        &self.tex
    }
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }
    /// Move prepared text and occurrence data to a host that retains the input.
    /// The returned data carries no source-admission or renderer proof. This is
    /// O(1) time and space; neither buffer is copied or re-serialized.
    pub fn into_parts(self) -> (String, Vec<Occurrence>) {
        (self.tex, self.occurrences)
    }
}
fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    if v.len() == v.capacity() {
        let next = v
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(4))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(
            Resource::AllocationUnits,
            ((next - v.capacity()) as u64).saturating_mul(core::mem::size_of::<T>() as u64),
        )?;
        b.charge(Resource::Work, v.len() as u64)?;
        v.try_reserve_exact(next - v.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    v.push(item);
    Ok(())
}
fn append(out: &mut String, text: &str, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, text.len() as u64)?;
    b.charge(Resource::OutputBytes, text.len() as u64)?;
    let needed = out
        .len()
        .checked_add(text.len())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    if needed > out.capacity() {
        let next = out
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(needed))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, (next - out.capacity()) as u64)?;
        b.charge(Resource::Work, out.len() as u64)?;
        out.try_reserve_exact(next - out.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    }
    out.push_str(text);
    Ok(())
}
#[derive(Clone, Copy)]
enum Part<'a> {
    Node(u64),
    Grouped(u64, u8, bool),
    Fixed(&'static str),
    Literal(u64, &'a str),
    Name(u64, &'a str),
    End(usize),
}
// A definite integral has an explicit differential closing its body. Unlike a
// sum or unstructured sequence, it can stand as an operand without outer fences.
fn binding_power(k: &K) -> u8 {
    match k {
        K::Number { value, .. } if value.numerator().is_negative() => 40,
        K::Let { .. } | K::Sequence { .. } | K::Sum { .. } => 0,
        K::Equal { .. } | K::Lt { .. } | K::Le { .. } => 10,
        K::Add { .. } | K::Sub { .. } => 20,
        K::Mul { .. } => 30,
        K::Integral { .. } | K::Neg { .. } => 40,
        K::Pow { .. } => 50,
        _ => 60,
    }
}
fn needs_group(k: &K, parent: u8, equal: bool) -> bool {
    let script = matches!(
        k,
        K::Pow { .. }
            | K::Subscript { .. }
            | K::Superscript { .. }
            | K::Scripts { .. }
            | K::Transpose { .. }
    );
    binding_power(k) < parent
        || (equal && ((parent < 60 && binding_power(k) == parent) || (parent >= 50 && script)))
}
fn delimiter(text: &str, node: u64) -> Result<&'static str, Error> {
    match text {
        "" => Ok("."),
        "(" => Ok("("),
        ")" => Ok(")"),
        "[" => Ok("["),
        "]" => Ok("]"),
        "{" => Ok("\\{"),
        "}" => Ok("\\}"),
        "|" => Ok("|"),
        "‖" => Ok("\\Vert"),
        "⟨" => Ok("\\langle"),
        "⟩" => Ok("\\rangle"),
        _ => Err(Error::Unsupported {
            node,
            reason: Unsupported::Delimiter,
        }),
    }
}
fn literal(
    out: &mut String,
    text: &str,
    node: u64,
    italic: bool,
    b: &mut Budget,
) -> Result<(), Error> {
    append(
        out,
        if italic {
            "\\mathord{\\textit{"
        } else {
            "\\text{"
        },
        b,
    )?;
    for c in text.chars() {
        b.charge(Resource::Work, 1)?;
        if c.is_control() {
            return Err(Error::Unsupported {
                node,
                reason: Unsupported::ControlCharacter,
            });
        }
        let mut bytes = [0; 4];
        let escaped = match c {
            '\\' => "\\textbackslash{}",
            '{' => "\\{",
            '}' => "\\}",
            '$' => "\\$",
            '&' => "\\&",
            '#' => "\\#",
            '_' => "\\_",
            '%' => "\\%",
            '^' => "\\textasciicircum{}",
            '~' => "\\textasciitilde{}",
            ' ' => "\\ ",
            '\'' | '`' => {
                return Err(Error::Unsupported {
                    node,
                    reason: Unsupported::LiteralQuote,
                });
            }
            '-' => "{-}",
            _ => c.encode_utf8(&mut bytes),
        };
        // Keep Unicode runs intact, including combining marks. Only the TeX
        // ligature triggers and syntax metacharacters require explicit encoding.
        append(out, escaped, b)?;
    }
    append(out, if italic { "}}" } else { "}" }, b)
}

/// Convert exact checked structure with precedence-sensitive operand grouping.
/// Time/storage are O(emitted occurrences + emitted bytes), plus decimal printing.
/// A shared DAG can expand: every occurrence and byte consumes the same caller's
/// budget. No partial result is returned. Foreign annotations fail explicitly so
/// the host can select the independent MathML path without dropping annotation.
pub fn render<'a>(input: &CheckedExpression<'a>, b: &mut Budget) -> Result<Rendered<'a>, Error> {
    use Part::{Fixed as F, Grouped as G, Literal as L, Node as N};
    b.poll()?;
    let source = input.value();
    let MathRoot::Expr(root) = source.root else {
        return Err(Error::Reference(u64::MAX));
    };
    let mut out = Rendered {
        source,
        tex: String::new(),
        occurrences: Vec::new(),
    };
    let mut pending = Vec::new();
    push(&mut pending, (N(root.0), 1_u64), b)?;
    while let Some((part, depth)) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        match part {
            F(text) => append(&mut out.tex, text, b)?,
            L(node, text) => literal(&mut out.tex, text, node, false, b)?,
            Part::Name(node, text) => literal(&mut out.tex, text, node, true, b)?,
            Part::End(i) => out.occurrences[i].end = out.tex.len() as u64,
            G(id, parent, equal) => {
                let kind = &source
                    .nodes
                    .get(usize::try_from(id).map_err(|_| Error::Reference(id))?)
                    .ok_or(Error::Reference(id))?
                    .kind;
                if needs_group(kind, parent, equal) {
                    push(&mut pending, (F("\\right)"), depth), b)?;
                    push(&mut pending, (N(id), depth), b)?;
                    push(&mut pending, (F("\\left("), depth), b)?;
                } else {
                    push(&mut pending, (N(id), depth), b)?;
                }
            }
            N(id) => {
                b.observe_depth(depth)?;
                b.charge(Resource::Nodes, 1)?;
                let k = &source
                    .nodes
                    .get(usize::try_from(id).map_err(|_| Error::Reference(id))?)
                    .ok_or(Error::Reference(id))?
                    .kind;
                let index = out.occurrences.len();
                push(
                    &mut out.occurrences,
                    Occurrence {
                        node: id,
                        start: out.tex.len() as u64,
                        end: 0,
                    },
                    b,
                )?;
                push(&mut pending, (Part::End(index), depth), b)?;
                let child_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
                let mut emit = |parts: &[Part<'a>]| -> Result<(), Error> {
                    for p in parts.iter().rev() {
                        push(&mut pending, (*p, child_depth), b)?;
                    }
                    Ok(())
                };
                match k {
                    K::Number { value, .. } => {
                        let text = number::print_decimal(value, b).map_err(|e| match e {
                            number::DecimalPrintError::Stopped(s) => Error::Stopped(s),
                            e => Error::Number(e),
                        })?;
                        append(&mut out.tex, &text, b)?;
                    }
                    K::Symbol { name } => emit(&[Part::Name(id, name)])?,
                    K::Text { text } => emit(&[L(id, text)])?,
                    K::Add { left, right }
                    | K::Sub { left, right }
                    | K::Mul { left, right }
                    | K::Equal { left, right }
                    | K::Lt { left, right }
                    | K::Le { left, right } => {
                        let op = match k {
                            K::Add { .. } => "+",
                            K::Sub { .. } => "-",
                            K::Mul { .. } => "\\cdot ",
                            K::Equal { .. } => "=",
                            K::Lt { .. } => "<",
                            _ => "\\le ",
                        };
                        let precedence = binding_power(k);
                        let comparison = precedence == 10;
                        emit(&[
                            G(left.0, precedence, comparison),
                            F(op),
                            G(
                                right.0,
                                precedence,
                                comparison || matches!(k, K::Sub { .. }),
                            ),
                        ])?;
                    }
                    K::Frac { left, right } => {
                        emit(&[F("\\frac{"), N(left.0), F("}{"), N(right.0), F("}")])?
                    }
                    K::Pow { left, right }
                    | K::Superscript { left, right }
                    | K::Subscript { left, right } => {
                        let op = if matches!(k, K::Subscript { .. }) {
                            "_{"
                        } else {
                            "^{"
                        };
                        emit(&[
                            F("{"),
                            G(
                                left.0,
                                if matches!(k, K::Pow { .. }) { 50 } else { 60 },
                                true,
                            ),
                            F("}"),
                            F(op),
                            N(right.0),
                            F("}"),
                        ])?;
                    }
                    K::Neg { value } => emit(&[
                        F("-"),
                        G(
                            value.0,
                            40,
                            !matches!(source.nodes[value.0 as usize].kind, K::Integral { .. }),
                        ),
                    ])?,
                    K::Sqrt { value } => emit(&[F("\\sqrt{"), N(value.0), F("}")])?,
                    K::Root { degree, radicand } => {
                        emit(&[F("\\sqrt[{"), N(degree.0), F("}]{"), N(radicand.0), F("}")])?
                    }
                    K::Transpose { value } => {
                        emit(&[F("{"), G(value.0, 60, true), F("}^{\\mathsf{T}}")])?
                    }
                    K::Det { value } => emit(&[F("\\det\\left("), N(value.0), F("\\right)")])?,
                    K::Scripts { base, sub, sup } => emit(&[
                        F("{"),
                        G(base.0, 60, true),
                        F("}_{"),
                        N(sub.0),
                        F("}^{"),
                        N(sup.0),
                        F("}"),
                    ])?,
                    K::Fence { open, close, value } => emit(&[
                        F("\\left"),
                        F(delimiter(open, id)?),
                        F(" "),
                        N(value.0),
                        F("\\right"),
                        F(delimiter(close, id)?),
                        F(" "),
                    ])?,
                    K::Let { name, init, body } => emit(&[
                        Part::Name(id, name),
                        F(":=\\left("),
                        N(init.0),
                        F("\\right);\\left("),
                        N(body.0),
                        F("\\right)"),
                    ])?,
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
                        if matches!(k, K::Sum { .. }) {
                            emit(&[
                                F("\\sum_{"),
                                Part::Name(id, index),
                                F("="),
                                N(lower.0),
                                F("}^{"),
                                N(upper.0),
                                F("} "),
                                G(body.0, 30, false),
                            ])?;
                        } else {
                            emit(&[
                                F("\\int_{"),
                                N(lower.0),
                                F("}^{"),
                                N(upper.0),
                                F("} "),
                                G(body.0, 20, false),
                                F("\\,\\mathrm{d}"),
                                Part::Name(id, index),
                            ])?;
                        }
                    }
                    K::Sequence { values } | K::Vector { values } | K::Row { values } => {
                        let (open, close, sep) = match k {
                            K::Vector { .. } => ("\\begin{pmatrix}", "\\end{pmatrix}", "\\\\"),
                            K::Row { .. } => ("", "", "&"),
                            _ => ("", "", ""),
                        };
                        emit(&[F(close)])?;
                        for (i, value) in values.iter().enumerate().rev() {
                            if i + 1 < values.len() {
                                emit(&[F(sep)])?;
                            }
                            emit(&[F("{"), N(value.0), F("}")])?;
                        }
                        emit(&[F(open)])?;
                    }
                    K::Matrix { rows } => {
                        emit(&[F("\\end{pmatrix}")])?;
                        for (i, row) in rows.iter().enumerate().rev() {
                            if i + 1 < rows.len() {
                                emit(&[F("\\\\")])?;
                            }
                            emit(&[N(row.0)])?;
                        }
                        emit(&[F("\\begin{pmatrix}")])?;
                    }
                    K::Call {
                        function,
                        arguments,
                    } => {
                        emit(&[F("\\right)")])?;
                        for (i, arg) in arguments.iter().enumerate().rev() {
                            if i + 1 < arguments.len() {
                                emit(&[F(",\\,")])?;
                            }
                            emit(&[F("{"), N(arg.0), F("}")])?;
                        }
                        emit(&[G(function.0, 60, false), F("\\left(")])?;
                    }
                    K::Label { .. } | K::SentenceGuest { .. } => {
                        return Err(Error::Unsupported {
                            node: id,
                            reason: Unsupported::ForeignAnnotation,
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}
