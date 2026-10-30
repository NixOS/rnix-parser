use expect_test::{Expect, expect};

fn check(source: &str, expected: Expect) {
    let parse = rnix::Root::parse(source);
    assert!(parse.errors().is_empty(), "{source:?}: {:?}", parse.errors());
    expected.assert_eq(&rnix_compat::render(&parse.syntax()));
}

#[test]
fn groups_every_expression() {
    check("1 + 2 * 3", expect!["( ( 1 ) + ( ( 2 ) * ( 3 ) ) )"]);
    check("f a b", expect!["( ( ( f ) ( a ) ) ( b ) )"]);
    check("-a.b or c", expect!["( - ( ( a ) . b or ( c ) ) )"]);
    check(
        "let x = 1; in with y; assert z; if c then x else y",
        expect![
            "( let x = ( 1 ) ; in ( with ( y ) ; ( assert ( z ) ; ( if ( c ) then ( x ) else ( y ) ) ) ) )"
        ],
    );
}

#[test]
fn leaves_names_alone() {
    check(
        "{ a.b = 1; \"c\".${d} = 2; inherit e; inherit (f) g; }",
        expect![[
            r#"( { a . b = ( 1 ) ; "c" . ${ ( d ) } = ( 2 ) ; inherit e ; inherit ( ( f ) ) g ; } )"#
        ]],
    );
    check("x: y", expect!["( x : ( y ) )"]);
    check("{ a ? b, c, ... }@args: a", expect!["( { a ? ( b ) , c , ... } @ args : ( a ) )"]);
    check("a ? b.c", expect!["( ( a ) ? b . c )"]);
    check("map or [ ]", expect!["( ( ( map ) or ) ( [ ] ) )"]);
}

#[test]
fn keeps_strings_and_paths_together() {
    check(r#""a${b}c""#, expect![[r#"( "a${ ( b ) }c" )"#]]);
    check(
        "''\n  a${b}\n''",
        expect![[r#"
        ( ''
          a${ ( b ) }
        '' )"#]],
    );
    check("./a/${b}.nix", expect!["( ./a/${ ( b ) }.nix )"]);
    check("<nixpkgs/lib>", expect!["( <nixpkgs/lib> )"]);
}

#[test]
fn writes_numbers_and_uris_canonically() {
    check(
        "[ 007 1.50 .5 1.e3 0.0 http://example.org/$x ]",
        expect![[
            r#"( [ ( 7 ) ( 1.5e0 ) ( 5.e-1 ) ( 1.e3 ) ( 0.0 ) ( "http://example.org/\$x" ) ] )"#
        ]],
    );
}

#[test]
fn leaves_comments_out() {
    check("a /* b */ + # c\n d", expect!["( ( a ) + ( d ) )"]);
}
