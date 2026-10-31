use crate::kinds::SyntaxKind::*;
use rowan::{NodeOrToken, ast::AstNode as OtherAstNode};

use crate::ast;

use super::{AstToken, InterpolPart, StrContent, support::children_tokens_u};

impl ast::Str {
    pub fn parts(&self) -> impl Iterator<Item = InterpolPart<StrContent>> + use<> {
        self.syntax().children_with_tokens().filter_map(|child| match child {
            NodeOrToken::Token(token) if token.kind() == TOKEN_STRING_CONTENT => {
                Some(InterpolPart::Literal(StrContent::cast(token).unwrap()))
            }
            NodeOrToken::Token(token) => {
                assert!(token.kind() == TOKEN_STRING_START || token.kind() == TOKEN_STRING_END);
                None
            }
            NodeOrToken::Node(node) => {
                assert_eq!(node.kind(), NODE_INTERPOL);
                Some(InterpolPart::Interpolation(ast::Interpol::cast(node).unwrap()))
            }
        })
    }

    pub fn normalized_parts(&self) -> Vec<InterpolPart<String>> {
        let multiline = children_tokens_u(self).next().is_some_and(|t| t.text() == "''");
        if multiline {
            return strip_indentation(self.parts().collect());
        }
        self.parts()
            .map(|part| match part {
                InterpolPart::Literal(literal) => {
                    InterpolPart::Literal(unescape(literal.syntax().text()))
                }
                InterpolPart::Interpolation(interpol) => InterpolPart::Interpolation(interpol),
            })
            .collect()
    }
}

/// A piece of the text of an indented string, split like Nix's lexer splits it into tokens:
/// text as written, and escapes (`'''`, `''$` and `''\x`), which are tokens of their own.
enum Piece<'a> {
    Written(&'a str),
    Escape(&'a str),
}

impl<'a> Piece<'a> {
    /// What the piece stands for.
    fn value(&self) -> &'a str {
        match *self {
            Piece::Written(text) => text,
            Piece::Escape("'''") => "''",
            Piece::Escape("''$") => "$",
            Piece::Escape(escape) => match &escape[3..] {
                "n" => "\n",
                "r" => "\r",
                "t" => "\t",
                c => c,
            },
        }
    }
}

/// Splits the text of an indented string into [`Piece`]s.
fn pieces(text: &str) -> impl Iterator<Item = Piece<'_>> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let (piece, len) = match rest.find("''") {
            // In the text of a string, `''` starts an escape.
            Some(0) => match rest[2..].chars().next() {
                Some('\'' | '$') => (Piece::Escape(&rest[..3]), 3),
                Some('\\') => {
                    let len = 3 + rest[3..].chars().next().map_or(0, char::len_utf8);
                    (Piece::Escape(&rest[..len]), len)
                }
                _ => (Piece::Written(&rest[..2]), 2),
            },
            Some(start) => (Piece::Written(&rest[..start]), start),
            None => (Piece::Written(rest), rest.len()),
        };
        rest = &rest[len..];
        Some(piece)
    })
}

/// The text of the literal part with index `i`. Nix's lexer reads spaces and a line feed right
/// after the opening `''` as part of it.
fn literal_text(i: usize, literal: &StrContent) -> &str {
    let text = literal.syntax().text();
    match text.trim_start_matches(' ').strip_prefix('\n') {
        Some(rest) if i == 0 => rest,
        _ => text,
    }
}

