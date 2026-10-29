//! This module contains functions for creating AST fragments
//!
//! This is inspired by rust-analyzer's make.rs module.

use crate::{
    Root,
    ast::{self, AstNode, BinOpKind, UnaryOpKind},
};

/// Parse text as a Nix file and extract a specific AST node type.
///
/// This is the core helper function for all constructors in this module.
/// It parses the provided text and searches for the first occurrence of the
/// requested node type in the parse tree.
///
/// Panics if the text doesn't parse or doesn't contain a node of type `N`.
#[track_caller]
fn ast_from_text<N: AstNode>(text: &str) -> N {
    let parse = Root::parse(text);

    match parse.syntax().descendants().find_map(N::cast) {
        Some(it) => it,
        None => {
            let node = std::any::type_name::<N>();
            panic!("Failed to make ast node `{node}` from text {text}")
        }
    }
}

/// Parse text as a Nix expression and extract a specific expression node type.
///
/// This is a specialized version of `ast_from_text` for expression nodes.
/// It wraps the text to ensure it parses as a valid expression context.
///
/// Panics if the text doesn't parse to contain an expression node of type `E`.
#[track_caller]
fn expr_from_text<E: AstNode>(text: &str) -> E {
    // Most Nix code is already an expression, but wrap in parens to be safe
    let wrapped = format!("({text})");
    let parse = Root::parse(&wrapped);

    match parse.syntax().descendants().find_map(E::cast) {
        Some(it) => it,
        None => {
            let node = std::any::type_name::<E>();
            panic!("Failed to make ast node `{node}` from text {text}")
        }
    }
}

pub fn ident(name: &str) -> ast::Ident {
    expr_from_text(name)
}

pub fn string(content: &str) -> ast::Str {
    // `\\` must be replaced first so the backslashes introduced by the other escapes are not
    // doubled. `${` is escaped last, for the same reason, so that its literal content is not
    // treated as the start of an interpolation.
    let escaped = content
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace("${", "\\${");
    expr_from_text(&format!("\"{escaped}\""))
}

#[track_caller]
pub fn integer(value: i64) -> ast::Literal {
    assert!(value >= 0, "integer literals are unsigned; use make::negate for negative numbers");
    ast_from_text(&value.to_string())
}

#[track_caller]
pub fn float(value: f64) -> ast::Literal {
    assert!(
        value.is_sign_positive(),
        "float literals are unsigned; use make::negate for negative numbers"
    );
    // `f64::to_string` drops the fractional part for integral values (`1.0` ->
    // "1"), which would parse back as an *integer* literal. Force a decimal
    // point so the result is always a float literal.
    let mut text = value.to_string();
    if !text.contains(['.', 'e', 'E']) {
        text.push_str(".0");
    }
    ast_from_text(&text)
}

pub fn bool_literal(value: bool) -> ast::Ident {
    let text = if value { "true" } else { "false" };
    expr_from_text(text)
}

pub fn null() -> ast::Ident {
    expr_from_text("null")
}

pub fn path_abs(path: &str) -> ast::PathAbs {
    expr_from_text(path)
}

pub fn path_rel(path: &str) -> ast::PathRel {
    expr_from_text(path)
}

pub fn path_home(path: &str) -> ast::PathHome {
    expr_from_text(path)
}

pub fn path_search(name: &str) -> ast::PathSearch {
    expr_from_text(&format!("<{name}>"))
}

pub fn list(items: impl IntoIterator<Item = ast::Expr>) -> ast::List {
    let items_str = items.into_iter().map(|item| item.to_string()).collect::<Vec<_>>().join(" ");
    expr_from_text(&format!("[ {items_str} ]"))
}

pub fn apply(lambda: ast::Expr, argument: ast::Expr) -> ast::Apply {
    expr_from_text(&format!("{lambda} {argument}"))
}

pub fn if_else(condition: ast::Expr, then_body: ast::Expr, else_body: ast::Expr) -> ast::IfElse {
    expr_from_text(&format!("if {condition} then {then_body} else {else_body}"))
}

