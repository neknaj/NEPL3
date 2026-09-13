//! Canonical prefix output for the standard Sentence surface. Presentation and
//! arena sharing are not source spelling; no guest is evaluated or discarded.
use crate::{
    check,
    model::{EmbedRef, InlineRef, Kind, Root, SentenceValue},
};
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Shape(check::Error),
    /// The selected standard surface has no form for this foreign closure.
    AdapterRequired(EmbedRef),
}
impl From<StopReason> for Error {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<check::Error> for Error {
    fn from(e: check::Error) -> Self {
        match e {
            check::Error::Stopped(s) => Self::Stopped(s),
            e => Self::Shape(e),
        }
    }
}
enum Part<'a> {
    Node(u64),
    List(&'a [InlineRef]),
    Word(&'static str),
    Text(&'a str),
}
fn push<'a>(s: &mut Vec<Part<'a>>, p: Part<'a>, b: &mut Budget) -> Result<(), Error> {
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Part<'a>>() as u64,
    )?;
    s.push(p);
    Ok(())
}
fn emit(out: &mut String, text: &str, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, text.len() as u64)?;
    b.charge(Resource::OutputBytes, text.len() as u64)?;
    b.charge(Resource::AllocationUnits, text.len() as u64)?;
    out.push_str(text);
    Ok(())
}
fn quoted(out: &mut String, text: &str, b: &mut Budget) -> Result<(), Error> {
    emit(out, "\"", b)?;
    for c in text.chars() {
        match c {
            '"' => emit(out, "\\\"", b)?,
            '\\' => emit(out, "\\\\", b)?,
            '\n' => emit(out, "\\n", b)?,
            '\r' => emit(out, "\\r", b)?,
            '\t' => emit(out, "\\t", b)?,
            c if c.is_control() => {
                emit(out, "\\u{", b)?;
                // A scalar needs at most six hex digits. No formatting allocation.
                let mut digits = [0u8; 6];
                let mut value = c as u32;
                let mut start = digits.len();
                loop {
                    start -= 1;
                    digits[start] = b"0123456789abcdef"[(value & 15) as usize];
                    value >>= 4;
                    if value == 0 {
                        break;
                    }
                }
                for digit in &digits[start..] {
                    let mut bytes = [0; 4];
                    emit(out, char::from(*digit).encode_utf8(&mut bytes), b)?;
                }
                emit(out, "}", b)?;
            }
            c => {
                let mut bytes = [0; 4];
                emit(out, c.encode_utf8(&mut bytes), b)?;
            }
        }
    }
    emit(out, "\"", b)
}
/// Emit all standard constructors without flattening Concat, changing Break to
/// text, or dropping Ruby/Anno boundaries. Inline roots require an Inline entry
/// context. Shared children expand per occurrence under the same output budget.
/// Foreign-inline output requires a separately selected surface adapter; errors
/// and stops return no partial string. This is not an HTML safety proof.
pub fn prefix(value: &SentenceValue, b: &mut Budget) -> Result<String, Error> {
    value.validate_shape(b)?;
    let mut out = String::new();
    let mut stack = Vec::new();
    let root = match value.root {
        Root::Sentence(r) => r.0,
        Root::Inline(r) => r.0,
    };
    push(&mut stack, Part::Node(root), b)?;
    while let Some(part) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        match part {
            Part::Word(word) => emit(&mut out, word, b)?,
            Part::Text(text) => quoted(&mut out, text, b)?,
            Part::List(items) => {
                if let Some((first, rest)) = items.split_first() {
                    emit(&mut out, "cons ", b)?;
                    push(&mut stack, Part::List(rest), b)?;
                    push(&mut stack, Part::Word(" "), b)?;
                    push(&mut stack, Part::Node(first.0), b)?;
                } else {
                    emit(&mut out, "nil", b)?;
                }
            }
            Part::Node(id) => {
                let node = value
                    .nodes
                    .get(id as usize)
                    .ok_or(check::Error::Reference(id))?;
                match node {
                    Kind::Sentence { inlines } | Kind::Concat { inlines } => {
                        emit(
                            &mut out,
                            if matches!(node, Kind::Sentence { .. }) {
                                "sentence "
                            } else {
                                "concat "
                            },
                            b,
                        )?;
                        push(&mut stack, Part::List(inlines), b)?;
                    }
                    Kind::Text { text } | Kind::Code { text } => {
                        emit(
                            &mut out,
                            if matches!(node, Kind::Text { .. }) {
                                "text "
                            } else {
                                "code "
                            },
                            b,
                        )?;
                        push(&mut stack, Part::Text(text), b)?;
                    }
                    Kind::Ruby { base, reading } => {
                        emit(&mut out, "ruby ", b)?;
                        push(&mut stack, Part::Node(reading.0), b)?;
                        push(&mut stack, Part::Word(" "), b)?;
                        push(&mut stack, Part::Node(base.0), b)?;
                    }
                    Kind::InlineAnno { base, notes } => {
                        emit(&mut out, "anno ", b)?;
                        push(&mut stack, Part::List(notes), b)?;
                        push(&mut stack, Part::Word(" "), b)?;
                        push(&mut stack, Part::Node(base.0), b)?;
                    }
                    Kind::Emphasis { inline } | Kind::Strong { inline } => {
                        emit(
                            &mut out,
                            if matches!(node, Kind::Emphasis { .. }) {
                                "em "
                            } else {
                                "strong "
                            },
                            b,
                        )?;
                        push(&mut stack, Part::Node(inline.0), b)?;
                    }
                    Kind::Break => emit(&mut out, "break", b)?,
                    Kind::ExternalLink { uri, label } => {
                        emit(&mut out, "link ", b)?;
                        push(&mut stack, Part::Node(label.0), b)?;
                        push(&mut stack, Part::Word(" "), b)?;
                        push(&mut stack, Part::Text(uri), b)?;
                    }
                    Kind::ForeignInline { syntax } => return Err(Error::AdapterRequired(*syntax)),
                }
            }
        }
    }
    b.poll()?;
    Ok(out)
}