/// Strips the indentation of an indented string like Nix's parser, which does it on the tokens of
/// its lexer (`ParserState::stripIndentation`): an escape counts as text when it measures the
/// indentation, but what the escape stands for is stripped like what's written.
fn strip_indentation(parts: Vec<InterpolPart<StrContent>>) -> Vec<InterpolPart<String>> {
    // Measure the indentation: the least of any line's but the empty ones.
    let mut at_start_of_line = true;
    let mut min_indent = usize::MAX;
    let mut cur_indent = 0;
    let mut end_of_indentation = |at_start_of_line: &mut bool, cur_indent: usize| {
        if *at_start_of_line {
            *at_start_of_line = false;
            min_indent = min_indent.min(cur_indent);
        }
    };
    for (i, part) in parts.iter().enumerate() {
        let InterpolPart::Literal(literal) = part else {
            end_of_indentation(&mut at_start_of_line, cur_indent);
            continue;
        };
        for piece in pieces(literal_text(i, literal)) {
            let Piece::Written(text) = piece else {
                end_of_indentation(&mut at_start_of_line, cur_indent);
                continue;
            };
            for c in text.chars() {
                if !at_start_of_line {
                    if c == '\n' {
                        at_start_of_line = true;
                        cur_indent = 0;
                    }
                } else if c == ' ' {
                    cur_indent += 1;
                } else if c == '\n' {
                    cur_indent = 0;
                } else {
                    end_of_indentation(&mut at_start_of_line, cur_indent);
                }
            }
        }
    }

    // Strip it.
    let mut normalized = Vec::with_capacity(parts.len());
    let mut at_start_of_line = true;
    let mut cur_dropped = 0;
    for (i, part) in parts.iter().enumerate() {
        let literal = match part {
            InterpolPart::Interpolation(interpol) => {
                at_start_of_line = false;
                cur_dropped = 0;
                normalized.push(InterpolPart::Interpolation(interpol.clone()));
                continue;
            }
            InterpolPart::Literal(literal) => literal,
        };
        let mut pieces = pieces(literal_text(i, literal)).peekable();
        // All of it belonged to the opening `''`.
        if pieces.peek().is_none() {
            continue;
        }
        let mut text = String::new();
        while let Some(piece) = pieces.next() {
            let start = text.len();
            for c in piece.value().chars() {
                if !at_start_of_line {
                    text.push(c);
                    at_start_of_line = c == '\n';
                } else if c == ' ' {
                    if cur_dropped >= min_indent {
                        text.push(c);
                    }
                    cur_dropped += 1;
                } else {
                    at_start_of_line = c == '\n';
                    cur_dropped = 0;
                    text.push(c);
                }
            }
            // Nix drops the last line of the last token if it's only spaces.
            if i == parts.len() - 1
                && pieces.peek().is_none()
                && let Some(newline) = text[start..].rfind('\n')
                && text[start + newline + 1..].bytes().all(|b| b == b' ')
            {
                text.truncate(start + newline + 1);
            }
        }
        normalized.push(InterpolPart::Literal(text));
    }
    normalized
}

