//! XML 1.0 character admission and canonical text/attribute escaping.
//! No input character is silently replaced or removed.
use alloc::string::String;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextContext {
    Content,
    /// A double-quoted attribute in a fixed element/attribute name.
    Attribute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextError {
    InvalidCharacter { byte: u64, scalar: u64 },
    Stopped(StopReason),
}
impl From<StopReason> for TextError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// XML 1.0 Fifth Edition Char, evaluated on an already valid Rust scalar.
pub fn is_xml_character(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn replacement(c: char, context: TextContext) -> Option<&'static str> {
    match (c, context) {
        ('&', _) => Some("&amp;"),
        ('<', _) => Some("&lt;"),
        ('>', _) => Some("&gt;"),
        // A literal CR is normalized by XML/HTML parsing even in text content.
        ('\r', _) => Some("&#xD;"),
        ('"', TextContext::Attribute) => Some("&quot;"),
        ('\n', TextContext::Attribute) => Some("&#xA;"),
        ('\t', TextContext::Attribute) => Some("&#x9;"),
        _ => None,
    }
}

/// Return the complete escaped text, or a typed error without a partial string.
/// Charges validation work and exact output bytes before allocating the result.
/// Input is a semantic value: source admission is its caller's separate duty.
pub fn escape(text: &str, context: TextContext, budget: &mut Budget) -> Result<String, TextError> {
    budget.charge(Resource::Work, text.len() as u64)?;
    let mut length = 0_u64;
    for (byte, c) in text.char_indices() {
        if !is_xml_character(c) {
            return Err(TextError::InvalidCharacter {
                byte: byte as u64,
                scalar: u64::from(c),
            });
        }
        let size = replacement(c, context).map_or(c.len_utf8(), str::len) as u64;
        length = length
            .checked_add(size)
            .ok_or_else(|| budget.stop(StopReason::OutputLimit))?;
    }
    budget.charge(Resource::Work, length)?;
    // This stage produces bytes rather than a markup node. Nodes and logical
    // tree depth are accounted for by the tree serializer that calls it.
    budget.charge(Resource::OutputBytes, length)?;
    budget.charge(Resource::AllocationUnits, length)?;
    let capacity = usize::try_from(length).map_err(|_| budget.stop(StopReason::AllocationLimit))?;
    if capacity > isize::MAX as usize {
        return Err(budget.stop(StopReason::AllocationLimit).into());
    }
    let mut output = String::with_capacity(capacity);
    for c in text.chars() {
        if let Some(escaped) = replacement(c, context) {
            output.push_str(escaped);
        } else {
            output.push(c);
        }
    }
    Ok(output)
}
