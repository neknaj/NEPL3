//! Host-only packaging of the fixed production stylesheet.
use super::err;
use nepl3_core::{
    budget::{Budget, Resource},
    source::Digest,
};
use std::{borrow::Cow, str::FromStr};

/// Stylesheet placement for the local document export; pages retain external CSS.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CssMode {
    #[default]
    External,
    Inline,
}

impl CssMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::External => "external",
            Self::Inline => "inline",
        }
    }
}

impl FromStr for CssMode {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "external" => Ok(Self::External),
            "inline" => Ok(Self::Inline),
            _ => Err(format!(
                "invalid CSS mode {value:?}; expected external or inline"
            )),
        }
    }
}

pub(super) const FONT_STYLESHEET: &str =
    "https://fonts.googleapis.com/css2?family=Klee+One:wght@400;600&display=swap";

const PREFIX: &str = "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src ";
const EXTERNAL: &str = "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'self' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; base-uri 'none'; form-action 'none'\"><title>NEPL3 Doc</title><link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?family=Klee+One:wght@400;600&amp;display=swap\"><link rel=\"stylesheet\" href=\"assets/doc.css\"></head><body>\n";
const MIDDLE: &str = "' https://fonts.googleapis.com; font-src https://fonts.gstatic.com; base-uri 'none'; form-action 'none'\"><title>NEPL3 Doc</title><link rel=\"stylesheet\" href=\"https://fonts.googleapis.com/css2?family=Klee+One:wght@400;600&amp;display=swap\"><style>";
const SUFFIX: &str = "</style></head><body>\n";

pub(super) fn head_with_css(
    mode: CssMode,
    css: &str,
    budget: &mut Budget,
) -> Result<Cow<'static, str>, String> {
    if mode == CssMode::External {
        return Ok(Cow::Borrowed(EXTERNAL));
    }
    // CSS is backend-owned, including numeric rules derived from validated SVG.
    // Reject raw-text delimiters rather than silently changing CSS/hash semantics.
    check_inline_css(css)?;
    let size = PREFIX.len() + "'sha256-".len() + 44 + MIDDLE.len() + css.len() + SUFFIX.len();
    budget
        .charge(Resource::Work, css.len() as u64)
        .map_err(err)?;
    budget
        .charge(Resource::AllocationUnits, (size + 44) as u64)
        .map_err(err)?;
    let hash = csp_digest(Digest::of(css.as_bytes()));
    Ok(Cow::Owned(format!(
        "{PREFIX}'sha256-{hash}{MIDDLE}{css}{SUFFIX}"
    )))
}

fn check_inline_css(css: &str) -> Result<(), String> {
    // '<' can open HTML raw-text end tags or comment/escape states. Current
    // production CSS needs none; fail closed if a future asset introduces one.
    if css.contains(['<', '\0', '\r']) {
        return Err("inline stylesheet contains an HTML raw-text delimiter".into());
    }
    Ok(())
}

/// RFC 4648 base64 of exactly one SHA-256 digest (32 bytes, one '=' padding).
fn csp_digest(digest: Digest) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(44);
    for chunk in digest.0.chunks(3) {
        match *chunk {
            [a, b, c] => {
                for index in [
                    a >> 2,
                    ((a & 3) << 4) | (b >> 4),
                    ((b & 15) << 2) | (c >> 6),
                    c & 63,
                ] {
                    encoded.push(TABLE[index as usize] as char);
                }
            }
            [a, b] => {
                for index in [a >> 2, ((a & 3) << 4) | (b >> 4), (b & 15) << 2] {
                    encoded.push(TABLE[index as usize] as char);
                }
                encoded.push('=');
            }
            _ => unreachable!("32-byte SHA-256 chunks finish with two bytes"),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::export::CSS;
    #[test]
    fn custom_backend_stylesheet_uses_its_own_csp_hash() -> Result<(), String> {
        let mut b = Budget::new(nepl3_core::budget::Limits {
            work: 100000,
            allocation_units: 100000,
            ..nepl3_core::budget::Limits::default()
        });
        let head = head_with_css(CssMode::Inline, "abc", &mut b)?;
        assert!(head.contains("sha256-ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="));
        assert!(head.contains("<style>abc</style>"));
        Ok(())
    }
    #[test]
    fn known_sha256_csp_hash_and_raw_text_boundaries() {
        // Published SHA-256 of 'abc', base64-encoded independently of the exporter.
        assert_eq!(
            csp_digest(Digest::of(b"abc")),
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
        for unsafe_css in [
            "</style><script>alert(1)</script>",
            "</StYlE>",
            "<!--",
            "<style>",
            "a\rb",
            "a\0b",
        ] {
            assert!(check_inline_css(unsafe_css).is_err());
        }
        assert!(check_inline_css(CSS).is_ok());
    }
    #[test]
    fn inline_header_respects_sticky_budget_failures() {
        let mut budget = super::super::budget();
        budget.cancel();
        assert_eq!(
            head_with_css(CssMode::Inline, CSS, &mut budget),
            Err("Cancelled".into())
        );
    }
}
