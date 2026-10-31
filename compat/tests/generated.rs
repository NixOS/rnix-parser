//! Generated inputs. These need a `nix-instantiate`; see `README.md`.
//!
//! Hegel generates input from Nix's grammar, with few parentheses, literals at the edges of what
//! Nix's lexer accepts, and any separator between tokens, including none. rnix has to parse what
//! Nix accepts like Nix does. Input that only rnix accepts passes, until rnix reports errors for
//! everything Nix rejects (the second goal in `README.md`).
//!
//! `HEGEL_TEST_CASES=10000 cargo test --test generated` runs more of them.

use std::sync::OnceLock;

use hegel::{TestCase, generators as gs};
use rnix_compat::{Checker, Nix, escape};

#[hegel::test]
fn expressions(tc: TestCase) {
    let mut generator = Generator::new(&tc);
    let tokens = generator.expr();
    check(&tc, &generator.join(&tokens));
}

#[hegel::test]
fn strings(tc: TestCase) {
    check(&tc, &Generator::new(&tc).string_literal());
}

fn check(tc: &TestCase, input: &str) {
    static CHECKER: OnceLock<Checker> = OnceLock::new();
    let checker = CHECKER.get_or_init(|| Checker::new(Nix::new().unwrap()).with_values());
    tc.note(&format!("input: {}", escape(input)));
    let outcome = checker.check(input);
    tc.event(outcome.kind());
    assert!(!outcome.is_failure(), "{}", outcome.describe(&escape(input)));
}

/// Draws the parts of an input. The first choice of each kind is the simplest, which is what
/// Hegel shrinks a failing input towards.
struct Generator<'a> {
    tc: &'a TestCase,
    /// How many more compound expressions to generate.
    budget: u32,
}

impl<'a> Generator<'a> {
    fn new(tc: &'a TestCase) -> Self {
        let budget = tc.draw_silent(gs::integers::<u32>().min_value(0).max_value(12));
        Self { tc, budget }
    }

    fn pick<T: Clone + Send + Sync>(&self, choices: &[T]) -> T {
        self.tc.draw_silent(gs::sampled_from(choices))
    }

    fn below(&self, n: usize) -> usize {
        self.tc.draw_silent(gs::integers::<usize>().min_value(0).max_value(n - 1))
    }

    fn chance(&self, p: f64) -> bool {
        self.tc.draw_silent(gs::weighted_booleans(p))
    }

    /// Whether to generate another compound expression.
    fn deeper(&mut self) -> bool {
        let deeper = self.budget > 0 && self.chance(0.8);
        self.budget = self.budget.saturating_sub(1);
        deeper
    }

    /// Joins tokens with separators, which are sometimes nothing at all.
    fn join(&self, tokens: &[String]) -> String {
        let mut out = String::new();
        for (i, token) in tokens.iter().enumerate() {
            if i > 0 {
                out.push_str(
                    self.pick(&[
                        " ", "", "\n", "\t", "\r\n", "\r", "/**/", "#c\n", "#c\r\n", "#c\r",
                    ]),
                );
            }
            out.push_str(token);
        }
        out
    }

    fn expr(&mut self) -> Vec<String> {
        if !self.deeper() {
            return self.op_chain();
        }
        let mut tokens = Vec::new();
        match self.below(10) {
            0 => {
                tokens.push(self.ident());
                tokens.push(":".into());
                tokens.extend(self.expr());
            }
            1 => {
                tokens.extend(self.formals());
                tokens.push(":".into());
                tokens.extend(self.expr());
            }
            2 => {
                tokens.push(self.pick(&["assert", "with"]).into());
                tokens.extend(self.expr());
                tokens.push(";".into());
                tokens.extend(self.expr());
            }
            3 => {
                tokens.push("let".into());
                tokens.extend(self.bindings());
                tokens.push("in".into());
                tokens.extend(self.expr());
            }
            4 => {
                tokens.push("if".into());
                tokens.extend(self.expr());
                tokens.push("then".into());
                tokens.extend(self.expr());
                tokens.push("else".into());
                tokens.extend(self.expr());
            }
            5 => {
                tokens.extend(self.op_chain());
                tokens.push(self.pick(&["|>", "<|"]).into());
                tokens.extend(self.op_chain());
            }
            _ => tokens.extend(self.op_chain()),
        }
        tokens
    }

