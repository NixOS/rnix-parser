//! Primitive tree editor.
//!
//! Starting rowan 0.17 exposes only an *immutable* green tree: there is no in-place
//! mutation. Every edit here is therefore takes the target element(s) plus
//! the tree they live in and returns a brand new root [`SyntaxNode`]. Untouched
//! subtrees are structurally shared with the original, so surrounding formatting
//! and trivia are preserved for free.
//!
//! The building block is [`splice`]: it rebuilds the parent of the target with
//! the child range replaced, then bubbles the change up to the root via
//! [`rowan::api::SyntaxNode::replace_with`].
//!
//! Originally adapted from rust-analyzer's `ted` module (MIT license):
//! <https://github.com/rust-lang/rust-analyzer/blob/master/crates/syntax/src/ted.rs>

use std::ops::RangeInclusive;

use rowan::{GreenNode, GreenToken, NodeOrToken};

use crate::{NixLanguage, SyntaxElement, SyntaxKind, SyntaxNode};
use rowan::Language;

/// A green element: either a green node or a green token. Mirrors rowan's
/// internal `GreenElement`
type GreenElement = NodeOrToken<GreenNode, GreenToken>;

pub trait Element {
    fn syntax_element(self) -> SyntaxElement;
}

impl<E: Element + Clone> Element for &'_ E {
    fn syntax_element(self) -> SyntaxElement {
        self.clone().syntax_element()
    }
}
impl Element for SyntaxElement {
    fn syntax_element(self) -> SyntaxElement {
        self
    }
}
impl Element for SyntaxNode {
    fn syntax_element(self) -> SyntaxElement {
        self.into()
    }
}
impl Element for crate::SyntaxToken {
    fn syntax_element(self) -> SyntaxElement {
        self.into()
    }
}

fn to_green(element: &SyntaxElement) -> GreenElement {
    match element {
        NodeOrToken::Node(n) => NodeOrToken::Node(n.green().to_owned()),
        NodeOrToken::Token(t) => NodeOrToken::Token(t.green().to_owned()),
    }
}

/// Replace the children of `target`'s parent in the index range `range` with
/// `replacement`, returning a fresh root.
///
/// All elements in `range` must be siblings (share the same parent). Returns
/// `None` if `target` has no parent (i.e. it is the root itself).
fn splice(
    target: &SyntaxElement,
    range: RangeInclusive<usize>,
    replacement: impl IntoIterator<Item = GreenElement>,
) -> Option<SyntaxNode> {
    let parent = target.parent()?;
    let new_parent_green =
        parent.green().splice_children(*range.start()..*range.end() + 1, replacement);
    let new_root_green = parent.replace_with(new_parent_green);
    Some(SyntaxNode::new_root(new_root_green))
}

/// Replace a single element with another
///
/// Returns `None` if `old` is the root node (it has no parent to rebuild).
pub fn replace(old: impl Element, new: impl Element) -> Option<SyntaxNode> {
    replace_with_many(old, vec![new.syntax_element()])
}

pub fn replace_with_many(old: impl Element, new: Vec<SyntaxElement>) -> Option<SyntaxNode> {
    let old = old.syntax_element();
    let index = old.index();
    let green = new.iter().map(to_green).collect::<Vec<_>>();
    splice(&old, index..=index, green)
}

pub fn replace_all(
    range: RangeInclusive<SyntaxElement>,
    new: Vec<SyntaxElement>,
) -> Option<SyntaxNode> {
    let start = range.start().index();
    let end = range.end().index();
    let green = new.iter().map(to_green).collect::<Vec<_>>();
    splice(range.start(), start..=end, green)
}

pub fn remove(element: impl Element) -> Option<SyntaxNode> {
    let element = element.syntax_element();
    let index = element.index();
    splice(&element, index..=index, std::iter::empty())
}

pub fn remove_all(range: RangeInclusive<SyntaxElement>) -> Option<SyntaxNode> {
    replace_all(range, Vec::new())
}

#[derive(Debug)]
pub struct Position {
    repr: PositionRepr,
}

#[derive(Debug)]
enum PositionRepr {
    FirstChild(SyntaxNode),
    After(SyntaxElement),
}

impl Position {
    pub fn after(elem: impl Element) -> Position {
        Position { repr: PositionRepr::After(elem.syntax_element()) }
    }

