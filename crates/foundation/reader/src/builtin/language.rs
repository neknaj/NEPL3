//! RFC 5646 section 2.1 ABNF only: registry validity is a separate operation.
use nepl3_core::budget::{Budget, Resource, StopReason};
const GRANDFATHERED: &[&str] = &[
    "en-GB-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "sgn-BE-FR",
    "sgn-BE-NL",
    "sgn-CH-DE",
    "art-lojban",
    "cel-gaulish",
    "no-bok",
    "no-nyn",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];
fn alpha(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic())
}
fn alnum(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}
pub(super) fn well_formed(input: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    budget.charge(Resource::Work, input.len() as u64 * 28)?;
    if GRANDFATHERED
        .iter()
        .any(|tag| input.eq_ignore_ascii_case(tag))
    {
        return Ok(true);
    }
    let mut parts = input.split('-').peekable();
    let Some(language) = parts.next() else {
        return Ok(false);
    };
    if language.eq_ignore_ascii_case("x") {
        let mut count = 0;
        for part in parts {
            if !alnum(part, 1, 8) {
                return Ok(false);
            }
            count += 1;
        }
        return Ok(count > 0);
    }
    if !alpha(language, 2, 8) {
        return Ok(false);
    }
    if language.len() <= 3 {
        for _ in 0..3 {
            if parts.peek().is_some_and(|s| alpha(s, 3, 3)) {
                parts.next();
            } else {
                break;
            }
        }
    }
    if parts.peek().is_some_and(|s| alpha(s, 4, 4)) {
        parts.next();
    }
    if parts
        .peek()
        .is_some_and(|s| alpha(s, 2, 2) || (s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit())))
    {
        parts.next();
    }
    while parts
        .peek()
        .is_some_and(|s| alnum(s, 5, 8) || (alnum(s, 4, 4) && s.as_bytes()[0].is_ascii_digit()))
    {
        parts.next();
    }
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("x") {
            let mut count = 0;
            for part in parts {
                if !alnum(part, 1, 8) {
                    return Ok(false);
                }
                count += 1;
            }
            return Ok(count > 0);
        }
        if !alnum(part, 1, 1) {
            return Ok(false);
        }
        let mut count = 0;
        while parts.peek().is_some_and(|s| alnum(s, 2, 8)) {
            parts.next();
            count += 1;
        }
        if count == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}