    fn op_chain(&mut self) -> Vec<String> {
        let mut tokens = self.operand();
        for _ in 0..self.below(3) {
            let op = self.pick(&[
                "+", "-", "*", "/", "++", "//", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "->",
                "?",
            ]);
            tokens.push(op.into());
            if op == "?" {
                tokens.extend(self.attrpath());
            } else {
                tokens.extend(self.operand());
            }
        }
        tokens
    }

    fn operand(&mut self) -> Vec<String> {
        let mut tokens = Vec::new();
        while self.chance(0.15) {
            tokens.push(self.pick(&["-", "!"]).into());
        }
        tokens.extend(self.select());
        while self.chance(0.25) {
            tokens.extend(self.select());
        }
        tokens
    }

    fn select(&mut self) -> Vec<String> {
        let mut tokens = self.simple();
        match self.below(8) {
            0 | 1 => {}
            2 => tokens.push("or".into()),
            3 => {
                tokens.push(".".into());
                tokens.extend(self.attrpath());
                tokens.push("or".into());
                tokens.extend(self.select());
            }
            4 | 5 => {
                tokens.push(".".into());
                tokens.extend(self.attrpath());
            }
            _ => {}
        }
        tokens
    }

    fn attrpath(&mut self) -> Vec<String> {
        let mut tokens = self.attr();
        while self.chance(0.3) {
            tokens.push(".".into());
            tokens.extend(self.attr());
        }
        tokens
    }

    fn attr(&mut self) -> Vec<String> {
        match self.below(6) {
            0..=2 => vec![self.ident()],
            3 => vec!["or".into()],
            4 => vec![self.string()],
            _ if self.deeper() => {
                let mut tokens = vec!["${".to_string()];
                tokens.extend(self.expr());
                tokens.push("}".into());
                tokens
            }
            _ => vec![self.ident()],
        }
    }

    fn bindings(&mut self) -> Vec<String> {
        let mut tokens = Vec::new();
        for _ in 0..self.below(3) {
            if self.chance(0.25) {
                tokens.push("inherit".into());
                if self.chance(0.5) && self.deeper() {
                    tokens.push("(".into());
                    tokens.extend(self.expr());
                    tokens.push(")".into());
                }
                for _ in 0..self.below(3) {
                    tokens.extend(self.attr());
                }
            } else {
                tokens.extend(self.attrpath());
                tokens.push("=".into());
                tokens.extend(self.expr());
            }
            tokens.push(";".into());
        }
        tokens
    }

    fn formals(&mut self) -> Vec<String> {
        let mut tokens = vec!["{".to_string()];
        let count = self.below(3);
        for i in 0..count {
            if i > 0 {
                tokens.push(",".into());
            }
            tokens.push(self.ident());
            if self.chance(0.3) && self.deeper() {
                tokens.push("?".into());
                tokens.extend(self.expr());
            }
        }
        if self.chance(0.3) {
            if count > 0 {
                tokens.push(",".into());
            }
            tokens.push("...".into());
        }
        tokens.push("}".into());
        if self.chance(0.25) {
            tokens.push("@".into());
            tokens.push(self.ident());
        }
        tokens
    }

