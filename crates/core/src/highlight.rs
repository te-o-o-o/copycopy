//! Colouring for a snippet whose language [`crate::lang`] has recognised.
//!
//! It answers byte ranges and a kind, never a colour: `core` knows nothing
//! about the interface, and which hue a keyword takes is the palette's business
//! — which is also how the five themes get this for free.
//!
//! Deliberately approximate. A clipboard entry is a fragment: it starts in the
//! middle of a function and ends mid-expression, so there is nothing to parse
//! and no grammar to satisfy. A scanner that knows comments, strings, numbers
//! and a keyword list reads a fragment as well as a parser would, and it cannot
//! be thrown off by the half of the file that was not copied.
//!
//! Two invariants the interface relies on, both checked by the tests: the
//! ranges come out sorted and never overlap, and every bound falls on a
//! character boundary — the view slices `body[range]` straight from them.

use crate::lang::Lang;
use std::ops::Range;

/// What a stretch of the snippet is. Six kinds, because the palette has exactly
/// five badge tints plus `faint` to spend on them, and because a seventh would
/// mean inventing a colour for every theme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Token {
    Comment,
    /// A string or character literal, quotes included.
    Str,
    Number,
    Keyword,
    /// The element name in a markup language, `<` included.
    Tag,
    /// The left-hand side of a pair: a JSON or YAML key, an XML attribute name,
    /// a CSS property.
    Key,
}

/// The stretches worth colouring, in order. Everything not named is ordinary
/// text and keeps the theme's own colour.
pub fn spans(text: &str, lang: Lang) -> Vec<(Range<usize>, Token)> {
    match lang {
        Lang::Html | Lang::Xml => markup(text),
        other => code(text, syntax(other)),
    }
}

/// How one language spells the four things every scanner has to find.
struct Syntax {
    line: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    quotes: &'static [u8],
    /// Quotes that open a *character* literal rather than a string: they only
    /// count when they close within a few bytes on the same line. Without this,
    /// a Rust lifetime — `&'a str` — paints the rest of the line, and so does
    /// the apostrophe in any prose the detector mistook for code.
    bounded: &'static [u8],
    keywords: &'static [&'static str],
    /// SQL is written `SELECT` as readily as `select`.
    fold_case: bool,
    /// A quoted string followed by `:` is a key, not a value — JSON, YAML.
    quoted_keys: bool,
    /// A bare word opening a line and followed by `:` or `=` is a key — YAML,
    /// TOML, and a CSS property.
    bare_keys: bool,
}

const C_LIKE: Syntax = Syntax {
    line: &["//"],
    block: Some(("/*", "*/")),
    quotes: b"\"'",
    bounded: b"",
    keywords: &[],
    fold_case: false,
    quoted_keys: false,
    bare_keys: false,
};

const HASH: Syntax = Syntax {
    line: &["#"],
    block: None,
    quotes: b"\"'",
    bounded: b"",
    keywords: &[],
    fold_case: false,
    quoted_keys: false,
    bare_keys: false,
};

fn syntax(lang: Lang) -> Syntax {
    match lang {
        Lang::Rust => Syntax {
            keywords: RUST,
            bounded: b"'",
            ..C_LIKE
        },
        Lang::Go => Syntax {
            keywords: GO,
            bounded: b"'",
            ..C_LIKE
        },
        // The backtick is a template literal, and only these two have one.
        Lang::JavaScript => Syntax {
            quotes: b"\"'`",
            keywords: JS,
            ..C_LIKE
        },
        Lang::TypeScript => Syntax {
            quotes: b"\"'`",
            keywords: TS,
            ..C_LIKE
        },
        // `#` as well as `//`: the opening `<?php` tag aside, both are legal.
        Lang::Php => Syntax {
            line: &["//", "#"],
            keywords: PHP,
            ..C_LIKE
        },
        Lang::Python => Syntax {
            keywords: PYTHON,
            ..HASH
        },
        Lang::Shell => Syntax {
            keywords: SHELL,
            ..HASH
        },
        Lang::Yaml => Syntax {
            keywords: YAML,
            quoted_keys: true,
            bare_keys: true,
            ..HASH
        },
        Lang::Toml => Syntax {
            keywords: BOOLEANS,
            bare_keys: true,
            ..HASH
        },
        Lang::Sql => Syntax {
            line: &["--"],
            quotes: b"'\"",
            keywords: SQL,
            fold_case: true,
            ..C_LIKE
        },
        // No comment syntax at all, which is half of what makes JSON JSON.
        Lang::Json => Syntax {
            line: &[],
            block: None,
            quotes: b"\"",
            keywords: JSON,
            quoted_keys: true,
            ..C_LIKE
        },
        Lang::Css => Syntax {
            line: &[],
            keywords: &[],
            bare_keys: true,
            ..C_LIKE
        },
        // Handled by `markup`, never reached.
        Lang::Html | Lang::Xml => C_LIKE,
    }
}

