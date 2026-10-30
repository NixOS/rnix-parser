# Cases

Inputs on which rnix and Nix disagree, or have disagreed, by cause. `tests/cases.rs` compares
rnix's parse of each input with Nix's, and records the outcomes in `cases.expect`. When a fix
changes an outcome, `UPDATE_EXPECT=1 cargo test` records the new one, and the input stays here as
a test.

Each line of a `nix` block is an input. `⟨LF⟩`, `⟨CR⟩`, `⟨TAB⟩`, `⟨NUL⟩` and `⟨U+XXXX⟩` stand for
characters that would break the line or be invisible.

## Nix accepts these

### zero-dot

A float literal needs a digit from 1 to 9 before a bare `.` (`1.` is a float, `0.` isn't), so
Nix reads `0.a` as selecting `a` from `0`. rnix reads the float `0.` applied to `a`. Lix rejects
`0.a`.

```nix
0.a
0.e1
x: x.a 0.a
[ 0. ]
```

### float-exponent

An exponent needs digits. Nix reads `1.5e` as the float `1.5` followed by the variable `e`; rnix
rejects the token.

```nix
1.5e
1.5ea
.5e != a
f 1.e
```

### slash-operand

`a/b` is a path, but a `/` followed by something other than a path character is division. rnix
reads anything but whitespace after `/` as part of a path.

```nix
a/(b)
2/(3)
a/"b"
a/[ b ]
c/{ }
r/!b
```

### not-operand

Nix accepts `!` at the start of any operand: `a + !b` is `a + (!b)`, and `-!a` is `-(!a)`. rnix
only accepts it at the start of an operand of `//`, `==`, `&&` and the other operators that bind
more loosely than `!`.

```nix
a + !b
-!a
a * !b
"" + !0
a + !b + c
-!a + b
a * !b ? c
a + !b == c
```

### legacy-let

`let { … }` is a simple expression in Nix: it can be applied, selected from, or followed by
`or`. rnix only parses it where an expression starts, and doesn't continue after it.

```nix
f let { body = 1; }
let { body = 1; }.body
let { } == x
e: let { } or
```

### comment-cr

A carriage return ends a `#` comment in Nix (see Nix's `parse-okay-crlf.nix`). rnix's comment
runs to the next line feed.

```nix
f # c⟨CR⟩x
# c⟨CR⟩x
1 # a⟨CR⟩+ 2
```

### string-cr

Nix reads a carriage return, and a carriage return followed by a line feed, in a `"…"` string as
a line feed. An indented string keeps them.

```nix
"a⟨CR⟩b"
"a⟨CR⟩⟨LF⟩b"
''a⟨CR⟩⟨LF⟩b''
```

### indented-escape

Nix strips an indented string's indentation token by token, and an escape (`''$`, `'''`, `''\x`)
is a token of its own: it counts as text when Nix measures the indentation, but a line feed it
produces still starts a line whose spaces get stripped.

```nix
''  ''\n ''
''''\⟨LF⟩ ''
''  ''\⟨LF⟩a⟨TAB⟩''
```

### nul-end

Outside a string, Nix takes a NUL character for the end of the input: it reads `a⟨NUL⟩b ) c`
as `a`. Its lexer returns a character it has no rule for as the token with that character's
number, and token 0 is the end of the input to its parser. A NUL in a string used to end the
string the same way ([NixOS/nix#12131](https://github.com/NixOS/nix/issues/12131)) until Nix 2.26
made it an error ([NixOS/nix#12135](https://github.com/NixOS/nix/pull/12135)), but nothing changed
outside strings. A NUL in a comment is part of the comment to both parsers.

```nix
a⟨NUL⟩b
a⟨NUL⟩b ) c
./a⟨NUL⟩b
# c⟨NUL⟩d⟨LF⟩x
/* ⟨NUL⟩ */ x
⟨NUL⟩
```

### not-utf8

Nix reads bytes: a string or a comment can hold bytes that aren't UTF-8, such as 0xFF. rnix only
parses UTF-8, as `Root::parse` takes a `&str`, which is what rowan stores, so it can't parse such
a file at all: `rnix-compat` reports such a file that Nix accepts as `rnix-rejects`, like Nix's
`eval-fail-toJSON-non-utf-8.nix` in `nix-lang.expect`. The notation here writes characters, not
bytes, so there are no inputs.

## Nix rejects these

### lambda-operand

Nix only allows a function (`x: …`, `{ … }: …`) where an expression can start: after `:`, `=`,
`;`, `in`, `then`, `else`, `(`, `${` and the like. rnix also parses one as an argument, an operand
or a list element.

```nix
f x: x
1 + x: x
[ x: x ]
[ { a }: a ]
{ } { }: x
```

### mixed-pipes

Nix doesn't mix `|>` and `<|` without parentheses. (rnix's `test_data/parser/success/pipe_mixed.nix`
does.)

```nix
a |> b <| c
a <| b |> c
```

### unclosed-paren

rnix takes whatever token follows a parenthesized expression as its closing parenthesis.

```nix
(a ]
(a }
(a ;
```

### indented-string-name

An attribute name can be an identifier, a `"…"` string or `${…}`, but not an indented string.

```nix
a.''b''
{ ''a'' = 1; }
{ inherit ''a''; }
let ''a'' = 1; in a
a ? ''b''
```

### search-path

A search path (`<a/b>`) is names separated by `/`, so it can't start or end with `/`.

```nix
</>
<a/>
<a/b/>
```

### number-range

Nix rejects an integer that doesn't fit in 64 bits, and a float that overflows or is subnormal.

```nix
9223372036854775808
1.e400
5.0e-324
```

### nul

Since 2.26, Nix rejects a NUL character in a string (see `nul-end`).

```nix
"a⟨NUL⟩b"
''a⟨NUL⟩b''
```

### whitespace

Nix's whitespace is space, tab, carriage return and line feed. rnix also skips other Unicode
whitespace.

```nix
a⟨U+000C⟩b
a⟨U+000B⟩b
a⟨U+00A0⟩b
a⟨U+2028⟩b
```

### duplicate-attr

Nix rejects an attribute defined twice in a set or a `let`, unless both definitions are attribute
sets, which it merges. `${"a"}` is the name `a`.

```nix
{ a = 1; a = 2; }
let a = 1; a = 2; in a
{ a = 1; ${"a"} = 2; }
{ a.b = 1; a = 2; }
{ a = { b = 1; }; a.b = 2; }
{ inherit a; a = 1; }
{ a = { }; a = { }; }
```

### duplicate-formal

Nix rejects a function with two parameters of the same name. (rnix has
`ParseError::DuplicatedArgs` for this, but doesn't report it.)

```nix
{ x, x }: x
x@{ x }: x
{ x }@x: x
```

### dynamic-name

Nix rejects names in a `let` or an `inherit` that it can't know while parsing. `${"a"}` is the
name `a`.

```nix
let ${a} = 1; in a
let "${a}" = 1; in a
{ inherit ${a}; }
{ inherit "${a}"; }
let ${"a"} = 1; in a
{ inherit ${"a"}; }
```

## Both agree

### tricky

Inputs that are easy to get wrong.

```nix
1e3
-./a
1 -./a
00.5
01.5
.5
1.
1..2
a.5
a-b
a'
x:x:x
http://example.org/a?b=c#d
<nixpkgs/lib>
./a/${b}c
~/a
f a or
map or [ ]
a.b or c or
a ? b ? c
!a + b
-a.b or c
a < b == c
a |> b |> c
a <| b <| c
x: x |> f
"$${a}"
''$${a}''
''a''$b''
9223372036854775807
./a//b
```
