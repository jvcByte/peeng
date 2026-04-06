/// Redact the password from a database URL for safe logging.
/// Finds the last `@` before the host to correctly handle usernames containing `@`.
pub fn redact_url_password(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        // Find the last @ before the host section (handles user@host:pass@host edge cases)
        if let Some(at_pos) = after_scheme.rfind('@') {
            let start = scheme_end + 3;
            let end = start + at_pos;
            let mut out = String::with_capacity(url.len());
            out.push_str(&url[..start]);
            out.push_str("[REDACTED]");
            out.push_str(&url[end..]);
            return out;
        }
    }
    url.to_string()
}
