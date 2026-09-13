use nepl3_core::budget::{Budget, Resource, StopReason};

/// Validate a generated SVG viewport: exactly four space-separated finite
/// decimal values in the path coordinate profile. Width and height must be
/// strictly positive and unsigned. Commas, exponents and non-ASCII whitespace
/// are outside this generated profile. This does not prove path containment or
/// aspect-ratio/fidelity. O(n) time, O(1) space, no allocation; 4n+1 Work.
pub fn view_box(input: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    let work = (input.len() as u64)
        .checked_mul(4)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
    budget.charge(Resource::Work, work)?;
    let mut values = input.split(' ');
    for index in 0..4 {
        let Some(value) = values.next() else {
            return Ok(false);
        };
        let mut cursor = 0;
        if !number(value.as_bytes(), &mut cursor, index >= 2) || cursor != value.len() {
            return Ok(false);
        }
        if index >= 2 && !value.bytes().any(|b| matches!(b, b'1'..=b'9')) {
            return Ok(false);
        }
    }
    Ok(values.next().is_none())
}

/// Validate the finite SVG path-data profile used by generated Math.
///
/// Requires initial M/m and complete parameter groups for M/L/H/V/C/S/Q/T/A/Z
/// (absolute or relative); implicit repeated groups retain their SVG semantics.
/// Arc radii are unsigned and flags are single 0/1 bytes (SVG 1.1 grammar).
/// Coordinates are
/// signed decimal numbers, at most ten integer/sixteen fractional digits with
/// magnitude <= 1,000,000,000. Exponents are outside this profile. Commas may
/// separate parameters, not precede a command's first parameter or trail a group.
///
/// O(n) time, O(1) space, no allocation; charge 4n+1 logical Work up front.
/// Does not validate the SVG element, viewport, transforms, computed geometry,
/// browser rendering cost or visual fidelity; no raw HTML admission is implied.
pub fn path_data(input: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    let work = (input.len() as u64)
        .checked_mul(4)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| budget.stop(StopReason::WorkLimit))?;
    budget.charge(Resource::Work, work)?;
    let bytes = input.as_bytes();
    let mut cursor = 0;
    let mut command = b' ';
    let mut arity = 0;
    let mut count = 0_usize;
    let mut started = false;
    loop {
        whitespace(bytes, &mut cursor);
        let Some(&next) = bytes.get(cursor) else {
            return Ok(started && (arity == 0 || count > 0 && count.is_multiple_of(arity)));
        };
        if next.is_ascii_alphabetic() {
            if arity != 0 && (count == 0 || !count.is_multiple_of(arity)) {
                return Ok(false);
            }
            command = next.to_ascii_uppercase();
            if !started && command != b'M' {
                return Ok(false);
            }
            arity = match command {
                b'M' | b'L' | b'T' => 2,
                b'H' | b'V' => 1,
                b'C' => 6,
                b'S' | b'Q' => 4,
                b'A' => 7,
                b'Z' => 0,
                _ => return Ok(false),
            };
            started = true;
            count = 0;
            cursor += 1;
            continue;
        }
        if !started || arity == 0 {
            return Ok(false);
        }
        if next == b',' {
            if count == 0 {
                return Ok(false);
            }
            cursor += 1;
            whitespace(bytes, &mut cursor);
        }
        let parameter = count % arity;
        if command == b'A' && matches!(parameter, 3 | 4) {
            if !matches!(bytes.get(cursor), Some(b'0' | b'1')) {
                return Ok(false);
            }
            cursor += 1;
        } else if !number(bytes, &mut cursor, command == b'A' && parameter < 2) {
            return Ok(false);
        }
        count += 1;
    }
}

fn whitespace(bytes: &[u8], cursor: &mut usize) {
    while matches!(bytes.get(*cursor), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        *cursor += 1;
    }
}

fn number(bytes: &[u8], cursor: &mut usize, nonnegative: bool) -> bool {
    if matches!(bytes.get(*cursor), Some(b'+' | b'-')) {
        if nonnegative {
            return false;
        }
        *cursor += 1;
    }
    let start = *cursor;
    let mut whole = 0_u64;
    while let Some(digit) = bytes.get(*cursor).filter(|b| b.is_ascii_digit()) {
        if *cursor - start == 10 {
            return false;
        }
        whole = whole * 10 + u64::from(*digit - b'0');
        *cursor += 1;
    }
    let integers = *cursor - start;
    let mut fractions = 0;
    let mut nonzero_fraction = false;
    if bytes.get(*cursor) == Some(&b'.') {
        *cursor += 1;
        while let Some(digit) = bytes.get(*cursor).filter(|b| b.is_ascii_digit()) {
            fractions += 1;
            if fractions > 16 {
                return false;
            }
            nonzero_fraction |= *digit != b'0';
            *cursor += 1;
        }
    }
    integers + fractions > 0
        && (whole < 1_000_000_000 || whole == 1_000_000_000 && !nonzero_fraction)
}
