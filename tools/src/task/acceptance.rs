use crate::Result;
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::collections::BTreeSet;

/// Read plain definition prefixes at the start of Markdown text lines.
/// The legacy prose and annotated list projections have the same visible IDs.
pub(super) fn ids(text: &str) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    let mut stack = Vec::new();
    let mut excluded = 0;
    let mut prefix = String::with_capacity(4);
    let mut line_start = false;
    let mut inline_html = false;
    let options = Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;
    for event in Parser::new_ext(text, options) {
        match event {
            Event::Start(tag) => {
                let allowed = matches!(tag, Tag::Paragraph | Tag::List(_) | Tag::Item);
                stack.push(!allowed);
                excluded += usize::from(!allowed);
                if matches!(tag, Tag::Paragraph | Tag::Item) {
                    inline_html = false;
                }
                prefix.clear();
                line_start = excluded == 0 && matches!(tag, Tag::Paragraph | Tag::Item);
            }
            Event::End(_) => {
                if stack.pop() == Some(true) {
                    excluded -= 1;
                }
                prefix.clear();
                line_start = false;
            }
            Event::SoftBreak | Event::HardBreak => {
                prefix.clear();
                line_start = excluded == 0 && !inline_html;
            }
            Event::InlineHtml(_) => {
                // Do not treat text on a later line of an inline HTML fragment
                // (including hidden spans) as a plain Markdown definition.
                // The next independent paragraph/item establishes a new scope.
                inline_html = true;
                prefix.clear();
                line_start = false;
            }
            Event::Text(value) if excluded == 0 && line_start => {
                // Escapes and entities can split the ID/colon across Text events.
                // Never concatenate code, HTML, a link or formatting into an ID.
                for ch in value.chars() {
                    prefix.push(ch);
                    if prefix.len() >= 4 {
                        let bytes = prefix.as_bytes();
                        if bytes.len() == 4
                            && bytes[0].is_ascii_uppercase()
                            && bytes[1..3].iter().all(u8::is_ascii_digit)
                            && bytes[3] == b':'
                        {
                            let id = prefix[..3].to_owned();
                            if !ids.insert(id.clone()) {
                                return Err(format!(
                                    "acceptance specification: duplicate identifier {id}"
                                )
                                .into());
                            }
                        }
                        line_start = false;
                        break;
                    }
                }
            }
            _ => {
                prefix.clear();
                line_start = false;
            }
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_lists_escapes_and_entities_have_the_same_visible_prefixes() -> Result<()> {
        let expected = BTreeSet::from(["A01".into(), "W02".into()]);
        for text in [
            "A01: first\nW02: second\n",
            "- A01\\: first\n- W02\\: second\n",
            "7. A01: first\n8. W02: second\n",
            "A&#48;1&#58; first  \nW02: second\n",
        ] {
            assert_eq!(ids(text)?, expected, "{text}");
        }
        Ok(())
    }

    #[test]
    fn examples_quotes_tables_and_inline_markup_are_not_definitions() -> Result<()> {
        let text = "# A01: heading\n\n```text\nA02: fenced\n```\n\n    A03: indented\n\n> A04: quote\n\n<div>\nA05: HTML\n</div>\n\nA06: cell | other\n--- | ---\nA07: cell | other\n\n[^note]: A08: footnote\n\n`A09:` code\n\n**A10:** emphasis\n\n[A11:](target.md) link\n\n<span>A12:</span> inline HTML\n\nsee A13: mention\n\nA1: short\n\nAA14: long\n\nＡ15: non-ASCII\n\nA16： fullwidth colon\n\nA17: actual definition\n";
        assert_eq!(ids(text)?, BTreeSet::from(["A17".into()]));
        Ok(())
    }

    #[test]
    fn duplicate_definitions_across_presentations_are_rejected() {
        assert!(ids("A01: original\n\n- A01\\: duplicate\n").is_err());
        assert!(ids("- A01: original\n\n<!-- -->\n\n- A&#48;1: duplicate\n").is_err());
    }

    #[test]
    fn inline_html_cannot_reenable_definitions_at_a_line_break() -> Result<()> {
        for fragment in [
            "intro <span hidden>\nA01: hidden definition\n</span>\n",
            "<span hidden>intro\nA01: hidden definition</span>\n",
        ] {
            assert!(ids(fragment)?.is_empty());
            assert_eq!(
                ids(&format!("{fragment}\nW02: independent paragraph\n"))?,
                BTreeSet::from(["W02".into()])
            );
        }
        let text = "- G01\\: <ruby>文法<rt>ぶんぽう</rt></ruby>\n  A01: continued HTML paragraph\n- G02\\: next item\n";
        assert_eq!(ids(text)?, BTreeSet::from(["G01".into(), "G02".into()]));
        Ok(())
    }
}
