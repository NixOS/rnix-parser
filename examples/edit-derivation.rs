//! 1. bump `version`,
//! 2. replace `src.hash` with a made-up hash, and
//! 3. add a `cat` dependency to the `buildInputs` list,

use rnix::{SyntaxKind, SyntaxNode, ast, build as nix, document::Nix, edit};
use rowan::ast::AstNode;

fn find_attr_value(scope: &SyntaxNode, name: &str) -> Option<ast::Expr> {
    scope
        .descendants()
        .filter_map(ast::AttrpathValue::cast)
        .find(|apv| apv.attrpath().map(|p| p.syntax().text() == name).unwrap_or(false))
        .and_then(|apv| apv.value())
}

fn find_list_after_attr(root: &SyntaxNode, name: &str) -> Option<ast::List> {
    find_attr_value(root, name).and_then(|value| {
        value
            .syntax()
            .descendants()
            .find(|n| n.kind() == SyntaxKind::NODE_LIST)
            .and_then(ast::List::cast)
    })
}

fn main() {
    let src = include_str!("hello.nix");
    let mut doc = Nix::parse(src).expect("hello.nix should parse cleanly");

    doc.set("version", nix::string("2.99.0")).expect("`version` attribute should exist");

    let src = doc.get("src").expect("`src` attribute should exist");
    let hash_value = find_attr_value(src.syntax(), "hash").expect("`src` should contain a `hash`");
    let new_hash = nix::string("sha256-0000000000000000000000000000000000000000000=");
    let new_root = edit::replace(hash_value.syntax().clone(), new_hash.syntax().clone())
        .expect("hash value has a parent to rebuild");
    doc = Nix::parse(&new_root.to_string()).expect("edited tree should reparse");

    // Add `cat` to buildInputs
    let root = doc.syntax().clone();
    let list =
        find_list_after_attr(&root, "buildInputs").expect("`buildInputs` should contain a list");

    let last_item = list.items().last().expect("buildInputs list should have at least one item");
    let cat = nix::ident("cat");
    let new_root =
        edit::insert(edit::Position::after(last_item.syntax().clone()), cat.syntax().clone());

    println!("{new_root}");
}
