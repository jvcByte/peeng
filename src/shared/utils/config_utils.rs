pub fn redact_url_password(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        if let Some(at_pos) = url[scheme_end + 3..].find('@') {
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
