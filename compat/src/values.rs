//! Literal values: the values rnix gives string and number literals, against Nix's evaluation.
//! Nix's rendering of a parse doesn't tell them apart well enough: `1`, `1.0` and `1.0000001` are
//! all `1` in it.

use rnix::{
    NodeOrToken,
    SyntaxKind::{NODE_STRING, TOKEN_FLOAT, TOKEN_INTEGER},
    SyntaxNode, SyntaxToken,
    ast::{AstToken, Float, Integer, InterpolPart, Str},
};
use rowan::ast::AstNode;

use crate::{Nix, render::canonical_float};

/// Stands in for interpolations, so that each string literal can be evaluated on its own. What
/// an interpolation contains doesn't change how a string's indentation is stripped.
const HOLE: char = '\u{F8FF}';

/// A literal whose value differs. The values are JSON: a string, or a number's type and value.
pub struct Difference {
    pub literal: String,
    pub number: bool,
    pub nix: String,
    pub rnix: String,
}

/// A literal: its source, to evaluate, and the value rnix gives it.
struct Literal {
    text: String,
    source: String,
    rnix: Value,
}

enum Value {
    String(String),
    /// A Nix literal for the number, or rnix's error reading it.
    Number(Result<String, String>),
}

impl Value {
    /// A Nix expression for the value, to compare with Nix's.
    fn expr(&self) -> String {
        match self {
            Value::String(value) => nix_string(value),
            Value::Number(Ok(literal)) => literal.clone(),
            Value::Number(Err(_)) => "null".into(),
        }
    }
}

/// Compares the value rnix gives every string and number literal in `root`, which it parsed
/// without errors, with Nix's, and returns the first one that differs, or Nix's error if it
/// can't evaluate them.
pub fn compare(nix: &Nix, root: &SyntaxNode) -> Result<Option<Difference>, String> {
    let literals = literals(root);
    if literals.is_empty() {
        return Ok(None);
    }
    // `1 == 1.0` in Nix, so the types are compared too.
    let first_difference = format!(
        "let
          nix = [ {} ];
          rnix = [ {} ];
          differ = i:
            let a = builtins.elemAt nix i; b = builtins.elemAt rnix i;
            in builtins.typeOf a != builtins.typeOf b || a != b;
          differences = builtins.filter differ (builtins.genList (i: i) (builtins.length nix));
        in
        if differences == [ ] then null else builtins.head differences",
        literals.iter().map(|l| format!("({})", l.source)).collect::<Vec<_>>().join(" "),
        literals.iter().map(|l| format!("({})", l.rnix.expr())).collect::<Vec<_>>().join(" "),
    );
    let literal = match nix.eval_json(&first_difference)?.as_str() {
        "null" => return Ok(None),
        json => &literals[json.parse::<usize>().unwrap()],
    };
    // Nix writes both sides' values as JSON.
    let shown = |expr: &str| nix.eval_json(expr).unwrap_or_else(|error| error);
    let typed = |expr: &str| format!("[ (builtins.typeOf ({expr})) ({expr}) ]");
    Ok(Some(match &literal.rnix {
        Value::String(_) => Difference {
            literal: literal.text.clone(),
            number: false,
            nix: shown(&literal.source),
            rnix: shown(&literal.rnix.expr()),
        },
        Value::Number(value) => Difference {
            literal: literal.text.clone(),
            number: true,
            nix: shown(&typed(&literal.source)),
            rnix: match value {
                Ok(expr) => shown(&typed(expr)),
                Err(error) => error.clone(),
            },
        },
    }))
}

/// The string and number literals in `root`.
fn literals(root: &SyntaxNode) -> Vec<Literal> {
    let mut literals = Vec::new();
    for element in root.descendants_with_tokens() {
        match element {
            NodeOrToken::Node(node) if node.kind() == NODE_STRING => literals.push(Literal {
                text: node.text().to_string(),
                source: with_holes(&node),
                rnix: Value::String(string_value(&node)),
            }),
            NodeOrToken::Token(token) if matches!(token.kind(), TOKEN_INTEGER | TOKEN_FLOAT) => {
                literals.push(Literal {
                    text: token.text().to_string(),
                    source: token.text().to_string(),
                    rnix: Value::Number(number_value(&token)),
                })
            }
            _ => {}
        }
    }
    literals
}

/// A Nix literal for the value rnix's [`Integer::value`] or [`Float::value`] gives `token`.
fn number_value(token: &SyntaxToken) -> Result<String, String> {
    let error = |error: &dyn std::error::Error| format!("error: {error}");
    match token.kind() {
        TOKEN_INTEGER => Integer::cast(token.clone())
            .unwrap()
            .value()
            .map(|i| i.to_string())
            .map_err(|e| error(&e)),
        _ => Float::cast(token.clone()).unwrap().value().map_err(|e| error(&e)).map(|f| {
            // Floats Nix can't represent never get this far: Nix rejects the source.
            canonical_float(f).unwrap_or_else(|| f.to_string())
        }),
    }
}

/// The source of `literal`, with a [`HOLE`] for each interpolation.
fn with_holes(literal: &SyntaxNode) -> String {
    let mut source = String::new();
    for child in literal.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) => source.push_str(token.text()),
            NodeOrToken::Node(_) => source.push_str(&format!("${{\"{HOLE}\"}}")),
        }
    }
    source
}

/// The value rnix gives the string `literal`, with a [`HOLE`] for each interpolation.
fn string_value(literal: &SyntaxNode) -> String {
    let mut value = String::new();
    for part in Str::cast(literal.clone()).unwrap().normalized_parts() {
        match part {
            InterpolPart::Literal(text) => value.push_str(&text),
            InterpolPart::Interpolation(_) => value.push(HOLE),
        }
    }
    value
}

/// A Nix string literal for `s`.
fn nix_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '$' => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