// --------------------------------------------------------------- the scanner

fn code(text: &str, syn: Syntax) -> Vec<(Range<usize>, Token)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        // A comment opener wins over everything: a `"` inside it is not a
        // string, and a `//` inside a string is not a comment. Order decides
        // that, not a special case.
        if let Some(open) = syn.line.iter().find(|m| starts(bytes, i, m)) {
            let end = line_end(bytes, i + open.len());
            out.push((i..end, Token::Comment));
            i = end;
            continue;
        }
        if let Some((open, close)) = syn.block {
            if starts(bytes, i, open) {
                let end = find(bytes, i + open.len(), close)
                    .map(|at| at + close.len())
                    .unwrap_or(bytes.len());
                out.push((i..end, Token::Comment));
                i = end;
                continue;
            }
        }
        if syn.quotes.contains(&bytes[i]) {
            let (end, closed) = string_end(bytes, i);
            // A character literal has a shape, not just a length: `&'a str`
            // closes on the next lifetime nine bytes later, which a size limit
            // alone would happily swallow. Anything that is not one character
            // or one escape is an apostrophe or a lifetime — leave it alone.
            if syn.bounded.contains(&bytes[i])
                && !(closed && is_char_literal(&bytes[i + 1..end - 1]))
            {
                i += 1;
                continue;
            }
            // In JSON and YAML the same literal is a key or a value depending
            // only on what follows it.
            let kind = if syn.quoted_keys && next_is(bytes, end, b':') {
                Token::Key
            } else {
                Token::Str
            };
            out.push((i..end, kind));
            i = end;
            continue;
        }
        if bytes[i].is_ascii_digit() && !ident_byte(prev(bytes, i)) {
            let end = number_end(bytes, i);
            out.push((i..end, Token::Number));
            i = end;
            continue;
        }
        if ident_start(bytes[i]) {
            let end = ident_end(bytes, i);
            let word = &text[i..end];
            if matches_keyword(word, syn.keywords, syn.fold_case) {
                out.push((i..end, Token::Keyword));
            } else if syn.bare_keys
                && line_is_blank_before(bytes, i)
                && (next_is(bytes, end, b':') || next_is(bytes, end, b'='))
            {
                out.push((i..end, Token::Key));
            }
            i = end;
            continue;
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- the markup

/// HTML and XML: a tag is the only thing worth finding, and everything else on
/// the page is prose that must stay readable.
fn markup(text: &str) -> Vec<(Range<usize>, Token)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if starts(bytes, i, "<!--") {
            let end = find(bytes, i + 4, "-->")
                .map(|at| at + 3)
                .unwrap_or(bytes.len());
            out.push((i..end, Token::Comment));
            i = end;
            continue;
        }
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // The name, bracket and closing slash together: `</div` reads as one
        // thing, and colouring the bracket apart from the name only looks
        // broken.
        let mut at = i + 1;
        while at < bytes.len() && (bytes[at] == b'/' || bytes[at] == b'!' || bytes[at] == b'?') {
            at += 1;
        }
        let name = ident_end(bytes, at);
        if name > at {
            out.push((i..name, Token::Tag));
        }
        i = name;
        // Inside the tag, until it closes: attribute names on the left of the
        // `=`, quoted values on the right.
        while i < bytes.len() && bytes[i] != b'>' {
            if bytes[i] == b'"' || bytes[i] == b'\'' {
                let (end, _) = string_end(bytes, i);
                out.push((i..end, Token::Str));
                i = end;
            } else if ident_start(bytes[i]) {
                let end = ident_end(bytes, i);
                out.push((i..end, Token::Key));
                i = end;
            } else {
                i += 1;
            }
        }
    }
    out
}

// ----------------------------------------------------------------- the tools

fn starts(bytes: &[u8], at: usize, needle: &str) -> bool {
    bytes[at..].starts_with(needle.as_bytes())
}

fn find(bytes: &[u8], from: usize, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    (from..bytes.len().saturating_sub(needle.len() - 1)).find(|&at| bytes[at..].starts_with(needle))
}

fn line_end(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|at| from + at)
        .unwrap_or(bytes.len())
}

/// Past the closing quote, and whether it was actually found. A fragment cut
/// mid-string is the normal case, not an error, so an unclosed literal still
/// ends somewhere — but the caller is told, because for a character literal
/// that changes the answer.
fn string_end(bytes: &[u8], start: usize) -> (usize, bool) {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == quote => return (i + 1, true),
            // A line break ends an unterminated literal: without this, one
            // stray apostrophe in an English comment would paint the rest of
            // the snippet as a string.
            b'\n' => return (i, false),
            _ => i += 1,
        }
    }
    (bytes.len(), false)
}

