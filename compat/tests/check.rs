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
fn paths_are_the_same_everywhere() {
    let nix = Nix::new().unwrap();
    assert_eq!(
        nix.parse("[ ./a ~/b <c> ]").unwrap(),
        "[ (/a) (/homeless-shelter/b) ((__findFile __nixPath \"c\")) ]"
    );
}
