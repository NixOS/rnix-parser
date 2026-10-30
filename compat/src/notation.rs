//! A notation for writing any input on one line: `⟨LF⟩`, `⟨CR⟩`, `⟨TAB⟩`, `⟨NUL⟩` and `⟨U+XXXX⟩`
//! stand for characters that would break the line or be invisible.

/// Writes `s` on one line.
pub fn escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\n' => out.push_str("⟨LF⟩"),
            '\r' => out.push_str("⟨CR⟩"),
            '\t' => out.push_str("⟨TAB⟩"),
            '\0' => out.push_str("⟨NUL⟩"),
            c if c.is_control() || (c.is_whitespace() && c != ' ') => {
                out.push_str(&format!("⟨U+{:04X}⟩", u32::from(c)))
            }
            c => out.push(c),
        }
    }
    out
}

/// Reads what [`escape`] writes.
pub fn unescape(s: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('⟨') {
        out.push_str(&rest[..start]);
        let after = &rest[start + '⟨'.len_utf8()..];
        let end = after.find('⟩').ok_or_else(|| format!("unclosed ⟨ in {s:?}"))?;
        let name = &after[..end];
        let c = match name {
            "LF" => '\n',
            "CR" => '\r',
            "TAB" => '\t',
            "NUL" => '\0',
            _ => name
                .strip_prefix("U+")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .and_then(char::from_u32)
                .ok_or_else(|| format!("unknown ⟨{name}⟩ in {s:?}"))?,
        };
        out.push(c);
        rest = &after[end + '⟩'.len_utf8()..];
    }
    out.push_str(rest);
    Ok(out)
}
