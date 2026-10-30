# rnix-compat

Compares rnix's parser with Nix's. rnix aims to parse like CppNix, the reference implementation
of the Nix language, following its parser release by release: the release is `nixReference` in
[`flake.nix`](../flake.nix). Where other implementations, such as Lix, differ from CppNix, rnix
follows CppNix, and the differences are documented on the
[wiki](https://github.com/NixOS/rnix-parser/wiki).

The goals, in order:

1. rnix parses input that Nix accepts to the same tree as Nix.
2. rnix reports errors for input that Nix rejects.

## How inputs are compared

1. rnix and `nix-instantiate --parse` both parse the input. If either rejects it, that is the
   outcome.
2. If both accept it, rnix's tree is written back as Nix with every expression in parentheses
   ([`src/render.rs`](src/render.rs)), and Nix parses that rendering too. If Nix parses the source
   and the rendering to the same expression, rnix grouped the tokens like Nix does. Nix is the only
   printer involved, so the comparison needs no copy of Nix's AST. The rendering separates tokens
   with spaces, except inside strings and paths, and writes numbers and URIs in canonical forms,
   so that a token rnix split or merged shows up as a different tree too. Paths are written as
   they are, though: if rnix read a path where Nix reads something else, Nix would read the
   rendering like the source, and the outcome would be `same`.
3. With `--values` (and in the tests), Nix also evaluates every string literal, with
   interpolations replaced by a placeholder, and every number literal, and the value must be
   the one rnix gives it: `Str::normalized_parts`, `Integer::value` and `Float::value`. Nix's
   rendering of a parse can't tell `1`, `1.0` and `1.0000001` apart, so a number's type is
   compared too.

Nix looks variables up while it parses, so inputs are parsed inside `with { };`. `nix-instantiate`
runs without a store or `nix.conf` (settings like `lint-url-literals` change what Nix accepts),
with the `pipe-operators` experimental feature, which rnix always parses, and from `/` with a
made-up `HOME`, so that paths come out the same everywhere.

| Outcome | Meaning |
| --- | --- |
| `same` | Both accept the input and parse it to the same expression |
| `both-reject` | Both reject the input |
| `rnix-rejects` | Nix accepts the input and rnix doesn't (goal 1) |
| `tree-differs` | Both accept the input, and parse it to different expressions (goal 1) |
| `string-differs` | Same expression, but a string literal has another value (goal 1) |
| `number-differs` | Same expression, but a number literal has another type or value (goal 1) |
| `render-rejected` | Nix rejects rnix's rendering of input it accepts: a bug in `render.rs` |
| `values-rejected` | Nix can't evaluate the literals of input it accepts: a bug in `values.rs` |
| `rnix-accepts` | rnix accepts the input and Nix doesn't (goal 2) |
| `unreadable` | A file that can't be read |

rnix only parses UTF-8, so a file that isn't is `rnix-rejects` if Nix accepts it.

## Running

```sh
nix develop .#compat
cd compat

cargo test                    # cases.md, rnix's test_data, Nix's language tests, generated inputs
UPDATE_EXPECT=1 cargo test    # record changed outcomes in the .expect files
HEGEL_TEST_CASES=10000 cargo test --test generated    # more generated inputs, a few minutes

cargo run --release -- -e '0.a'                            # compare an expression
cargo run --release -- --values "$RNIX_COMPAT_NIXPKGS"     # all of nixpkgs, a few minutes
```

Update expectations from `compat/`: Cargo only reads `compat/.cargo/config.toml`, which tells
expect-test where to write the expectations in `tests/render.rs`, when it runs there.

`rnix-compat` lists the inputs on which the parsers disagree, counts the outcomes, and exits with
status 1 if any input is `rnix-rejects`, `tree-differs`, `string-differs`, `number-differs`,
`render-rejected`, `values-rejected` or `unreadable`. As of rnix 0.15.1 and Nix 2.35.3, every file
in nixpkgs is `same`. To use another Nix, set `RNIX_COMPAT_NIX_INSTANTIATE`.

CI runs the tests on every pull request, and the `compat` workflow compares the latest nixpkgs
every week, the values of literals included.

- [`cases.md`](cases.md) lists the inputs on which the parsers disagree, by cause, and inputs on
  which they agree but that are easy to get wrong. [`cases.expect`](cases.expect) records their
  outcomes.
- [`test_data.expect`](test_data.expect) and [`nix-lang.expect`](nix-lang.expect) record the files
  in rnix's `test_data` and in Nix's language tests on which the parsers disagree.
- [`tests/generated.rs`](tests/generated.rs) has [Hegel](https://hegel.dev/) generate expressions
  from Nix's grammar, and strings, and fails on any that rnix parses differently from Nix, with
  the smallest such input it can find. Input that only rnix accepts passes, for now. On CI, the
  inputs are the same every run.

## Following a Nix release

1. Point `nixReference` in `flake.nix` at the release (update nixpkgs first if it doesn't have it).
2. Run `UPDATE_EXPECT=1 cargo test`, and read the diff of the `.expect` files: changed outcomes,
   new language tests, changed error messages.
3. Read what changed in Nix's lexer and parser, as the release notes don't mention every change:
   `git log OLD..NEW -- src/libexpr/lexer.l src/libexpr/parser.y
   src/libexpr/include/nix/expr/parser-state.hh` in a Nix checkout.
4. Run `rnix-compat` over nixpkgs.
5. Add new disagreements to `cases.md`.