/// What sits between the quotes of a real character literal: one escape, or
/// exactly one character — no more, which is the whole point.
fn is_char_literal(content: &[u8]) -> bool {
    match content.first() {
        None => false,
        // `\n`, `\'`, `\u{1F600}` — bounded by the longest of them.
        Some(b'\\') => content.len() <= 10,
        Some(&lead) => content.len() == utf8_len(lead),
    }
}

/// How many bytes the character starting with this byte occupies.
fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// Digits, and whatever a literal glues to them: `0xFF`, `1_000`, `3.14`,
/// `1e-9`, `100px`, `42u32`.
fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        let belongs = b.is_ascii_alphanumeric()
            || b == b'_'
            || b == b'.'
            // The sign of an exponent, and nowhere else: `1e-9` is one number,
            // `n-9` is two.
            || (matches!(b, b'-' | b'+') && i > start && matches!(bytes[i - 1], b'e' | b'E'));
        if !belongs {
            break;
        }
        i += 1;
    }
    i
}

/// Bytes above ASCII count as part of a word. Not for correctness of the
/// language — for correctness of the ranges: stopping mid-character would hand
/// the view a slice it cannot take.
fn ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

fn ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$' || b == b'@' || b >= 0x80
}

fn ident_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    if i < bytes.len() && (bytes[i] == b'@' || bytes[i] == b'$') {
        i += 1;
    }
    while i < bytes.len() && ident_byte(bytes[i]) {
        i += 1;
    }
    i
}

fn prev(bytes: &[u8], at: usize) -> u8 {
    if at == 0 {
        b' '
    } else {
        bytes[at - 1]
    }
}

fn next_is(bytes: &[u8], from: usize, want: u8) -> bool {
    bytes[from..]
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == want)
}

fn line_is_blank_before(bytes: &[u8], at: usize) -> bool {
    bytes[..at]
        .iter()
        .rev()
        .take_while(|&&b| b != b'\n')
        .all(|b| b.is_ascii_whitespace() || *b == b'-')
}

fn matches_keyword(word: &str, keywords: &[&str], fold_case: bool) -> bool {
    if fold_case {
        keywords.iter().any(|k| k.eq_ignore_ascii_case(word))
    } else {
        keywords.contains(&word)
    }
}

// -------------------------------------------------------------- the keywords
//
// Not exhaustive, and not meant to be. A list covers what a fragment of that
// language actually contains; the rarities it misses cost one uncoloured word,
// which nobody notices, while a list long enough to hold them all would make
// every snippet look like a Christmas tree.

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const PYTHON: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];

const JS: &[&str] = &[
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "yield",
];

const TS: &[&str] = &[
    "any",
    "as",
    "async",
    "await",
    "boolean",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "declare",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "namespace",
    "never",
    "new",
    "null",
    "number",
    "of",
    "private",
    "protected",
    "public",
    "readonly",
    "return",
    "string",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "unknown",
    "var",
    "void",
    "while",
    "yield",
];

const PHP: &[&str] = &[
    "abstract",
    "array",
    "as",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "declare",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "extends",
    "false",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "global",
    "if",
    "implements",
    "include",
    "instanceof",
    "interface",
    "namespace",
    "new",
    "null",
    "private",
    "protected",
    "public",
    "require",
    "return",
    "static",
    "switch",
    "this",
    "throw",
    "trait",
    "true",
    "try",
    "use",
    "var",
    "while",
    "yield",
];

const GO: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "nil",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "true",
    "false",
    "type",
    "var",
];

const SHELL: &[&str] = &[
    "case", "cd", "do", "done", "echo", "elif", "else", "esac", "exit", "export", "fi", "for",
    "function", "if", "in", "local", "readonly", "return", "set", "source", "then", "unset",
    "until", "while",
];

const SQL: &[&str] = &[
    "all",
    "alter",
    "and",
    "as",
    "asc",
    "avg",
    "between",
    "by",
    "case",
    "count",
    "create",
    "delete",
    "desc",
    "distinct",
    "drop",
    "else",
    "end",
    "exists",
    "foreign",
    "from",
    "group",
    "having",
    "in",
    "index",
    "inner",
    "insert",
    "into",
    "is",
    "join",
    "key",
    "left",
    "like",
    "limit",
    "max",
    "min",
    "not",
    "null",
    "offset",
    "on",
    "or",
    "order",
    "outer",
    "primary",
    "references",
    "right",
    "select",
    "set",
    "sum",
    "table",
    "then",
    "union",
    "update",
    "values",
    "when",
    "where",
];

