//! Editing facade over a parsed Nix document.
//!
//! [`Nix`] wraps a parsed [`Root`] and offers attribute-path based reads and
//! edits (`get`/`set`). Unlike [`crate::build`], this layer is **fallible**: its
//! input is user-controlled text, so it returns [`NixError`] instead of
//! panicking.
//!
//! Edits are functional (see [`crate::edit`]): each `set` rebuilds the tree with
//! one value replaced and swaps the held root for the new one. Untouched regions
//! are structurally shared, so their formatting is preserved byte-for-byte.
//!
//! ```
//! use rnix::{document::Nix, ast::make};
//!
//! let mut doc = Nix::parse("{\n  x = 1;\n}").unwrap();
//! doc.set("x", make::integer(2).into()).unwrap();
//! assert_eq!(doc.to_string(), "{\n  x = 2;\n}");
//! ```

use std::fmt;

use rowan::ast::AstNode;

use crate::{Root, SyntaxNode, ast, edit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NixError {
    Parse(String),
    PathNotFound(String),
    NotAnAttrSet(String),
}

impl fmt::Display for NixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NixError::Parse(msg) => write!(f, "parse error: {msg}"),
            NixError::PathNotFound(path) => write!(f, "attribute path not found: {path}"),
            NixError::NotAnAttrSet(path) => write!(f, "not an attribute set: {path}"),
        }
    }
}

impl std::error::Error for NixError {}

#[derive(Debug, Clone)]
pub struct Nix {
    root: SyntaxNode,
}

impl Nix {
    pub fn parse(src: &str) -> Result<Nix, NixError> {
        let parse = Root::parse(src);
        if let Some(err) = parse.errors().first() {
            return Err(NixError::Parse(err.to_string()));
        }
        Ok(Nix { root: parse.syntax() })
    }

    pub fn get(&self, path: &str) -> Option<ast::Expr> {
        let segments: Vec<&str> = path.split('.').collect();
        let set = root_attr_set(&self.root)?;
        find_value(&set, &segments)
    }

    pub fn set(&mut self, path: &str, value: ast::Expr) -> Result<(), NixError> {
        let segments: Vec<&str> = path.split('.').collect();
        let set = root_attr_set(&self.root)
            .ok_or_else(|| NixError::NotAnAttrSet("<root>".to_string()))?;
        let target =
            find_value(&set, &segments).ok_or_else(|| NixError::PathNotFound(path.to_string()))?;

        let new_root = edit::replace(target.syntax().clone(), value.syntax().clone())
            .ok_or_else(|| NixError::PathNotFound(path.to_string()))?;
        self.root = new_root;
        Ok(())
    }

    pub fn syntax(&self) -> &SyntaxNode {
        &self.root
    }
}

impl fmt::Display for Nix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)
    }
}

fn root_attr_set(root: &SyntaxNode) -> Option<ast::AttrSet> {
    root.descendants().find_map(ast::AttrSet::cast)
}

fn attrpath_segments(attrpath: &ast::Attrpath) -> Option<Vec<String>> {
    attrpath
        .attrs()
        .map(|attr| match attr {
            ast::Attr::Ident(ident) => ident.ident_token().map(|t| t.text().to_string()),
            _ => None,
        })
        .collect()
}

/// Walk `set` following `path`, returning the value expression at the end.
///
/// Each `AttrpathValue` may itself carry a multi-segment attrpath
/// (`meta.license = …`), so we match its segments as a prefix of the remaining
/// path and descend into nested sets for the rest.
fn find_value(set: &ast::AttrSet, path: &[&str]) -> Option<ast::Expr> {
    use crate::ast::HasEntry;

    for entry in set.entries() {
        let ast::Entry::AttrpathValue(apv) = entry else { continue };
        let segments = match attrpath_segments(&apv.attrpath()?) {
            Some(s) => s,
            None => continue,
        };
        if segments.len() > path.len() {
            continue;
        }
        if !segments.iter().zip(path).all(|(seg, want)| seg == want) {
            continue;
        }
        let rest = &path[segments.len()..];
        let value = apv.value()?;
        if rest.is_empty() {
            return Some(value);
        }
        let nested = ast::AttrSet::cast(value.syntax().clone())?;
        return find_value(&nested, rest);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::make;

    #[test]
    fn get_scalar() {
        let doc = Nix::parse("{ x = 1; y = 2; }").unwrap();
        assert_eq!(doc.get("x").unwrap().to_string(), "1");
        assert_eq!(doc.get("y").unwrap().to_string(), "2");
        assert!(doc.get("z").is_none());
    }

    #[test]
    fn get_nested_via_flat_attrpath() {
        let doc = Nix::parse("{ meta.license = \"mit\"; }").unwrap();
        assert_eq!(doc.get("meta.license").unwrap().to_string(), "\"mit\"");
    }

    #[test]
    fn get_nested_via_nested_set() {
        let doc = Nix::parse("{ meta = { license = \"mit\"; }; }").unwrap();
        assert_eq!(doc.get("meta.license").unwrap().to_string(), "\"mit\"");
        assert_eq!(doc.get("meta").unwrap().to_string(), "{ license = \"mit\"; }");
    }

    #[test]
    fn set_scalar_value() {
        let mut doc = Nix::parse("{ x = 1; }").unwrap();
        doc.set("x", make::integer(42).into()).unwrap();
        assert_eq!(doc.to_string(), "{ x = 42; }");
    }

    #[test]
    fn set_whole_set() {
        let mut doc = Nix::parse("{ x = 1; }").unwrap();
        let new = make::attr_set(
            vec![
                make::attrpath_value(
                    make::attrpath(vec![make::ident("y").into()]),
                    make::integer(2).into(),
                )
                .into(),
            ],
            false,
        );
        doc.set("x", new.into()).unwrap();
        assert_eq!(doc.to_string(), "{ x = { y = 2; }; }");
    }

    #[test]
    fn set_preserves_surrounding_formatting() {
        let src = "{\n  a = 1;\n  b = 2;\n}";
        let mut doc = Nix::parse(src).unwrap();
        doc.set("b", make::integer(3).into()).unwrap();
        assert_eq!(doc.to_string(), "{\n  a = 1;\n  b = 3;\n}");
    }

    #[test]
    fn set_nested() {
        let mut doc = Nix::parse("{ meta = { license = \"mit\"; }; }").unwrap();
        doc.set("meta.license", make::string("bsd").into()).unwrap();
        assert_eq!(doc.to_string(), "{ meta = { license = \"bsd\"; }; }");
    }

    #[test]
    fn set_missing_path_errors() {
        let mut doc = Nix::parse("{ x = 1; }").unwrap();
        let err = doc.set("y", make::integer(2).into()).unwrap_err();
        assert_eq!(err, NixError::PathNotFound("y".to_string()));
    }

    #[test]
    fn parse_error_is_reported() {
        let err = Nix::parse("{ x = ").unwrap_err();
        assert!(matches!(err, NixError::Parse(_)));
    }

    #[test]
    fn edited_document_reparses_cleanly() {
        let mut doc = Nix::parse("{ x = 1; }").unwrap();
        doc.set("x", make::integer(9).into()).unwrap();
        assert!(Root::parse(&doc.to_string()).errors().is_empty());
    }
}
