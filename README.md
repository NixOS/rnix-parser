# rnix-parser [![Crates.io](https://img.shields.io/crates/v/rnix.svg)](http://crates.io/crates/rnix) [![Chat on Matrix](https://matrix.to/img/matrix-badge.svg)](https://matrix.to/#/#rnix-lsp:matrix.org)

rnix is a parser for the [Nix language](https://nixos.org/nix/) written in Rust.

This can be used to manipulate the Nix AST and can for example be used for:

- Interactively render Nix on a GUI
- Formatting Nix code
- Rename identifiers

and a lot more!

rnix nowadays uses [@matklad](https://github.com/matklad)'s
[rowan](https://crates.io/crates/rowan) crate to ensure:

- all span information is preserved, meaning you can use the AST to for
  example apply highlighting
- printing out the AST prints out 100% the original code. This is not an
  over-exaggeration, even completely invalid nix code such as this README can
  be intact after a parsing session (though the AST will mark errnous nodes)
- easy ways to walk the tree without resorting to recursion

## Demo

Examples can be found in the `examples/` directory. 

You may also want to see
[nix-explorer](https://gitlab.com/jD91mZM2/nix-explorer): An example
that highlights AST nodes in Nix code. This demonstrates how
whitespaces and commands are preserved.

## Hacking

Tests can be run with `cargo test`.

In order to update all `.expect`-files to the currently expected results,
you may run `UPDATE_TESTS=1 cargo test`.

You can parse Nix expressions from standard input using the `from-stdin` example.
To try that, run the following in your shell:

```sh
echo "[hello nix]" | cargo run --quiet --example from-stdin
```

rnix aims to parse like Nix, following its parser release by release. [compat](compat/README.md)
compares the two.

## Release Checklist

- [ ] In [CHANGELOG.md](CHANGELOG.md), add a `## [vX.Y.Z] - YYYY-MM-DD` heading below
      `## [Unreleased]`, add a `[vX.Y.Z]` compare link at the bottom, and point the `[Unreleased]`
      link at `vX.Y.Z...master`.
- [ ] Bump `version` in [Cargo.toml](Cargo.toml).
- [ ] Refresh the lockfiles: `cargo update -w`, and the same with `--manifest-path fuzz/Cargo.toml`
      and `--manifest-path compat/Cargo.toml`.
- [ ] Run `cargo test`, `nix build` and `cargo publish --dry-run`.
- [ ] Commit as `chore: release vX.Y.Z` on a branch, open a PR, and add it to the merge queue.
- [ ] Once it merges, tag the release commit `vX.Y.Z` and push the tag.
- [ ] Approve the `release` workflow run the tag started. It checks that the tag is on `master`
      and matches `Cargo.toml`, publishes to crates.io, and creates the GitHub release from the
      changelog section.
- [ ] Check the new version on crates.io and the GitHub release.

# RIP jd91mzm2

Sadly, the original author of this project, [@jD91mZM2 has passed
away](https://www.redox-os.org/news/open-source-mental-health/). His online
presence was anonymous and what we have left is his code. This is but one of
his many repos that he contributed to.
