//! Ergonomic generation facade over [`crate::ast::make`].
//!
//! `make` is primitive and verbose; this module offers fluent builders and a
//! handful of free helpers for the common case of assembling Nix fragments from
//! scratch.
//!
//! ```
//! use rnix::build as nix;
//!
//! let set = nix::set()
//!     .entry("x", nix::int(1))
//!     .entry("y", nix::string("hi"))
//!     .build();
//! assert_eq!(set.to_string(), "{ x = 1; y = \"hi\"; }");
//! ```

use crate::ast::{self, make};

pub fn set() -> AttrSetBuilder {
    AttrSetBuilder { entries: Vec::new(), recursive: false }
}

pub fn list() -> ListBuilder {
    ListBuilder { items: Vec::new() }
}

/// An identifier expression, e.g. `pkgs`.
pub fn ident(name: &str) -> ast::Expr {
    make::ident(name).into()
}

pub fn string(content: &str) -> ast::Expr {
    make::string(content).into()
}

pub fn int(value: i64) -> ast::Expr {
    make::integer(value).into()
}

pub fn float(value: f64) -> ast::Expr {
    make::float(value).into()
}

pub fn bool(value: bool) -> ast::Expr {
    make::bool_literal(value).into()
}

pub fn null() -> ast::Expr {
    make::null().into()
}

pub fn neg(expr: ast::Expr) -> ast::Expr {
    make::negate(expr).into()
}

/// A dotted attribute selection, e.g. `select("licenses.mit")`. Panics if `path` is empty.
pub fn select(path: &str) -> ast::Expr {
    let mut segs = path.split('.');
    let base: ast::Expr = make::ident(segs.next().expect("empty selection path")).into();
    let rest: Vec<ast::Attr> = segs.map(|s| make::ident(s).into()).collect();
    if rest.is_empty() {
        return base;
    }
    make::select(base, make::attrpath(rest), None).into()
}

pub struct AttrSetBuilder {
    entries: Vec<ast::Entry>,
    recursive: bool,
}

impl AttrSetBuilder {
    pub fn entry(mut self, path: &str, value: ast::Expr) -> Self {
        let attrs = path.split('.').map(|seg| make::ident(seg).into());
        let attrpath = make::attrpath(attrs);
        self.entries.push(make::attrpath_value(attrpath, value).into());
        self
    }

    pub fn inherit(mut self, from: Option<ast::Expr>, names: &[&str]) -> Self {
        let attrs = names.iter().map(|n| make::ident(n).into());
        self.entries.push(make::inherit(from, attrs).into());
        self
    }

    pub fn recursive(mut self) -> Self {
        self.recursive = true;
        self
    }

    pub fn build(self) -> ast::AttrSet {
        make::attr_set(self.entries, self.recursive)
    }
}

impl From<AttrSetBuilder> for ast::Expr {
    fn from(b: AttrSetBuilder) -> Self {
        b.build().into()
    }
}

pub struct ListBuilder {
    items: Vec<ast::Expr>,
}

impl ListBuilder {
    pub fn item(mut self, expr: ast::Expr) -> Self {
        self.items.push(expr);
        self
    }

    pub fn items(mut self, exprs: impl IntoIterator<Item = ast::Expr>) -> Self {
        self.items.extend(exprs);
        self
    }

    pub fn build(self) -> ast::List {
        make::list(self.items)
    }
}

impl From<ListBuilder> for ast::Expr {
    fn from(b: ListBuilder) -> Self {
        b.build().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Root;

    fn assert_valid(text: &str) {
        let parse = Root::parse(&format!("({text})"));
        assert!(parse.errors().is_empty(), "generated invalid Nix {text:?}: {:?}", parse.errors());
    }

    #[test]
    fn empty_set() {
        let s = set().build();
        assert_eq!(s.to_string(), "{  }");
        assert_valid(&s.to_string());
    }

    #[test]
    fn set_with_scalar_entries() {
        let s = set().entry("x", int(1)).entry("y", string("hi")).build();
        assert_eq!(s.to_string(), "{ x = 1; y = \"hi\"; }");
        assert_valid(&s.to_string());
    }

    #[test]
    fn recursive_set() {
        let s = set().recursive().entry("x", int(1)).build();
        assert_eq!(s.to_string(), "rec { x = 1; }");
        assert_valid(&s.to_string());
    }

    #[test]
    fn nested_attrpath_entry() {
        let s = set().entry("meta.license", select("licenses.mit")).build();
        assert_eq!(s.to_string(), "{ meta.license = licenses.mit; }");
        assert_valid(&s.to_string());
    }

    #[test]
    fn set_with_inherit() {
        let s = set().inherit(None, &["a", "b"]).build();
        assert_eq!(s.to_string(), "{ inherit a b; }");
        assert_valid(&s.to_string());

        let s = set().inherit(Some(ident("pkgs")), &["stdenv"]).build();
        assert_eq!(s.to_string(), "{ inherit (pkgs) stdenv; }");
        assert_valid(&s.to_string());
    }

    #[test]
    fn list_of_items() {
        let l = list().item(int(1)).item(int(2)).item(string("x")).build();
        assert_eq!(l.to_string(), "[ 1 2 \"x\" ]");
        assert_valid(&l.to_string());
    }

    #[test]
    fn list_items_iter() {
        let l = list().items(vec![int(1), int(2)]).build();
        assert_eq!(l.to_string(), "[ 1 2 ]");
    }

    #[test]
    fn nested_set_and_list() {
        let inner = set().entry("y", int(2));
        let l = list().item(int(1)).item(inner.into()).build();
        let s = set().entry("xs", l.into()).build();
        assert_eq!(s.to_string(), "{ xs = [ 1 { y = 2; } ]; }");
        assert_valid(&s.to_string());
    }

    #[test]
    fn scalars_roundtrip() {
        assert_eq!(neg(int(5)).to_string(), "-5");
        assert_eq!(bool(true).to_string(), "true");
        assert_eq!(null().to_string(), "null");
        assert_eq!(float(1.0).to_string(), "1.0");
    }
}
