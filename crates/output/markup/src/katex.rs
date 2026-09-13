//! Lexical primitives for the generated-Math KaTeX profile. These do not admit
//! arbitrary HTML or CSS, prove fidelity, or authorize inline style in a CSP.
use nepl3_core::budget::{Budget, Resource, StopReason};

/// Check computed declarations emitted by the current Math-to-TeX surface.
///
/// Accepts exact lowercase property names, no whitespace, comments, escapes,
/// duplicate declarations, custom properties, functions or URLs. Lengths are
/// decimal `em`, at most four fraction digits and magnitude <= 1,000,000;
/// unitless zero is allowed. Width also admits `100%`; position only `relative`.
/// Negative values are limited to offsets, margins and vertical-align. The
/// original declaration order and bytes are left untouched.
///
/// This is not all of KaTeX's CSS vocabulary: color, shadows and arbitrary TeX
/// extensions require their own reviewed profile. A false result must never be
/// handled by stripping declarations and calling the output faithful.
///
/// O(n) time, O(1) space, no allocation. Charges 4n+1 Work before scanning, so
/// even an empty/invalid input preserves a stopped parent's reason.
pub fn computed_style(input: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    let work = (input.len() as u64)
        .checked_mul(4)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
    budget.charge(Resource::Work, work)?;
    let mut seen = 0_u32;
    for declaration in input.split_terminator(';') {
        let Some((property, value)) = declaration.split_once(':') else {
            return Ok(false);
        };
        let Some((id, negative)) = property_id(property) else {
            return Ok(false);
        };
        if seen & (1 << id) != 0 {
            return Ok(false);
        }
        seen |= 1 << id;
        let accepted = match property {
            "position" => value == "relative",
            "width" if value == "100%" => true,
            _ => length(value, negative),
        };
        if !accepted {
            return Ok(false);
        }
    }
    Ok(true)
}

fn property_id(property: &str) -> Option<(u32, bool)> {
    Some(match property {
        "height" => (0, false),
        "width" => (1, false),
        "min-width" => (2, false),
        "padding-left" => (3, false),
        "border-bottom-width" => (4, false),
        "border-top-width" => (5, false),
        "border-right-width" => (6, false),
        "top" => (7, true),
        "bottom" => (8, true),
        "left" => (9, true),
        "margin-left" => (10, true),
        "margin-right" => (11, true),
        "margin-top" => (12, true),
        "vertical-align" => (13, true),
        "position" => (14, false),
        _ => return None,
    })
}

fn length(value: &str, negative: bool) -> bool {
    if value == "0" {
        return true;
    }
    let Some(number) = value.strip_suffix("em") else {
        return false;
    };
    let number = if let Some(number) = number.strip_prefix('-') {
        if !negative {
            return false;
        }
        number
    } else {
        number
    };
    let (integer, fraction) = number.split_once('.').unwrap_or((number, ""));
    if integer.is_empty() && fraction.is_empty()
        || integer.len() > 7
        || fraction.len() > 4
        || number.ends_with('.')
        || !integer
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let whole = integer
        .bytes()
        .fold(0_u32, |n, digit| n * 10 + u32::from(digit - b'0'));
    whole < 1_000_000 || whole == 1_000_000 && fraction.bytes().all(|b| b == b'0')
}