    fn simple(&mut self) -> Vec<String> {
        if !self.deeper() {
            return vec![self.leaf()];
        }
        match self.below(14) {
            0 => {
                let mut tokens = vec!["(".to_string()];
                tokens.extend(self.expr());
                tokens.push(")".into());
                tokens
            }
            1 => {
                let mut tokens = vec![self.pick(&["{", "rec {", "let {"]).to_string()];
                tokens.extend(self.bindings());
                tokens.push("}".into());
                tokens
            }
            2 => {
                let mut tokens = vec!["[".to_string()];
                for _ in 0..self.below(3) {
                    tokens.extend(self.select());
                }
                tokens.push("]".into());
                tokens
            }
            3 => {
                // An interpolation in a string or a path.
                let mut tokens = vec!["${".to_string()];
                tokens.extend(self.expr());
                tokens.push("}".into());
                let interpolation = self.join(&tokens);
                let (before, after) = self.pick(&[
                    ("\"", "\""),
                    ("\"a", "b\""),
                    ("''", "''"),
                    ("''\n  a", "\n''"),
                    ("./a/", ""),
                    ("./a/", ".nix"),
                    ("a/", "/b"),
                    ("~/", ""),
                    ("/", ""),
                ]);
                vec![format!("{before}{interpolation}{after}")]
            }
            _ => vec![self.leaf()],
        }
    }

    fn ident(&self) -> String {
        self.pick(&["a", "b", "x", "e", "a-b", "a'", "_x", "or-a", "if1", "inx", "__curPos", "E5"])
            .into()
    }

    fn string(&self) -> String {
        self.pick(&[
            "\"a\"",
            "\"\"",
            "\"$\"",
            "\"$${a}\"",
            "\"\\${a}\"",
            "\"a\\\"b\"",
            "\"\\n\"",
            "\"a\rb\"",
            "\"a\r\nb\"",
            "''a''",
            "''''",
            "''\n  a\n    b\n''",
            "''a''$b''",
            "''a'''b''",
            "''\\n''",
            "''a''\\nb''",
            "''$''",
            "''\r\n  a\r\n''",
            "'' \n  a''",
        ])
        .into()
    }

    fn leaf(&mut self) -> String {
        match self.below(6) {
            0 | 1 => self.ident(),
            2 => self.string(),
            3 => self
                .pick(&[
                    "1",
                    "0",
                    "007",
                    "00.5",
                    "0.5",
                    ".5",
                    "1.",
                    "1.5e3",
                    "1.5E+3",
                    "1.5e",
                    "1e5",
                    "0.e1",
                    "1.2.3",
                    "9223372036854775807",
                    "9223372036854775808",
                    "1.e400",
                ])
                .into(),
            4 => self
                .pick(&[
                    "./a",
                    "a/b",
                    "../a",
                    "/a",
                    "~/a",
                    "./a.nix",
                    "a/b/",
                    "./a//b",
                    "<a>",
                    "<a/b>",
                    "<a/>",
                    "a:b",
                    "http://x/y?z=1",
                    "x:x",
                    "a.b:c",
                    "-./a",
                    "+/a",
                ])
                .into(),
            _ => self.pick(&["true", "null"]).into(),
        }
    }

    /// A string literal with the escapes, line endings and indentation that are easy to get
    /// wrong.
    fn string_literal(&self) -> String {
        let indented = self.chance(0.5);
        let pieces: &[&str] = if indented {
            &[
                "a", " ", "  ", "\n", "\t", "\r\n", "\r", "b c", "''$", "'''", "''\\n", "''\\t",
                "''\\ ", "''\\\n", "''\\\r", "${x}", "$", "'", "$$", "\\n", "\\", "\"", "\u{a0}",
            ]
        } else {
            &[
                "a", " ", "\t", "\n", "\r\n", "\r", "\\n", "\\r", "\\t", "\\\\", "\\\"", "\\$",
                "\\${", "$$", "$", "${x}", "''", "\\\r\n", "\\\r", "\\a",
            ]
        };
        let text: String = (0..self.below(10)).map(|_| self.pick(pieces)).collect();
        if indented { format!("''{text}''") } else { format!("\"{text}\"") }
    }
}
