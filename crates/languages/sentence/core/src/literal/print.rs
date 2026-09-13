//! Print the literal-expressible content subset; prefix-only forms stay explicit.
use crate::{
    check,
    model::{InlineRef, Kind, Root, SentenceValue},
};
use alloc::{string::String, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Debug, Eq, PartialEq)]
pub enum PrintError {
    Stopped(StopReason),
    Shape(check::Error),
    ExpectedSentence,
    /// Code, emphasis, links, Break and foreign syntax require prefix/adapter output.
    NotLiteral(InlineRef),
}
impl From<StopReason> for PrintError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<check::Error> for PrintError {
    fn from(v: check::Error) -> Self {
        match v {
            check::Error::Stopped(s) => Self::Stopped(s),
            other => Self::Shape(other),
        }
    }
}
enum Part {
    Inline(InlineRef),
    Delimiter(char),
}
fn push(stack: &mut Vec<Part>, part: Part, b: &mut Budget) -> Result<(), PrintError> {
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Part>() as u64,
    )?;
    stack.push(part);
    Ok(())
}
fn emit(out: &mut String, c: char, b: &mut Budget) -> Result<(), PrintError> {
    b.charge(Resource::OutputBytes, c.len_utf8() as u64)?;
    b.charge(Resource::AllocationUnits, c.len_utf8() as u64)?;
    out.push(c);
    Ok(())
}
/// Preserves sentence content, not original spelling or arena sharing. No partial
/// output is returned on unsupported forms, malformed values or resource stops.
pub fn print(value: &SentenceValue, b: &mut Budget) -> Result<String, PrintError> {
    value.validate_shape(b)?;
    let Root::Sentence(root) = value.root else {
        return Err(PrintError::ExpectedSentence);
    };
    let Some(Kind::Sentence { inlines }) = value.nodes.get(root.0 as usize) else {
        return Err(PrintError::ExpectedSentence);
    };
    let mut out = String::new();
    let mut stack = Vec::new();
    emit(&mut out, '"', b)?;
    push(&mut stack, Part::Delimiter('"'), b)?;
    for child in inlines.iter().rev() {
        push(&mut stack, Part::Inline(*child), b)?;
    }
    while let Some(part) = stack.pop() {
        b.charge(Resource::Work, 1)?;
        let id = match part {
            Part::Inline(id) => id,
            Part::Delimiter(c) => {
                emit(&mut out, c, b)?;
                continue;
            }
        };
        let node = value
            .nodes
            .get(id.0 as usize)
            .ok_or(check::Error::Reference(id.0))?;
        match node {
            Kind::Text { text } => {
                for c in text.chars() {
                    b.charge(Resource::Work, 1)?;
                    let escaped = match c {
                        '\n' => Some('n'),
                        '\r' => Some('r'),
                        '\t' => Some('t'),
                        '\\' | '"' | '[' | ']' | '{' | '}' | '/' => Some(c),
                        _ => None,
                    };
                    if let Some(c) = escaped {
                        emit(&mut out, '\\', b)?;
                        emit(&mut out, c, b)?;
                    } else {
                        emit(&mut out, c, b)?;
                    }
                }
            }
            Kind::Concat { inlines } => {
                for child in inlines.iter().rev() {
                    push(&mut stack, Part::Inline(*child), b)?;
                }
            }
            Kind::Ruby { base, reading } => {
                emit(&mut out, '[', b)?;
                push(&mut stack, Part::Delimiter(']'), b)?;
                push(&mut stack, Part::Inline(*reading), b)?;
                push(&mut stack, Part::Delimiter('/'), b)?;
                push(&mut stack, Part::Inline(*base), b)?;
            }
            Kind::InlineAnno { base, notes } => {
                emit(&mut out, '{', b)?;
                push(&mut stack, Part::Delimiter('}'), b)?;
                for note in notes.iter().rev() {
                    push(&mut stack, Part::Inline(*note), b)?;
                    push(&mut stack, Part::Delimiter('/'), b)?;
                }
                push(&mut stack, Part::Inline(*base), b)?;
            }
            _ => return Err(PrintError::NotLiteral(id)),
        }
    }
    b.poll()?;
    Ok(out)
}
