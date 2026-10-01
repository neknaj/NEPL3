//! Bounded static SVG prototype: svg/g/path only, no references or active content.
//! This is deliberately narrower than SVG; unsupported input is rejected intact.
use alloc::string::String;
use nepl3_core::budget::{Budget, Resource, StopReason};
use xmlparser::{ElementEnd, Token, Tokenizer};
pub const MAX_BYTES: usize = 1_048_576;

pub fn validate(source: &str, budget: &mut Budget) -> Result<bool, StopReason> {
    budget.charge(Resource::Work, source.len() as u64)?;
    if source.len() > MAX_BYTES || source.contains('&') || source.contains('\0') {
        return Ok(false);
    }
    let mut stack = [""; 64];
    let mut depth = 0usize;
    let mut current = "";
    let mut attrs = [""; 32];
    let mut count = 0;
    let mut ids = [""; 1024];
    let mut id_count = 0;
    let mut pending = false;
    let mut closed = false;
    let mut root = false;
    let mut namespace = false;
    let mut viewbox = false;
    for token in Tokenizer::from(source) {
        budget.charge(Resource::Work, 1)?;
        let Ok(token) = token else { return Ok(false) };
        match token {
            Token::Declaration {
                version, encoding, ..
            } => {
                if root
                    || version.as_str() != "1.0"
                    || encoding.is_some_and(|e| !e.as_str().eq_ignore_ascii_case("utf-8"))
                {
                    return Ok(false);
                }
            }
            Token::Comment { .. } => {}
            Token::ElementStart { prefix, local, .. } => {
                budget.observe_depth(depth as u64 + 1)?;
                budget.charge(Resource::Nodes, 1)?;
                if !prefix.is_empty() || depth == stack.len() {
                    return Ok(false);
                }
                if pending || closed {
                    return Ok(false);
                }
                pending = true;
                current = local.as_str();
                if depth == 0 {
                    if root || current != "svg" {
                        return Ok(false);
                    }
                    root = true;
                } else if !matches!(current, "g" | "path") || stack[depth - 1] == "path" {
                    return Ok(false);
                }
                count = 0;
            }
            Token::Attribute {
                prefix,
                local,
                value,
                ..
            } => {
                let key = local.as_str();
                let value = value.as_str();
                budget.charge(Resource::Work, (value.len() + count) as u64)?;
                if count == attrs.len() || attrs[..count].contains(&key) {
                    return Ok(false);
                }
                attrs[count] = key;
                count += 1;
                if prefix.as_str() == "xmlns"
                    && key == "xlink"
                    && current == "svg"
                    && value == "http://www.w3.org/1999/xlink"
                {
                    continue;
                }
                if !prefix.is_empty() {
                    return Ok(false);
                }
                let ok = match key {
                    "xmlns" if current == "svg" => {
                        namespace = value == "http://www.w3.org/2000/svg";
                        namespace
                    }
                    "version" if current == "svg" => matches!(value, "1.1" | "2.0"),
                    "viewBox" if current == "svg" => {
                        viewbox = crate::katex::view_box(value, budget)?;
                        viewbox
                    }
                    "width" | "height" if current == "svg" => length(value),
                    "id" => {
                        budget.charge(Resource::Work, (id_count * value.len()) as u64)?;
                        if id_count == ids.len() || ids[..id_count].contains(&value) {
                            return Ok(false);
                        }
                        ids[id_count] = value;
                        id_count += 1;
                        !value.is_empty()
                            && value
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
                    }
                    "d" if current == "path" => crate::katex::path_data(value, budget)?,
                    "fill" | "stroke" => {
                        value == "none"
                            || (matches!(value.len(), 4 | 7)
                                && value.starts_with('#')
                                && value[1..].bytes().all(|c| c.is_ascii_hexdigit()))
                    }
                    "stroke-width" | "stroke-miterlimit" => number(value).is_some_and(|n| n >= 0.0),
                    "opacity" | "fill-opacity" | "stroke-opacity" => {
                        number(value).is_some_and(|n| (0.0..=1.0).contains(&n))
                    }
                    "stroke-linejoin" => matches!(value, "miter" | "round" | "bevel"),
                    "stroke-linecap" => matches!(value, "butt" | "round" | "square"),
                    "fill-rule" => matches!(value, "nonzero" | "evenodd"),
                    _ => false,
                };
                if !ok {
                    return Ok(false);
                }
            }
            Token::ElementEnd { end, .. } => match end {
                ElementEnd::Open => {
                    pending = false;
                    stack[depth] = current;
                    depth += 1;
                }
                ElementEnd::Empty => {
                    pending = false;
                    if depth == 0 {
                        closed = true;
                    }
                }
                ElementEnd::Close(prefix, local) => {
                    if !prefix.is_empty() || depth == 0 || stack[depth - 1] != local.as_str() {
                        return Ok(false);
                    }
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                    }
                }
            },
            Token::Text { text } if text.as_str().trim().is_empty() => {}
            _ => return Ok(false),
        }
    }
    Ok(root && namespace && viewbox && depth == 0 && closed && !pending)
}
fn number(value: &str) -> Option<f64> {
    if value.len() > 64 {
        return None;
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && n.abs() <= 1e9)
}
fn length(value: &str) -> bool {
    let v = value
        .strip_suffix("pt")
        .or_else(|| value.strip_suffix("px"))
        .unwrap_or(value);
    number(v).is_some_and(|n| n > 0.0)
}
pub(crate) fn data_url(svg: &str, b: &mut Budget) -> Result<String, StopReason> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let size = svg
        .len()
        .checked_add(2)
        .and_then(|n| n.checked_div(3))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| n.checked_add(26))
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, size as u64)?;
    b.charge(Resource::Work, size as u64)?;
    let mut out = String::with_capacity(size);
    out.push_str("data:image/svg+xml;base64,");
    for chunk in svg.as_bytes().chunks(3) {
        let a = chunk[0];
        let c = chunk.get(1).copied().unwrap_or(0);
        let d = chunk.get(2).copied().unwrap_or(0);
        out.push(CHARS[(a >> 2) as usize] as char);
        out.push(CHARS[(((a & 3) << 4) | (c >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            CHARS[(((c & 15) << 2) | (d >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[(d & 63) as usize] as char
        } else {
            '='
        });
    }
    Ok(out)
}

/// Explicit root dimensions converted to CSS px. Missing dimensions or invalid
/// input yield None; this metadata helper is not an admission proof.
pub fn intrinsic_size(source: &str, budget: &mut Budget) -> Result<Option<(f64, f64)>, StopReason> {
    if !validate(source, budget)? {
        return Ok(None);
    }
    budget.charge(Resource::Work, source.len() as u64)?;
    let mut width = None;
    let mut height = None;
    for token in Tokenizer::from(source) {
        budget.charge(Resource::Work, 1)?;
        match token {
            Ok(Token::Attribute { local, value, .. }) => {
                let raw = value.as_str();
                let multiplier = if raw.ends_with("pt") { 4.0 / 3.0 } else { 1.0 };
                let value = raw
                    .strip_suffix("pt")
                    .or_else(|| raw.strip_suffix("px"))
                    .unwrap_or(raw);
                match local.as_str() {
                    "width" => width = number(value).map(|n| n * multiplier),
                    "height" => height = number(value).map(|n| n * multiplier),
                    _ => {}
                }
            }
            Ok(Token::ElementEnd { .. }) => break,
            Err(_) => return Ok(None),
            _ => {}
        }
    }
    Ok(width.zip(height))
}
