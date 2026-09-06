//! Primitive recognition; composite execution is handled by the explicit VM stack.
use crate::{
    model::Expectation,
    plan::{CharClass, ReaderExpr},
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceError, SourceSnapshot},
    value::NdfValue,
};
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Primitive {
    Matched {
        value: NdfValue,
        end: u64,
    },
    NoMatch {
        expected: Vec<Expectation>,
        furthest: u64,
    },
    NeedMore {
        expected: Vec<Expectation>,
    },
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveError {
    Source(SourceError),
    Stopped(StopReason),
}
impl From<SourceError> for PrimitiveError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
impl From<StopReason> for PrimitiveError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
pub(crate) fn recognize(
    expr: &ReaderExpr,
    source: &SourceSnapshot,
    start: u64,
    limit: u64,
    final_input: bool,
    budget: &mut Budget,
) -> Result<Option<Primitive>, PrimitiveError> {
    let input = source.slice_range(start, limit)?;
    let result = match expr {
        ReaderExpr::Literal(expected) => {
            let mut actual = input.chars();
            let mut end = start;
            let mut result = None;
            for expected_char in expected.chars() {
                budget.charge(Resource::Work, 1)?;
                match actual.next() {
                    Some(c) if c == expected_char => end += c.len_utf8() as u64,
                    Some(_) => {
                        result = Some(missing(
                            literal_expectation(expected, budget)?,
                            end,
                            false,
                            budget,
                        )?);
                        break;
                    }
                    None => {
                        result = Some(missing(
                            literal_expectation(expected, budget)?,
                            end,
                            !final_input,
                            budget,
                        )?);
                        break;
                    }
                }
            }
            result.unwrap_or(Primitive::Matched {
                value: NdfValue::Unit,
                end,
            })
        }
        ReaderExpr::Scalar(class) => {
            budget.charge(Resource::Work, 1)?;
            match class {
                CharClass::Chars(text) | CharClass::Except(text) => {
                    budget.charge(Resource::Work, text.len() as u64)?
                }
                _ => {}
            }
            match input.chars().next() {
                Some(c) if class.contains(c) => {
                    let end = start + c.len_utf8() as u64;
                    matched_text(source, start, end, budget)?
                }
                Some(_) => missing(class_expectation(class, budget)?, start, false, budget)?,
                None => missing(
                    class_expectation(class, budget)?,
                    start,
                    !final_input,
                    budget,
                )?,
            }
        }
        ReaderExpr::Eof => {
            budget.charge(Resource::Work, 1)?;
            if start == limit && final_input {
                Primitive::Matched {
                    value: NdfValue::Unit,
                    end: start,
                }
            } else {
                missing(
                    Expectation::EndOfInput,
                    start,
                    start == limit && !final_input,
                    budget,
                )?
            }
        }
        ReaderExpr::TakeCount(count) => {
            let mut end = start;
            let mut scalars = input.chars();
            let mut complete = true;
            for _ in 0..*count {
                budget.charge(Resource::Work, 1)?;
                if let Some(c) = scalars.next() {
                    end += c.len_utf8() as u64;
                } else {
                    complete = false;
                    break;
                }
            }
            if complete {
                matched_text(source, start, end, budget)?
            } else {
                missing(
                    Expectation::ScalarClass(CharClass::Any),
                    end,
                    !final_input,
                    budget,
                )?
            }
        }
        ReaderExpr::Until(delimiter) => {
            budget.charge(Resource::Work, input.len() as u64)?;
            if let Some(index) = input.find(delimiter) {
                matched_text(source, start, start + index as u64, budget)?
            } else {
                missing(
                    literal_expectation(delimiter, budget)?,
                    limit,
                    !final_input,
                    budget,
                )?
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(result))
}
fn matched_text(
    source: &SourceSnapshot,
    start: u64,
    end: u64,
    budget: &mut Budget,
) -> Result<Primitive, PrimitiveError> {
    let text = source.slice_range(start, end)?;
    budget.charge(Resource::AllocationUnits, text.len() as u64)?;
    Ok(Primitive::Matched {
        value: NdfValue::Text(text.into()),
        end,
    })
}
fn missing(
    expectation: Expectation,
    furthest: u64,
    need_more: bool,
    budget: &mut Budget,
) -> Result<Primitive, PrimitiveError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Expectation>() as u64,
    )?;
    let expected = alloc::vec![expectation];
    Ok(if need_more {
        Primitive::NeedMore { expected }
    } else {
        Primitive::NoMatch { expected, furthest }
    })
}

fn literal_expectation(text: &str, budget: &mut Budget) -> Result<Expectation, PrimitiveError> {
    budget.charge(Resource::AllocationUnits, text.len() as u64)?;
    Ok(Expectation::Literal(text.into()))
}
fn class_expectation(
    class: &CharClass,
    budget: &mut Budget,
) -> Result<Expectation, PrimitiveError> {
    let bytes = match class {
        CharClass::Chars(s) | CharClass::Except(s) => s.len() as u64,
        _ => 0,
    };
    budget.charge(Resource::AllocationUnits, bytes)?;
    Ok(Expectation::ScalarClass(class.clone()))
}
