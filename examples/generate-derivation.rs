//! Generate a Nix derivation from scratch.
//!
//! ```text
//! { lib, stdenv, fetchurl }:
//! stdenv.mkDerivation {
//!   pname = "hello";
//!   version = "2.12.3";
//!   src = fetchurl { url = "…"; hash = "…"; };
//!   buildInputs = [ ];
//!   meta = with lib; { … };
//! }
//! ```
//!
//! Let Nix parse it:
//!
//! ```shell
//! cargo run --example generate-derivation | nix-instantiate --parse -
//! ```

use rnix::ast::make;
use rnix::{Root, build as nix};

fn main() {
    let src_args = nix::set()
        .entry("url", nix::string("mirror://gnu/hello/hello-2.12.3.tar.gz"))
        .entry("hash", nix::string("sha256-DV9gFUOC/uELEUocNOeF2LH0kgc64tOm97FHaHs2aqA="))
        .build();
    let src = make::apply(nix::ident("fetchurl"), src_args.into());

    let meta_set = nix::set()
        .entry("description", nix::string("A program that produces a familiar, friendly greeting"))
        .entry("homepage", nix::string("https://www.gnu.org/software/hello/"))
        .entry("license", nix::select("licenses.gpl3Plus"))
        .entry("platforms", nix::select("platforms.all"))
        .entry("mainProgram", nix::string("hello"))
        .build();
    let meta = make::with_expr(nix::ident("lib"), meta_set.into());

    let drv_attrs = nix::set()
        .entry("pname", nix::string("hello"))
        .entry("version", nix::string("2.12.3"))
        .entry("src", src.into())
        .entry("buildInputs", nix::list().build().into())
        .entry("doCheck", nix::bool(true))
        .entry("meta", meta.into())
        .build();

    let mk_derivation = make::select(
        nix::ident("stdenv"),
        make::attrpath(vec![make::ident("mkDerivation").into()]),
        None,
    );
    let body = make::apply(mk_derivation.into(), drv_attrs.into());

    let params = make::pattern(
        vec![
            make::pat_entry("lib", None),
            make::pat_entry("stdenv", None),
            make::pat_entry("fetchurl", None),
        ],
        false,
        None,
    );
    let package = make::lambda(params.into(), body.into());

    let generated = package.to_string();

    // Prove the generated fragment is syntactically valid Nix.
    let parse = Root::parse(&generated);
    assert!(parse.errors().is_empty(), "generated invalid Nix: {:?}", parse.errors());

    println!("{generated}");
}
