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
    // Nix stops reading at the NUL, which rnix rejects on purpose (see cases.md).
    assert_eq!(kind("a\0b"), "rnix-rejects");
    assert_eq!(kind("(a ]"), "rnix-accepts");
}

#[test]
fn string_values() {
    let checker = Checker::new(Nix::new().unwrap()).with_values();
    assert_eq!(checker.check("''\n  a ${b}\n    c\n''").kind(), "same");
    // Nix reads a carriage return in a string as a line feed.
    assert_eq!(checker.check("\"a\r\nb\rc\"").kind(), "same");
}

#[test]
fn number_values() {
    let checker = Checker::new(Nix::new().unwrap()).with_values();
    let numbers = "[ 0 007 9223372036854775807 1. 1.0 .5 0.5 1.0000001 1.5e3 1.E-2 1.e5 ]";
    assert_eq!(checker.check(numbers).kind(), "same");
}

#[test]
fn paths_are_the_same_everywhere() {
    let nix = Nix::new().unwrap();
    assert_eq!(
        nix.parse("[ ./a ~/b <c> ]").unwrap(),
        "[ (/a) (/homeless-shelter/b) ((__findFile __nixPath \"c\")) ]"
    );
}

#[test]
fn error_positions_are_in_the_input() {
    let nix = Nix::new().unwrap();
    assert_eq!(
        nix.parse("{ a = 1;\n  a = 2; }").unwrap_err(),
        "error: attribute 'a' already defined at «stdin»:1:3"
    );
}