pub fn bin_op(lhs: ast::Expr, op: BinOpKind, rhs: ast::Expr) -> ast::BinOp {
    let op_str = match op {
        BinOpKind::Concat => "++",
        BinOpKind::Update => "//",
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mul => "*",
        BinOpKind::Div => "/",
        BinOpKind::And => "&&",
        BinOpKind::Equal => "==",
        BinOpKind::Implication => "->",
        BinOpKind::Less => "<",
        BinOpKind::LessOrEq => "<=",
        BinOpKind::More => ">",
        BinOpKind::MoreOrEq => ">=",
        BinOpKind::NotEqual => "!=",
        BinOpKind::Or => "||",
        BinOpKind::PipeRight => "|>",
        BinOpKind::PipeLeft => "<|",
    };
    expr_from_text(&format!("{lhs} {op_str} {rhs}"))
}

pub fn add(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Add, rhs)
}

pub fn sub(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Sub, rhs)
}

pub fn mul(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Mul, rhs)
}

pub fn div(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Div, rhs)
}

pub fn equal(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Equal, rhs)
}

pub fn not_equal(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::NotEqual, rhs)
}

pub fn less(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Less, rhs)
}

pub fn less_or_eq(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::LessOrEq, rhs)
}

pub fn more(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::More, rhs)
}

pub fn more_or_eq(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::MoreOrEq, rhs)
}

pub fn and(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::And, rhs)
}

pub fn or(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Or, rhs)
}

pub fn concat(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Concat, rhs)
}

pub fn update(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Update, rhs)
}

pub fn implication(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::Implication, rhs)
}

pub fn pipe_right(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::PipeRight, rhs)
}

pub fn pipe_left(lhs: ast::Expr, rhs: ast::Expr) -> ast::BinOp {
    bin_op(lhs, BinOpKind::PipeLeft, rhs)
}

pub fn unary_op(op: UnaryOpKind, expr: ast::Expr) -> ast::UnaryOp {
    let op_str = match op {
        UnaryOpKind::Invert => "!",
        UnaryOpKind::Negate => "-",
    };
    expr_from_text(&format!("{op_str}{expr}"))
}

pub fn invert(expr: ast::Expr) -> ast::UnaryOp {
    unary_op(UnaryOpKind::Invert, expr)
}

pub fn negate(expr: ast::Expr) -> ast::UnaryOp {
    unary_op(UnaryOpKind::Negate, expr)
}

pub fn paren(expr: ast::Expr) -> ast::Paren {
    // Use `ast_from_text` rather than `expr_from_text` here: the latter wraps
    // the text in an extra pair of parentheses, which would produce a nested
    // `Paren` node (e.g. `(( 42 ))`) and be picked up first.
    ast_from_text(&format!("( {expr} )"))
}

pub fn assert_expr(condition: ast::Expr, body: ast::Expr) -> ast::Assert {
    expr_from_text(&format!("assert {condition}; {body}"))
}

pub fn with_expr(namespace: ast::Expr, body: ast::Expr) -> ast::With {
    expr_from_text(&format!("with {namespace}; {body}"))
}

pub fn cur_pos() -> ast::CurPos {
    expr_from_text("__curPos")
}

pub fn attrpath(attrs: impl IntoIterator<Item = ast::Attr>) -> ast::Attrpath {
    let attrs_str = attrs.into_iter().map(|attr| attr.to_string()).collect::<Vec<_>>().join(".");
    ast_from_text(&format!("{{ {attrs_str} = null; }}"))
}

pub fn attrpath_value(attrpath: ast::Attrpath, value: ast::Expr) -> ast::AttrpathValue {
    ast_from_text(&format!("{{ {attrpath} = {value}; }}"))
}

pub fn attr_set(entries: impl IntoIterator<Item = ast::Entry>, is_rec: bool) -> ast::AttrSet {
    let rec_keyword = if is_rec { "rec " } else { "" };
    let entries_str =
        entries.into_iter().map(|entry| entry.to_string()).collect::<Vec<_>>().join(" ");
    expr_from_text(&format!("{rec_keyword}{{ {entries_str} }}"))
}

pub fn let_in(entries: impl IntoIterator<Item = ast::Entry>, body: ast::Expr) -> ast::LetIn {
    let entries_str =
        entries.into_iter().map(|entry| entry.to_string()).collect::<Vec<_>>().join(" ");
    expr_from_text(&format!("let {entries_str} in {body}"))
}

pub fn select(expr: ast::Expr, attrpath: ast::Attrpath, default: Option<ast::Expr>) -> ast::Select {
    let default_str = default.map(|d| format!(" or {d}")).unwrap_or_default();
    expr_from_text(&format!("{expr}.{attrpath}{default_str}"))
}

