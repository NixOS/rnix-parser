//! String values: rnix's [`Str::normalized_parts`] against Nix's evaluation.

use rnix::{
    NodeOrToken,
    SyntaxKind::NODE_STRING,
    SyntaxNode,
    ast::{InterpolPart, Str},
};
use rowan::ast::AstNode;

use crate::Nix;

/// Stands in for interpolations, so that each literal can be evaluated on its own. What an
/// interpolation contains doesn't change how a string's indentation is stripped.
const HOLE: char = '\u{F8FF}';

/// A string literal whose value differs. The values are JSON strings.
pub struct Difference {
    pub literal: String,
    pub nix: String,
    pub rnix: String,
}

/// Compares the value of every string literal in `root`, which rnix parsed without errors, with
/// Nix's value for it, and returns the first one that differs, or Nix's error if it can't
/// evaluate them.
pub fn compare(nix: &Nix, root: &SyntaxNode) -> Result<Option<Difference>, String> {
    let literals: Vec<SyntaxNode> =
        root.descendants().filter(|node| node.kind() == NODE_STRING).collect();
    if literals.is_empty() {
        return Ok(None);
    }
    let sources: Vec<String> = literals.iter().map(with_holes).collect();
    let values: Vec<String> = literals.iter().map(rnix_value).collect();
    let first_difference = format!(
        "let
          nix = [ {} ];
          rnix = [ {} ];
          differ = i: builtins.elemAt nix i != builtins.elemAt rnix i;
          differences = builtins.filter differ (builtins.genList (i: i) (builtins.length nix));
        in
        if differences == [ ] then null else builtins.head differences",
        sources.iter().map(|s| format!("({s})")).collect::<Vec<_>>().join(" "),
        values.iter().map(|v| nix_string(v)).collect::<Vec<_>>().join(" "),
    );
    let i: usize = match nix.eval_json(&first_difference)?.as_str() {
        "null" => return Ok(None),
        json => json.parse().unwrap(),
    };
    Ok(Some(Difference {
        literal: literals[i].text().to_string(),
        nix: nix.eval_json(&sources[i]).unwrap_or_else(|error| error),
        rnix: json_string(&values[i]),
    }))
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

/// The value rnix gives `literal`, with a [`HOLE`] for each interpolation.
fn rnix_value(literal: &SyntaxNode) -> String {
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

/// A JSON string for `s`, escaped like Nix's `builtins.toJSON`.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