    pub fn before(elem: impl Element) -> Position {
        let elem = elem.syntax_element();
        let repr = match elem.prev_sibling_or_token() {
            Some(prev) => PositionRepr::After(prev),
            None => PositionRepr::FirstChild(elem.parent().unwrap()),
        };
        Position { repr }
    }

    pub fn first_child_of(node: &(impl Into<SyntaxNode> + Clone)) -> Position {
        Position { repr: PositionRepr::FirstChild(node.clone().into()) }
    }

    pub fn last_child_of(node: &(impl Into<SyntaxNode> + Clone)) -> Position {
        let node = node.clone().into();
        let repr = match node.last_child_or_token() {
            Some(last) => PositionRepr::After(last),
            None => PositionRepr::FirstChild(node),
        };
        Position { repr }
    }

    fn place(&self) -> (SyntaxNode, usize) {
        match &self.repr {
            PositionRepr::FirstChild(parent) => (parent.clone(), 0),
            PositionRepr::After(child) => (child.parent().unwrap(), child.index() + 1),
        }
    }
}

/// Insert an element at `position` **without** any whitespace fixup, returning
/// the new root. The caller is responsible for spacing. Prefer [`insert`] unless
/// you need exact control over trivia.
pub fn insert_raw(position: Position, elem: impl Element) -> SyntaxNode {
    insert_all_raw(position, vec![elem.syntax_element()])
}

/// Insert several elements at `position` **without** whitespace fixup, returning
/// the new root. See [`insert_raw`].
pub fn insert_all_raw(position: Position, elements: Vec<SyntaxElement>) -> SyntaxNode {
    let (parent, index) = position.place();
    let green = elements.iter().map(to_green).collect::<Vec<_>>();
    let new_parent_green = parent.green().splice_children(index..index, green);
    let new_root_green = parent.replace_with(new_parent_green);
    SyntaxNode::new_root(new_root_green)
}

pub fn insert(position: Position, elem: impl Element) -> SyntaxNode {
    insert_all(position, vec![elem.syntax_element()])
}

pub fn insert_all(position: Position, elements: Vec<SyntaxElement>) -> SyntaxNode {
    let (parent, index) = position.place();
    let mut padded: Vec<GreenElement> = Vec::with_capacity(elements.len() + 2);

    if let Some(ws) = ws_before(&parent, index, &elements) {
        padded.push(NodeOrToken::Token(whitespace(&ws)));
    }
    padded.extend(elements.iter().map(to_green));
    if let Some(ws) = ws_after(&parent, index, &elements) {
        padded.push(NodeOrToken::Token(whitespace(&ws)));
    }

    let new_parent_green = parent.green().splice_children(index..index, padded);
    let new_root_green = parent.replace_with(new_parent_green);
    SyntaxNode::new_root(new_root_green)
}

fn whitespace(text: &str) -> GreenToken {
    let kind = NixLanguage::kind_to_raw(SyntaxKind::TOKEN_WHITESPACE);
    GreenToken::new(kind, text)
}

fn no_space_before(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::TOKEN_SEMICOLON | SyntaxKind::TOKEN_COMMA | SyntaxKind::TOKEN_DOT)
}

fn child_at(parent: &SyntaxNode, index: usize) -> Option<SyntaxElement> {
    parent.children_with_tokens().nth(index)
}

fn ws_before(parent: &SyntaxNode, index: usize, elements: &[SyntaxElement]) -> Option<String> {
    let first = elements.first()?;
    if no_space_before(first.kind()) {
        return None;
    }
    if index == 0 {
        // Inserting as the very first child (e.g. after `{`, `[`). Skip
        return None;
    }
    let prev = child_at(parent, index - 1)?;
    if prev.kind() == SyntaxKind::TOKEN_WHITESPACE {
        return None;
    }
    Some(separator(parent))
}

fn ws_after(parent: &SyntaxNode, index: usize, elements: &[SyntaxElement]) -> Option<String> {
    let next = child_at(parent, index)?;
    if no_space_before(next.kind()) {
        return None;
    }
    if next.kind() == SyntaxKind::TOKEN_WHITESPACE {
        return None;
    }
    let _ = elements;
    Some(separator(parent))
}

fn separator(parent: &SyntaxNode) -> String {
    if let Some(indent) = multiline_indent(parent) {
        format!("\n{indent}")
    } else {
        " ".to_string()
    }
}