pub fn has_attr(expr: ast::Expr, attrpath: ast::Attrpath) -> ast::HasAttr {
    expr_from_text(&format!("{expr} ? {attrpath}"))
}

pub fn dynamic(expr: ast::Expr) -> ast::Dynamic {
    ast_from_text(&format!("{{ ${{{expr}}} = null; }}"))
}

pub fn inherit(
    from: Option<ast::Expr>,
    attrs: impl IntoIterator<Item = ast::Attr>,
) -> ast::Inherit {
    let from_str = from.map(|e| format!(" ({e})")).unwrap_or_default();
    let attrs_str = attrs.into_iter().map(|attr| attr.to_string()).collect::<Vec<_>>().join(" ");
    ast_from_text(&format!("{{ inherit{from_str} {attrs_str}; }}"))
}

pub fn pat_bind(ident: &str) -> ast::PatBind {
    ast_from_text(&format!("{{ }}@{ident}: null"))
}

pub fn pat_entry(ident: &str, default: Option<ast::Expr>) -> ast::PatEntry {
    let default_str = default.map(|d| format!(" ? {d}")).unwrap_or_default();
    ast_from_text(&format!("{{ {ident}{default_str} }}: null"))
}

pub fn pattern(
    entries: impl IntoIterator<Item = ast::PatEntry>,
    ellipsis: bool,
    bind: Option<&str>,
) -> ast::Pattern {
    let entries_str = entries.into_iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
    let ellipsis_str =
        if ellipsis { if entries_str.is_empty() { "..." } else { ", ..." } } else { "" };
    let bind_str = bind.map(|b| format!(" @ {b}")).unwrap_or_default();
    ast_from_text(&format!("{{ {entries_str}{ellipsis_str} }}{bind_str}: null"))
}

pub fn ident_param(name: &str) -> ast::IdentParam {
    ast_from_text(&format!("{name}: null"))
}

