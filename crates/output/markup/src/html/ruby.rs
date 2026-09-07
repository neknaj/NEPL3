//! HTML Ruby base/annotation groups, including text-only parenthetic fallback.
use super::*;
use nepl3_core::budget::{Budget, Resource};
pub(super) fn sequence(
    f: &HtmlFragment,
    root: u64,
    children: &[u64],
    b: &mut Budget,
) -> Result<(), HtmlError> {
    // 0 needs base; 1 ordinary base; 2 one nested ruby base;
    // 3 plain rt sequence; 4 first rt after rp; 5 needs closing rp;
    // 6 rp-completed annotation sequence (may continue with rt).
    let mut state = 0_u8;
    for r in children {
        b.charge(Resource::Work, 1)?;
        let n = check::node(f, *r)?;
        if let HtmlNode::Text { text } = n {
            b.charge(Resource::Work, text.len() as u64)?;
            if text
                .bytes()
                .all(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0c))
            {
                continue;
            }
        }
        let tag = if let HtmlNode::Element { tag, .. } = n {
            Some(*tag)
        } else {
            None
        };
        state = match (state, tag) {
            (1 | 2, Some(HtmlTag::Rt)) | (3, Some(HtmlTag::Rt)) => 3,
            (4 | 6, Some(HtmlTag::Rt)) => 5,
            (1 | 2, Some(HtmlTag::Rp)) => 4,
            (5, Some(HtmlTag::Rp)) => 6,
            (_, Some(HtmlTag::Rt | HtmlTag::Rp)) => return Err(HtmlError::Content(root)),
            (0 | 3 | 6, Some(HtmlTag::Ruby)) => 2,
            (0 | 1 | 3 | 6, tag) if tag != Some(HtmlTag::Ruby) => 1,
            _ => return Err(HtmlError::Content(root)),
        };
    }
    if matches!(state, 3 | 6) {
        Ok(())
    } else {
        Err(HtmlError::Content(root))
    }
}