/// Interpret escape sequences in the text of a `"…"` string and return the converted value
fn unescape(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut input = input.chars().peekable();
    loop {
        match input.next() {
            None | Some('"') => break,
            Some('\\') => match input.next() {
                None => break,
                Some('n') => output.push('\n'),
                Some('r') => output.push('\r'),
                Some('t') => output.push('\t'),
                Some(c) => output.push(c),
            },
            // Like Nix, read a carriage return, alone or before a line feed, as a line feed.
            Some('\r') => {
                input.next_if_eq(&'\n');
                output.push('\n');
            }
            Some(c) => output.push(c),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::Root;

    use super::*;

    /// The literal parts of the string `source`.
    fn literals(source: &str) -> Vec<String> {
        let Some(ast::Expr::Str(str)) = Root::parse(source).ok().unwrap().expr() else {
            panic!("{source:?} isn't a string");
        };
        str.normalized_parts()
            .into_iter()
            .filter_map(|part| match part {
                InterpolPart::Literal(literal) => Some(literal),
                InterpolPart::Interpolation(_) => None,
            })
            .collect()
    }

    #[test]
    fn string_unescapes() {
        assert_eq!(unescape(r#"Hello\n\"World\" :D"#), "Hello\n\"World\" :D");
        assert_eq!(unescape(r#"\"Hello\""#), "\"Hello\"");

        assert_eq!(literals(r#"''Hello''\n'''World''' :D''"#), ["Hello\n''World'' :D"]);
        assert_eq!(literals(r#"''"Hello"''"#), ["\"Hello\""]);
    }
    #[test]
    fn string_carriage_returns() {
        // An escaped carriage return stays one.
        assert_eq!(unescape("a\rb\r\nc\\\rd"), "a\nb\nc\rd");
    }
    #[test]
    fn indented_string_escapes() {
        // Like Nix, an escape ends the indentation of its line, but the spaces after an escaped
        // line feed are stripped, and only the last line of the last token can be dropped.
        assert_eq!(literals("''  ''\\n ''"), ["\n"]);
        assert_eq!(literals("''''\\\n ''"), ["\n "]);
        assert_eq!(literals("''  ''\\\na\t''"), ["\na\t"]);
        // An escaped carriage return stays one, as in a `"…"` string.
        assert_eq!(literals("''a''\\\rb''"), ["a\rb"]);
    }
    #[test]
    fn parts_leading_ws() {
        let inp = "''\n  hello\n  world''";
        let expr = Root::parse(inp).ok().unwrap().expr().unwrap();
        match expr {
            ast::Expr::Str(str) => {
                assert_eq!(
                    str.normalized_parts(),
                    vec![InterpolPart::Literal("hello\nworld".to_string())]
                )
            }
            _ => unreachable!(),
        }
    }
    #[test]
    fn parts_trailing_ws_single_line() {
        let inp = "''hello ''";
        let expr = Root::parse(inp).ok().unwrap().expr().unwrap();
        match expr {
            ast::Expr::Str(str) => {
                assert_eq!(
                    str.normalized_parts(),
                    vec![InterpolPart::Literal("hello ".to_string())]
                )
            }
            _ => unreachable!(),
        }
    }
    #[test]
    fn parts_trailing_ws_multiline() {
        let inp = "''hello\n ''";
        let expr = Root::parse(inp).ok().unwrap().expr().unwrap();
        match expr {
            ast::Expr::Str(str) => {
                assert_eq!(
                    str.normalized_parts(),
                    vec![InterpolPart::Literal("hello\n".to_string())]
                )
            }
            _ => unreachable!(),
        }
    }
    #[test]
    fn parts() {
        use crate::{NixLanguage, SyntaxNode};
        use rowan::{GreenNodeBuilder, Language};

        fn string_node(content: &str) -> ast::Str {
            let mut builder = GreenNodeBuilder::new();
            builder.start_node(NixLanguage::kind_to_raw(NODE_STRING));
            builder.token(NixLanguage::kind_to_raw(TOKEN_STRING_START), "''");
            builder.token(NixLanguage::kind_to_raw(TOKEN_STRING_CONTENT), content);
            builder.token(NixLanguage::kind_to_raw(TOKEN_STRING_END), "''");
            builder.finish_node();

            ast::Str::cast(SyntaxNode::new_root(builder.finish())).unwrap()
        }

        let txtin = r#"
                        |trailing-whitespace
                              |trailing-whitespace
                    This is a multiline string :D
                      indented by two
                    \'\'\'\'\
                    ''${ interpolation was escaped }
                    two single quotes: '''
                    three single quotes: ''''
                "#
        .replace("|trailing-whitespace", "");

        if let [InterpolPart::Literal(lit)] =
            &ast::Str::normalized_parts(&string_node(txtin.as_str()))[..]
        {
            assert_eq!(
                lit,
                // Get the below with nix repl
                "    \n          \nThis is a multiline string :D\n  indented by two\n\\'\\'\\'\\'\\\n${ interpolation was escaped }\ntwo single quotes: ''\nthree single quotes: '''\n"
            );
        } else {
            unreachable!();
        }
    }

    #[test]
    fn parts_ast() {
        fn assert_eq_ast_ctn(it: &mut dyn Iterator<Item = InterpolPart<String>>, x: &str) {
            let tmp = it.next().expect("unexpected EOF");
            if let InterpolPart::Interpolation(astn) = tmp {
                assert_eq!(astn.expr().unwrap().syntax().to_string(), x);
            } else {
                unreachable!("unexpected literal {:?}", tmp);
            }
        }

        let inp = r#"''

        This version of Nixpkgs requires Nix >= ${requiredVersion}, please upgrade:

        - If you are running NixOS, `nixos-rebuild' can be used to upgrade your system.

        - Alternatively, with Nix > 2.0 `nix upgrade-nix' can be used to imperatively
          upgrade Nix. You may use `nix-env --version' to check which version you have.

        - If you installed Nix using the install script (https://nixos.org/nix/install),
          it is safe to upgrade by running it again:

              curl -L https://nixos.org/nix/install | sh

        For more information, please see the NixOS release notes at
        https://nixos.org/nixos/manual or locally at
        ${toString ./nixos/doc/manual/release-notes}.

        If you need further help, see https://nixos.org/nixos/support.html
      ''"#;
        let expr = Root::parse(inp).ok().unwrap().expr().unwrap();
        match expr {
            ast::Expr::Str(s) => {
                let mut it = s.normalized_parts().into_iter();
                assert_eq!(
                    it.next().unwrap(),
                    InterpolPart::Literal("\nThis version of Nixpkgs requires Nix >= ".to_string())
                );
                assert_eq_ast_ctn(&mut it, "requiredVersion");
                assert_eq!(it.next().unwrap(), InterpolPart::Literal(
                        ", please upgrade:\n\n- If you are running NixOS, `nixos-rebuild' can be used to upgrade your system.\n\n- Alternatively, with Nix > 2.0 `nix upgrade-nix' can be used to imperatively\n  upgrade Nix. You may use `nix-env --version' to check which version you have.\n\n- If you installed Nix using the install script (https://nixos.org/nix/install),\n  it is safe to upgrade by running it again:\n\n      curl -L https://nixos.org/nix/install | sh\n\nFor more information, please see the NixOS release notes at\nhttps://nixos.org/nixos/manual or locally at\n".to_string()
                    ));
                assert_eq_ast_ctn(&mut it, "toString ./nixos/doc/manual/release-notes");
                assert_eq!(
                    it.next().unwrap(),
                    InterpolPart::Literal(
                        ".\n\nIf you need further help, see https://nixos.org/nixos/support.html\n"
                            .to_string()
                    )
                );
            }
            _ => unreachable!(),
        }
    }
}