pub fn lambda(param: ast::Param, body: ast::Expr) -> ast::Lambda {
    expr_from_text(&format!("{param}: {body}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{InterpolPart, LiteralKind};

    #[test]
    fn identifiers() {
        assert_eq!(ident("myVariable").to_string(), "myVariable");
    }

    #[test]
    fn strings() {
        assert_eq!(string("hello world").to_string(), "\"hello world\"");
        assert_eq!(string("line1\nline2\ttabbed").to_string(), "\"line1\\nline2\\ttabbed\"");
    }

    #[test]
    fn string_escapes_interpolation() {
        let s = string("${x}");
        assert!(
            matches!(
                s.normalized_parts().as_slice(),
                [InterpolPart::Literal(lit)] if lit == "${x}"
            ),
            "expected a single literal part, got {:?}",
            s.normalized_parts()
        );
    }

    #[test]
    fn integer_is_unsigned_literal() {
        let n = integer(42);
        assert_eq!(n.to_string(), "42");
        assert!(matches!(n.kind(), LiteralKind::Integer(_)));
    }

    #[test]
    #[should_panic(expected = "unsigned")]
    fn integer_rejects_negative() {
        let _ = integer(-5);
    }

    #[test]
    fn float_keeps_decimal_for_integral_values() {
        let f = float(1.0);
        assert_eq!(f.to_string(), "1.0");
        assert!(matches!(f.kind(), LiteralKind::Float(_)));

        let g = float(8.14);
        assert_eq!(g.to_string(), "8.14");
        assert!(matches!(g.kind(), LiteralKind::Float(_)));
    }

    #[test]
    #[should_panic(expected = "unsigned")]
    fn float_rejects_negative() {
        let _ = float(-8.14);
    }

    #[test]
    fn integers() {
        assert_eq!(integer(42).to_string(), "42");
    }

    #[test]
    fn floats() {
        assert_eq!(float(8.14).to_string(), "8.14");
        assert_eq!(float(8.718).to_string(), "8.718");
    }

    #[test]
    fn null_value() {
        assert_eq!(null().to_string(), "null");
    }

    #[test]
    fn paths() {
        assert_eq!(
            path_abs("/etc/nixos/configuration.nix").to_string(),
            "/etc/nixos/configuration.nix"
        );
        assert_eq!(path_rel("./default.nix").to_string(), "./default.nix");
        assert_eq!(path_home("~/.config/nixpkgs").to_string(), "~/.config/nixpkgs");
        assert_eq!(path_search("nixpkgs").to_string(), "<nixpkgs>");
    }

    #[test]
    fn lists() {
        let simple_list = list(vec![integer(1).into(), integer(2).into(), integer(3).into()]);
        assert_eq!(simple_list.to_string(), "[ 1 2 3 ]");

        let mixed_list =
            list(vec![string("hello").into(), integer(42).into(), bool_literal(true).into()]);
        assert_eq!(mixed_list.to_string(), "[ \"hello\" 42 true ]");
    }

    #[test]
    fn booleans() {
        assert_eq!(bool_literal(true).to_string(), "true");
        assert_eq!(bool_literal(false).to_string(), "false");
    }

    #[test]
    fn lambdas() {
        let identity = lambda(ident_param("x").into(), ident("x").into());
        assert_eq!(identity.to_string(), "x: x");

        let const_lambda = lambda(ident_param("n").into(), integer(100).into());
        assert_eq!(const_lambda.to_string(), "n: 100");
    }

    #[test]
    fn function_applications() {
        let app = apply(ident("map").into(), ident("xs").into());
        assert_eq!(app.to_string(), "map xs");

        let map_f_xs =
            apply(apply(ident("map").into(), ident("f").into()).into(), ident("xs").into());
        assert_eq!(map_f_xs.to_string(), "map f xs");
    }

    #[test]
    fn if_then_else() {
        let if_expr = if_else(bool_literal(true).into(), string("yes").into(), string("no").into());
        assert_eq!(if_expr.to_string(), "if true then \"yes\" else \"no\"");
    }

    #[test]
    fn binary_operations() {
        assert_eq!(add(integer(1).into(), integer(2).into()).to_string(), "1 + 2");
        assert_eq!(mul(integer(3).into(), integer(4).into()).to_string(), "3 * 4");
        assert_eq!(equal(ident("x").into(), integer(5).into()).to_string(), "x == 5");
        assert_eq!(
            and(bool_literal(true).into(), bool_literal(false).into()).to_string(),
            "true && false"
        );

        let list1 = list(vec![integer(1).into()]);
        let list2 = list(vec![integer(2).into()]);
        assert_eq!(concat(list1.into(), list2.into()).to_string(), "[ 1 ] ++ [ 2 ]");

        assert_eq!(pipe_right(ident("x").into(), ident("f").into()).to_string(), "x |> f");
        assert_eq!(pipe_left(ident("f").into(), ident("x").into()).to_string(), "f <| x");
    }

    #[test]
    fn unary_operations() {
        assert_eq!(invert(bool_literal(true).into()).to_string(), "!true");
        assert_eq!(negate(integer(42).into()).to_string(), "-42");
    }

    #[test]
    fn other_expressions() {
        let paren_expr = paren(add(integer(1).into(), integer(2).into()).into());
        assert_eq!(paren_expr.to_string(), "( 1 + 2 )");

        let assert_e = assert_expr(bool_literal(true).into(), string("ok").into());
        assert_eq!(assert_e.to_string(), "assert true; \"ok\"");

        let with_e = with_expr(ident("pkgs").into(), ident("stdenv").into());
        assert_eq!(with_e.to_string(), "with pkgs; stdenv");

        assert_eq!(cur_pos().to_string(), "__curPos");
    }

    #[test]
    fn attribute_sets() {
        let simple_set = attr_set(
            vec![
                attrpath_value(attrpath(vec![ident("name").into()]), string("Alice").into()).into(),
                attrpath_value(attrpath(vec![ident("age").into()]), integer(30).into()).into(),
            ],
            false,
        );
        assert_eq!(simple_set.to_string(), "{ name = \"Alice\"; age = 30; }");

        let rec_set = attr_set(
            vec![attrpath_value(attrpath(vec![ident("x").into()]), integer(1).into()).into()],
            true,
        );
        assert_eq!(rec_set.to_string(), "rec { x = 1; }");

        let nested_attrpath =
            attrpath(vec![ident("foo").into(), ident("bar").into(), ident("baz").into()]);
        assert_eq!(nested_attrpath.to_string(), "foo.bar.baz");
    }

    #[test]
    fn let_in_expressions() {
        let simple_let = let_in(
            vec![attrpath_value(attrpath(vec![ident("x").into()]), integer(42).into()).into()],
            ident("x").into(),
        );
        assert_eq!(simple_let.to_string(), "let x = 42; in x");

        let complex_let = let_in(
            vec![
                attrpath_value(attrpath(vec![ident("x").into()]), integer(1).into()).into(),
                attrpath_value(attrpath(vec![ident("y").into()]), integer(2).into()).into(),
            ],
            add(ident("x").into(), ident("y").into()).into(),
        );
        assert_eq!(complex_let.to_string(), "let x = 1; y = 2; in x + y");
    }

    #[test]
    fn select_expressions() {
        let simple_select =
            select(ident("pkgs").into(), attrpath(vec![ident("stdenv").into()]), None);
        assert_eq!(simple_select.to_string(), "pkgs.stdenv");

        let nested_select = select(
            ident("config").into(),
            attrpath(vec![ident("services").into(), ident("nginx").into(), ident("enable").into()]),
            None,
        );
        assert_eq!(nested_select.to_string(), "config.services.nginx.enable");

        let select_with_default = select(
            ident("attrs").into(),
            attrpath(vec![ident("missing").into()]),
            Some(string("fallback").into()),
        );
        assert_eq!(select_with_default.to_string(), "attrs.missing or \"fallback\"");
    }

    #[test]
    fn has_attr_expressions() {
        let has = has_attr(ident("config").into(), attrpath(vec![ident("services").into()]));
        assert_eq!(has.to_string(), "config ? services");

        let nested_has = has_attr(
            ident("attrs").into(),
            attrpath(vec![ident("foo").into(), ident("bar").into()]),
        );
        assert_eq!(nested_has.to_string(), "attrs ? foo.bar");
    }

    #[test]
    fn dynamic_attributes() {
        let dynamic_attr = dynamic(string("dynamicKey").into());
        assert_eq!(dynamic_attr.to_string(), "${\"dynamicKey\"}");

        let dynamic_with_var = dynamic(ident("keyName").into());
        assert_eq!(dynamic_with_var.to_string(), "${keyName}");

        let dynamic_set = attr_set(
            vec![
                attrpath_value(attrpath(vec![dynamic_with_var.into()]), string("value").into())
                    .into(),
            ],
            false,
        );
        assert_eq!(dynamic_set.to_string(), "{ ${keyName} = \"value\"; }");
    }

    #[test]
    fn inherit_expressions() {
        let simple_inherit = inherit(None, vec![ident("x").into(), ident("y").into()]);
        assert_eq!(simple_inherit.to_string(), "inherit x y;");

        let inherit_from = inherit(
            Some(ident("pkgs").into()),
            vec![ident("stdenv").into(), ident("fetchurl").into()],
        );
        assert_eq!(inherit_from.to_string(), "inherit (pkgs) stdenv fetchurl;");

        let set_with_inherit = attr_set(
            vec![
                inherit(None, vec![ident("x").into(), ident("y").into()]).into(),
                attrpath_value(attrpath(vec![ident("z").into()]), integer(3).into()).into(),
            ],
            false,
        );
        assert_eq!(set_with_inherit.to_string(), "{ inherit x y; z = 3; }");
    }

    #[test]
    fn pattern_parameters() {
        let simple_pattern = pattern(vec![pat_entry("x", None), pat_entry("y", None)], false, None);
        assert_eq!(simple_pattern.to_string(), "{ x, y }");

        let pattern_with_default = pattern(
            vec![pat_entry("x", None), pat_entry("y", Some(integer(10).into()))],
            false,
            None,
        );
        assert_eq!(pattern_with_default.to_string(), "{ x, y ? 10 }");

        let pattern_with_ellipsis = pattern(vec![pat_entry("required", None)], true, None);
        assert_eq!(pattern_with_ellipsis.to_string(), "{ required, ... }");

        let pattern_with_bind =
            pattern(vec![pat_entry("x", None), pat_entry("y", None)], false, Some("args"));
        assert_eq!(pattern_with_bind.to_string(), "{ x, y } @ args");

        let complex_pattern = pattern(
            vec![pat_entry("a", None), pat_entry("b", Some(integer(5).into()))],
            true,
            Some("all"),
        );
        assert_eq!(complex_pattern.to_string(), "{ a, b ? 5, ... } @ all");
    }

    #[test]
    fn lambda_with_pattern() {
        let simple_pattern = pattern(vec![pat_entry("x", None), pat_entry("y", None)], false, None);
        let pattern_lambda =
            lambda(simple_pattern.into(), add(ident("x").into(), ident("y").into()).into());
        assert_eq!(pattern_lambda.to_string(), "{ x, y }: x + y");
    }
}
