//! rnix's tree, written back as Nix with explicit grouping.

use rnix::{
    NodeOrToken,
    SyntaxKind::{self, *},
    SyntaxNode, SyntaxToken,
};

/// Writes a tree that rnix parsed without errors back as Nix, with every expression in
/// parentheses. If Nix parses the rendering to the same expression as the source, rnix grouped
/// the source's tokens like Nix does.
///
/// Tokens are separated by spaces (except inside strings and paths), so that a token rnix split
/// in two stays two tokens. Numbers are written in a canonical form and URIs as strings, so that
/// tokens rnix merged stay one. Comments are left out.
pub fn render(root: &SyntaxNode) -> String {
    let mut out = String::new();
    render_node(root, &mut out);
    out
}

fn is_expr(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        NODE_APPLY
            | NODE_ASSERT
            | NODE_ATTR_SET
            | NODE_BIN_OP
            | NODE_CUR_POS
            | NODE_HAS_ATTR
            | NODE_IDENT
            | NODE_IF_ELSE
            | NODE_LAMBDA
            | NODE_LEGACY_LET
            | NODE_LET_IN
            | NODE_LIST
            | NODE_LITERAL
            | NODE_PAREN
            | NODE_PATH_ABS
            | NODE_PATH_HOME
            | NODE_PATH_REL
            | NODE_PATH_SEARCH
            | NODE_SELECT
            | NODE_STRING
            | NODE_UNARY_OP
            | NODE_WITH
    )
}

/// Nodes whose tokens must stay together: a space would change a string or end a path.
fn is_glued(kind: SyntaxKind) -> bool {
    matches!(kind, NODE_STRING | NODE_PATH_ABS | NODE_PATH_HOME | NODE_PATH_REL | NODE_PATH_SEARCH)
}

/// Whether `child`, an expression node, can be put in parentheses where it is. Nix's grammar
/// doesn't allow that for names: attribute names, parameters and inherited names (which rnix
/// represents with the same nodes as expressions), and the variable `or` in `f or`, which Nix
/// only parses after a function (`(or)` is an error).
fn can_wrap(parent: SyntaxKind, child: &SyntaxNode) -> bool {
    match parent {
        NODE_ATTRPATH | NODE_INHERIT | NODE_PAT_BIND | NODE_IDENT_PARAM => false,
        // The first identifier is the parameter's name, anything after it its default.
        NODE_PAT_ENTRY => child.prev_sibling().is_some(),
        NODE_APPLY => {
            !(child.kind() == NODE_IDENT && child.text() == "or" && child.prev_sibling().is_some())
        }
        _ => true,
    }
}

fn render_node(node: &SyntaxNode, out: &mut String) {
    let glued = is_glued(node.kind());
    let mut first = true;
    for child in node.children_with_tokens() {
        if child.kind().is_trivia() {
            continue;
        }
        if !glued && !first {
            out.push(' ');
        }
        first = false;
        match child {
            NodeOrToken::Token(token) => render_token(&token, out),
            NodeOrToken::Node(child) => {
                let wrap = is_expr(child.kind()) && can_wrap(node.kind(), &child);
                if wrap {
                    out.push_str("( ");
                }
                render_node(&child, out);
                if wrap {
                    out.push_str(" )");
                }
            }
        }
    }
}

fn render_token(token: &SyntaxToken, out: &mut String) {
    let text = token.text();
    let canonical = match token.kind() {
        TOKEN_INTEGER => text.parse::<i64>().ok().map(|i| i.to_string()),
        TOKEN_FLOAT => text.parse::<f64>().ok().and_then(canonical_float),
        // Nix parses a URI to the same expression as a string.
        TOKEN_URI => Some(format!("\"{}\"", text.replace('$', "\\$"))),
        _ => None,
    };
    // Numbers Nix can't represent are left as they are; Nix rejects the source anyway.
    out.push_str(canonical.as_deref().unwrap_or(text));
}

/// A float literal Nix reads as exactly `f`: digits before the point, and an exponent.
pub(crate) fn canonical_float(f: f64) -> Option<String> {
    if f == 0.0 {
        return Some("0.0".into());
    }
    if !f.is_normal() {
        return None;
    }
    let scientific = format!("{f:e}");
    let (mantissa, exponent) = scientific.split_once('e')?;
    let point = if mantissa.contains('.') { "" } else { "." };
    Some(format!("{mantissa}{point}e{exponent}"))
}