const JSON: &[&str] = &["true", "false", "null"];
const YAML: &[&str] = &["true", "false", "null", "yes", "no"];
const BOOLEANS: &[&str] = &["true", "false"];

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str, lang: Lang) -> Vec<(&str, Token)> {
        spans(text, lang)
            .into_iter()
            .map(|(range, token)| (&text[range], token))
            .collect()
    }

    /// The two invariants the view depends on. Anything it slices has to be a
    /// real slice, and spans are consumed in order without backtracking.
    fn check_invariants(text: &str, lang: Lang) {
        let mut previous_end = 0;
        for (range, _) in spans(text, lang) {
            assert!(range.start >= previous_end, "spans overlap in {text:?}");
            assert!(
                text.is_char_boundary(range.start) && text.is_char_boundary(range.end),
                "span {range:?} cuts a character in {text:?}"
            );
            previous_end = range.end;
        }
    }

    #[test]
    fn rust_snippet() {
        let found = kinds(
            "let n = 42; // note\nfn go() -> &str { \"ok\" }",
            Lang::Rust,
        );
        assert!(found.contains(&("let", Token::Keyword)));
        assert!(found.contains(&("42", Token::Number)));
        assert!(found.contains(&("// note", Token::Comment)));
        assert!(found.contains(&("fn", Token::Keyword)));
        assert!(found.contains(&("\"ok\"", Token::Str)));
    }

    #[test]
    fn a_comment_swallows_what_looks_like_code() {
        let found = kinds("// let \"x\" = 3", Lang::Rust);
        assert_eq!(found, vec![("// let \"x\" = 3", Token::Comment)]);
    }

    #[test]
    fn a_string_swallows_what_looks_like_a_comment() {
        let found = kinds("let url = \"https://a // b\";", Lang::Rust);
        assert!(found.contains(&("\"https://a // b\"", Token::Str)));
        assert!(!found.iter().any(|(_, t)| *t == Token::Comment));
    }

    /// The apostrophe in "doesn't" opens a literal that never closes. Without
    /// the line break ending it, everything after would be painted as a string.
    #[test]
    fn an_unclosed_quote_stops_at_the_end_of_its_line() {
        let found = kinds("# doesn't\nx = 1", Lang::Python);
        assert!(found.contains(&("1", Token::Number)));
    }

    #[test]
    fn json_tells_a_key_from_a_value() {
        let found = kinds(r#"{"name": "copycopy", "n": 3}"#, Lang::Json);
        assert!(found.contains(&("\"name\"", Token::Key)));
        assert!(found.contains(&("\"copycopy\"", Token::Str)));
        assert!(found.contains(&("3", Token::Number)));
    }

    #[test]
    fn sql_keywords_ignore_case() {
        let upper = kinds("SELECT * FROM t", Lang::Sql);
        let lower = kinds("select * from t", Lang::Sql);
        assert!(upper.contains(&("SELECT", Token::Keyword)));
        assert!(lower.contains(&("select", Token::Keyword)));
    }

    #[test]
    fn markup_finds_tags_and_attributes() {
        let found = kinds("<a href=\"/x\">texte</a>", Lang::Html);
        assert!(found.contains(&("<a", Token::Tag)));
        assert!(found.contains(&("href", Token::Key)));
        assert!(found.contains(&("\"/x\"", Token::Str)));
        assert!(found.contains(&("</a", Token::Tag)));
        // The text between the tags stays plain: a page is mostly prose.
        assert!(!found.iter().any(|(s, _)| *s == "texte"));
    }

    #[test]
    fn yaml_keys_are_bare() {
        let found = kinds("name: copycopy\nport: 8080", Lang::Yaml);
        assert!(found.contains(&("name", Token::Key)));
        assert!(found.contains(&("8080", Token::Number)));
    }

    /// The scanner walks bytes, so this is the test that stops it handing the
    /// view a slice that starts inside a character.
    #[test]
    fn ranges_survive_accents_and_emoji() {
        for text in [
            "// éàü — un commentaire\nlet x = 1;",
            "let s = \"日本語のテキスト\"; // 🚀",
            "# 🎯 cible\nvaleur = 3.14",
        ] {
            check_invariants(text, Lang::Rust);
            check_invariants(text, Lang::Python);
        }
    }

    /// `&'a str`: the lifetime must not be read as a string running to the end
    /// of the line, which is what a naive scanner does.
    #[test]
    fn a_lifetime_is_not_a_string() {
        let found = kinds("fn go<'a>(s: &'a str) -> &'a str { s }", Lang::Rust);
        assert!(!found.iter().any(|(_, t)| *t == Token::Str));
        assert!(found.contains(&("fn", Token::Keyword)));
    }

    #[test]
    fn a_character_literal_is_still_one() {
        let found = kinds("let c = 'x'; let n = '\\n';", Lang::Rust);
        assert!(found.contains(&("'x'", Token::Str)));
        assert!(found.contains(&("'\\n'", Token::Str)));
    }

    #[test]
    fn plain_prose_is_left_alone() {
        assert!(spans("bonjour, ceci n'est pas du code", Lang::Rust).is_empty());
    }
}
