//! These need a `nix-instantiate`; see `README.md`.

use rnix_compat::{Checker, Nix};

fn kind(source: &str) -> &'static str {
    Checker::new(Nix::new().unwrap()).check(source).kind()
}

#[test]
fn outcomes() {
    assert_eq!(kind("1 + 2 * 3"), "same");
    assert_eq!(kind("a |> f"), "same");
    assert_eq!(kind("./a//b"), "both-reject");
    // Nix reads these as `(0).a` and `1.5 e`.
    assert_eq!(kind("0.a"), "tree-differs");
    assert_eq!(kind("1.5e"), "rnix-rejects");
    assert_eq!(kind("(a ]"), "rnix-accepts");
}

#[test]
fn string_values() {
    let checker = Checker::new(Nix::new().unwrap()).with_strings();
    assert_eq!(checker.check("''\n  a ${b}\n    c\n''").kind(), "same");
    // Nix reads a carriage return in a string as a newline.
    assert_eq!(
        checker.check("\"a\r\nb\"").describe("\"a⟨CR⟩⟨LF⟩b\""),
        "string-differs  \"a⟨CR⟩⟨LF⟩b\"\n    literal: \"a⟨CR⟩⟨LF⟩b\"\n    nix: \"a\\nb\"\n    rnix: \"a\\r\\nb\"\n"
    );
}

#[test]
fn paths_are_the_same_everywhere() {
    let nix = Nix::new().unwrap();
    assert_eq!(
        nix.parse("[ ./a ~/b <c> ]").unwrap(),
        "[ (/a) (/homeless-shelter/b) ((__findFile __nixPath \"c\")) ]"
    );
}
