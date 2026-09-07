// Deliberately constrained output URI profile, not a browser URL parser.
pub(super) fn id(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
pub(super) fn path(s: &str) -> bool {
    !s.is_empty()
        && s.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        })
}
fn suffix(s: &str) -> bool {
    let mut chars = s.bytes();
    while let Some(c) = chars.next() {
        if c == b'%' {
            if !chars.next().is_some_and(|c| c.is_ascii_hexdigit())
                || !chars.next().is_some_and(|c| c.is_ascii_hexdigit())
            {
                return false;
            }
        } else if !(c.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=".contains(&c)) {
            return false;
        }
    }
    true
}
pub(super) fn external(s: &str) -> bool {
    if let Some(mail) = s.strip_prefix("mailto:") {
        let address = mail.split('?').next().unwrap_or("");
        return address.contains('@')
            && !address.starts_with('@')
            && !address.ends_with('@')
            && !address.contains(['/', '#'])
            && suffix(mail);
    }
    let Some(rest) = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    // DNS/punycode or IPv4 hostname, optional decimal port. No credentials,
    // percent encoding, IPv6 literals or browser whitespace normalization.
    let (host, port) = authority
        .split_once(':')
        .map_or((authority, None), |(h, p)| (h, Some(p)));
    let good_host = !host.is_empty()
        && host.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        });
    good_host
        && port.is_none_or(|p| {
            !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()) && p.parse::<u16>().is_ok()
        })
        && suffix(rest)
}
