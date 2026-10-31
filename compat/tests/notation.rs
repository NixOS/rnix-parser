use rnix_compat::{escape, unescape};

#[test]
fn round_trips() {
    for s in ["a\nb", "\r\n\t\0", "\u{a0}\u{2028}", "⟨LF⟩"] {
        assert_eq!(unescape(&escape(s)).unwrap(), s);
    }
    // The text `⟨LF⟩` doesn't look like a line feed.
    assert_eq!(escape("⟨LF⟩\n"), "⟨U+27E8⟩LF⟩⟨LF⟩");
}
