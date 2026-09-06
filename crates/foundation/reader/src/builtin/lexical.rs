use super::BuiltinReader;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Scan {
    Matched(usize),
    NoMatch,
    NeedMore,
    Failed(&'static str, usize),
}
pub(super) fn scan(
    kind: BuiltinReader,
    input: &str,
    final_input: bool,
    start: u64,
    budget: &mut Budget,
) -> Result<Scan, StopReason> {
    let Some(first) = input.chars().next() else {
        return Ok(if final_input {
            Scan::NoMatch
        } else {
            Scan::NeedMore
        });
    };
    match kind {
        BuiltinReader::Name => {
            if first != '_' && !unicode_ident::is_xid_start(first) {
                return Ok(Scan::NoMatch);
            }
            let mut end = first.len_utf8();
            for ch in input[end..].chars() {
                budget.charge(Resource::Work, 1)?;
                if !unicode_ident::is_xid_continue(ch) {
                    break;
                }
                end += ch.len_utf8();
            }
            Ok(complete(end, input.len(), final_input))
        }
        BuiltinReader::Nat | BuiltinReader::Number => number(kind, input, final_input, budget),
        BuiltinReader::Lang => {
            if !first.is_ascii_alphabetic() {
                return Ok(Scan::NoMatch);
            }
            let mut end = 0;
            for ch in input.chars() {
                budget.charge(Resource::Work, 1)?;
                if !ch.is_ascii_alphanumeric() && ch != '-' {
                    if unicode_ident::is_xid_continue(ch) {
                        return Ok(Scan::Failed("BoundaryMismatch", end));
                    }
                    break;
                }
                end += ch.len_utf8();
            }
            if end == input.len() && !final_input {
                return Ok(Scan::NeedMore);
            }
            Ok(if super::language::well_formed(&input[..end], budget)? {
                Scan::Matched(end)
            } else {
                Scan::Failed("InvalidLanguageTag", end)
            })
        }
        BuiltinReader::Trivia => {
            if first == '\u{feff}' {
                return Ok(if start == 0 {
                    Scan::Matched(first.len_utf8())
                } else {
                    Scan::NoMatch
                });
            }
            let comment = first == '#';
            if !comment && !matches!(first, ' ' | '\t' | '\r' | '\n') {
                return Ok(Scan::NoMatch);
            }
            let mut end = 0;
            for ch in input.chars() {
                budget.charge(Resource::Work, 1)?;
                if if comment {
                    matches!(ch, '\r' | '\n')
                } else {
                    !matches!(ch, ' ' | '\t' | '\r' | '\n')
                } {
                    break;
                }
                end += ch.len_utf8();
            }
            Ok(complete(end, input.len(), final_input))
        }
        BuiltinReader::Text => Ok(Scan::NoMatch), // Text's committed decoder is a separate path.
    }
}
fn complete(end: usize, limit: usize, final_input: bool) -> Scan {
    if end == limit && !final_input {
        Scan::NeedMore
    } else {
        Scan::Matched(end)
    }
}
fn number(
    kind: BuiltinReader,
    input: &str,
    final_input: bool,
    budget: &mut Budget,
) -> Result<Scan, StopReason> {
    let bytes = input.as_bytes();
    let mut end = usize::from(kind == BuiltinReader::Number && bytes.first() == Some(&b'-'));
    if end == bytes.len() {
        return Ok(if final_input {
            Scan::Failed("InvalidNumber", end)
        } else {
            Scan::NeedMore
        });
    }
    if !bytes[end].is_ascii_digit() {
        return Ok(Scan::NoMatch);
    }
    let leading_zero = bytes[end] == b'0';
    end += 1;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        budget.charge(Resource::Work, 1)?;
        if leading_zero {
            return Ok(Scan::Failed("InvalidNumber", end));
        }
        end += 1;
    }
    if kind == BuiltinReader::Number && bytes.get(end) == Some(&b'.') {
        end += 1;
        if end == bytes.len() {
            return Ok(if final_input {
                Scan::Failed("InvalidNumber", end)
            } else {
                Scan::NeedMore
            });
        }
        if !bytes[end].is_ascii_digit() {
            return Ok(Scan::Failed("InvalidNumber", end));
        }
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            budget.charge(Resource::Work, 1)?;
            end += 1;
        }
    }
    if input[end..]
        .chars()
        .next()
        .is_some_and(unicode_ident::is_xid_continue)
    {
        return Ok(Scan::Failed("BoundaryMismatch", end));
    }
    Ok(complete(end, input.len(), final_input))
}