fn multiline_indent(parent: &SyntaxNode) -> Option<String> {
    let text = parent.text().to_string();
    if !text.contains('\n') {
        return None;
    }
    for child in parent.children_with_tokens() {
        if child.kind() == SyntaxKind::TOKEN_WHITESPACE
            && let Some(tok) = child.as_token()
            && let Some(idx) = tok.text().rfind('\n')
        {
            return Some(tok.text()[idx + 1..].to_string());
        }
    }
    Some(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Root, ast};
    use rowan::ast::AstNode;

    fn parse(src: &str) -> SyntaxNode {
        Root::parse(src).syntax()
    }

    fn token(root: &SyntaxNode, text: &str) -> SyntaxElement {
        root.descendants_with_tokens()
            .find(|e| e.as_token().is_some_and(|t| t.text() == text))
            .unwrap()
    }

    #[test]
    fn replace_value_token() {
        let root = parse("{ x = 1; }");
        let one = token(&root, "1");
        let repl = token(&parse("(42)"), "42");
        let new_root = replace(one, repl).unwrap();
        assert_eq!(new_root.to_string(), "{ x = 42; }");
    }

    #[test]
    fn replace_preserves_surrounding_formatting() {
        let src = "{\n  a = 1;\n  b = 2;\n}";
        let root = parse(src);
        let two = token(&root, "2");
        let repl = token(&parse("(3)"), "3");
        let new_root = replace(two, repl).unwrap();
        assert_eq!(new_root.to_string(), "{\n  a = 1;\n  b = 3;\n}");
    }

    #[test]
    fn round_trip_is_stable() {
        let root = parse("{ x = 1; }");
        let one = token(&root, "1");
        let repl = token(&parse("(2)"), "2");
        let new_root = replace(one, repl).unwrap();
        let reparsed = Root::parse(&new_root.to_string());
        assert!(reparsed.errors().is_empty());
    }

    #[test]
    fn replace_whole_value_node() {
        let root = parse("{ x = 1; }");
        let value = root.descendants().find(|n| ast::Literal::cast(n.clone()).is_some()).unwrap();
        let repl = parse("({ y = 2; })");
        let new_value: ast::AttrSet = repl.descendants().find_map(ast::AttrSet::cast).unwrap();
        let new_root = replace(value, new_value.syntax().clone()).unwrap();
        assert_eq!(new_root.to_string(), "{ x = { y = 2; }; }");
    }

    #[test]
    fn remove_returns_new_root() {
        let root = parse("[ 1 2 3 ]");
        let two = token(&root, "2");
        let new_root = remove(two).unwrap();
        assert!(!new_root.to_string().contains('2'));
    }

    #[test]
    fn insert_spaces_between_list_items_single_line() {
        let root = parse("[ 1 2 ]");
        let one = token(&root, "1");
        let repl = token(&parse("(9)"), "9");
        let new_root = insert(Position::after(one), repl);
        assert_eq!(new_root.to_string(), "[ 1 9 2 ]");
        assert!(Root::parse(&new_root.to_string()).errors().is_empty());
    }

    #[test]
    fn insert_no_space_before_semicolon() {
        let root = parse("{ x = 1; }");
        let semi = root
            .descendants_with_tokens()
            .find(|e| e.kind() == SyntaxKind::TOKEN_SEMICOLON)
            .unwrap();
        let repl = token(&parse("(2)"), "2");
        let new_root = insert(Position::before(semi), repl);
        assert!(!new_root.to_string().contains("2 ;"), "got: {:?}", new_root.to_string());
    }

    #[test]
    fn insert_multiline_uses_newline_and_indent() {
        let src = "{\n  a = 1;\n}";
        let root = parse(src);
        let set = root.descendants().find_map(ast::AttrSet::cast).unwrap();
        let first_entry = set
            .syntax()
            .children_with_tokens()
            .find(|e| e.kind() == SyntaxKind::NODE_ATTRPATH_VALUE)
            .unwrap();
        let repl = token(&parse("(2)"), "2");
        let new_root = insert(Position::after(first_entry), repl);
        assert!(new_root.to_string().contains("\n  2"), "got: {:?}", new_root.to_string());
    }

    #[test]
    fn insert_raw_adds_no_whitespace() {
        let root = parse("[ 1 2 ]");
        let one = token(&root, "1");
        let repl = token(&parse("(9)"), "9");
        let new_root = insert_raw(Position::after(one), repl);
        assert_eq!(new_root.to_string(), "[ 19 2 ]");
    }

    #[test]
    fn replace_root_returns_none() {
        let root = parse("1");
        assert!(replace(root.clone(), root).is_none());
    }
}
