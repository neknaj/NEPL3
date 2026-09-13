//! Render the current Markdown canonical overview with the existing parser.
//! This is not a Markdown-to-Doc migration or a new Markdown implementation.
use super::{Result, canonical};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::path::Path;

pub(super) struct Overview {
    pub html: String,
    pub source_sha256: String,
}

pub(super) fn generate(
    root: &Path,
    registry: &canonical::Registry,
    base: &str,
    commit: &str,
) -> Result<Overview> {
    let bytes = canonical::bounded(root, "README.md", 256 * 1024)?;
    crate::command(
        root,
        "git",
        &["ls-files", "--error-unmatch", "--", "README.md"],
    )?;
    let html = render(root, std::str::from_utf8(&bytes)?, registry, base, commit)?;
    Ok(Overview {
        html,
        source_sha256: super::hash(&bytes),
    })
}

fn destination(
    root: &Path,
    value: &str,
    registry: &canonical::Registry,
    base: &str,
    commit: &str,
) -> Result<String> {
    if value.starts_with("https://") {
        return Ok(value.into());
    }
    // README links use repository paths; reject ambient origins and unsupported
    // URI schemes instead of letting the browser interpret them as executable.
    if value.contains([':', '?', '#', '\\', '%']) || value.starts_with('/') {
        return Err(
            "unsupported overview link; use an explicit HTTPS URL or repository file".into(),
        );
    }
    canonical::bounded(root, value, 4 * 1024 * 1024)?;
    crate::command(root, "git", &["ls-files", "--error-unmatch", "--", value])?;
    if let Some(page) = registry.pages.iter().find(|p| p.projection == value) {
        return Ok(format!("{base}{}", page.route));
    }
    Ok(format!(
        "https://github.com/neknaj/NEPL3/blob/{commit}/{value}"
    ))
}

fn render(
    root: &Path,
    source: &str,
    registry: &canonical::Registry,
    base: &str,
    commit: &str,
) -> Result<String> {
    let mut events = Vec::new();
    for event in Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    ) {
        events.push(match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => Event::Start(Tag::Link {
                link_type,
                dest_url: destination(root, &dest_url, registry, base, commit)?.into(),
                title,
                id,
            }),
            // Badges/images remain their authored alt text; no build-time fetch
            // or third-party image load is introduced by the static overview.
            Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => continue,
            Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
            other => other,
        });
    }
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events.into_iter());
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overview_keeps_content_without_remote_images_or_active_html() -> Result<()> {
        let fixture = crate::testing::Fixture::new()?;
        crate::command(fixture.root(), "git", &["init", "-q"])?;
        fixture.write("page.md", "page")?;
        fixture.write("other.md", "other")?;
        crate::command(fixture.root(), "git", &["add", "."])?;
        let registry = serde_json::from_str(
            r#"{"version":1,"pages":[{"id":"page","source":"page.nepld","projection":"page.md","aliases":"aliases.json","route":"docs/page.html","renderer":"test"}]}"#,
        )?;
        let source = "# Overview\n\n[![CI](https://example.org/badge.svg)](https://example.org/ci)\n\n<script>alert(1)</script>\n\n[Page](page.md) [Other](other.md)\n\n| A | B |\n|---|---|\n|one|two|";
        for base in ["/NEPL3/", "/acceptance/project/"] {
            let html = render(fixture.root(), source, &registry, base, "test-commit")?;
            assert!(html.contains("<h1>Overview</h1>"));
            assert!(html.contains("&lt;script&gt;"));
            assert!(!html.contains("<script") && !html.contains("<img"));
            assert!(html.contains("<table>"));
            assert!(html.contains(&format!("href=\"{base}docs/page.html\"")));
            assert!(html.contains("https://github.com/neknaj/NEPL3/blob/test-commit/other.md"));
        }
        for bad in [
            "javascript:alert(1)",
            "//example.org/a",
            "../outside",
            "missing.md",
            "page.md#missing",
        ] {
            assert!(
                render(
                    fixture.root(),
                    &format!("[bad]({bad})"),
                    &registry,
                    "/NEPL3/",
                    "test-commit"
                )
                .is_err(),
                "{bad}"
            );
        }
        Ok(())
    }
}
