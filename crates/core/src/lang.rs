//! Language detection for clipboard snippets.
//!
//! A clipboard carries no file extension, and syntect only recognises a
//! language from a shebang or a modeline. This guesses from the content, and
//! cheaply: a bounded sample, three passes, no dependency.
//!
//! It answers `None` whenever it is not sure. Highlighting a snippet in the
//! wrong language is worse than not highlighting it at all.

/// A language the detector can name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Php,
    Sql,
    Shell,
    Go,
    Json,
    Yaml,
    Toml,
    Html,
    Xml,
    Css,
}

impl Lang {
    /// Lower-case name, as shown in the interface.
    pub fn name(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::Python => "python",
            Lang::JavaScript => "javascript",
            Lang::TypeScript => "typescript",
            Lang::Php => "php",
            Lang::Sql => "sql",
            Lang::Shell => "shell",
            Lang::Go => "go",
            Lang::Json => "json",
            Lang::Yaml => "yaml",
            Lang::Toml => "toml",
            Lang::Html => "html",
            Lang::Xml => "xml",
            Lang::Css => "css",
        }
    }

    /// The extension syntax definitions are looked up by, for the highlighting
    /// that will build on this.
    pub fn token(self) -> &'static str {
        match self {
            Lang::Rust => "rs",
            Lang::Python => "py",
            Lang::JavaScript => "js",
            Lang::TypeScript => "ts",
            Lang::Php => "php",
            Lang::Sql => "sql",
            Lang::Shell => "sh",
            Lang::Go => "go",
            Lang::Json => "json",
            Lang::Yaml => "yaml",
            Lang::Toml => "toml",
            Lang::Html => "html",
            Lang::Xml => "xml",
            Lang::Css => "css",
        }
    }
}

/// Past this, the rest of a text settles nothing its opening has not, and a
/// megabyte of log must cost what a line costs.
const SAMPLE_BYTES: usize = 4096;
const SAMPLE_LINES: usize = 60;
/// The winning score needs this many points…
const THRESHOLD: u32 = 5;
/// …and this lead over the runner-up, or the answer is "not sure".
const MARGIN: u32 = 3;
/// A single token repeated a hundred times is one clue, not a hundred: every
/// count stops here, so no language wins on one word alone.
const CAP: usize = 4;

pub fn detect(text: &str) -> Option<Lang> {
    let sample = sample(text).trim();
    if sample.is_empty() {
        return None;
    }
    signature(sample)
        .or_else(|| structure(sample))
        .or_else(|| scored(sample))
}

fn sample(text: &str) -> &str {
    let mut end = text.len().min(SAMPLE_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let cut = &text[..end];
    match cut.match_indices('\n').nth(SAMPLE_LINES - 1) {
        Some((i, _)) => &cut[..i],
        None => cut,
    }
}

// ------------------------------------------------------------ pass 1: marks

/// Marks that settle the question on their own.
fn signature(t: &str) -> Option<Lang> {
    let first = t.lines().next().unwrap_or("").trim();
    if let Some(bang) = first.strip_prefix("#!") {
        let words: Vec<&str> = bang
            .split(|c: char| c == '/' || c.is_whitespace())
            .collect();
        let has = |name: &str| words.iter().any(|w| w.starts_with(name));
        return if has("python") {
            Some(Lang::Python)
        } else if has("node") || has("deno") || has("bun") {
            Some(Lang::JavaScript)
        } else if has("php") {
            Some(Lang::Php)
        } else if words
            .iter()
            .any(|w| matches!(*w, "sh" | "bash" | "zsh" | "fish" | "dash" | "ksh"))
        {
            Some(Lang::Shell)
        } else {
            None
        };
    }
    if t.contains("<?php") {
        return Some(Lang::Php);
    }
    if first.starts_with("<?xml") {
        return Some(Lang::Xml);
    }
    let lower = first.to_ascii_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return Some(Lang::Html);
    }
    if first.starts_with("package ") && t.contains("func ") {
        return Some(Lang::Go);
    }
    None
}

// -------------------------------------------------------- pass 2: structure

/// Data formats, recognised by their shape rather than their vocabulary.
fn structure(t: &str) -> Option<Lang> {
    if json(t) {
        return Some(Lang::Json);
    }
    if t.starts_with('<') && t.ends_with('>') && t.contains("</") {
        return Some(if known_tags(t) > 0 {
            Lang::Html
        } else {
            Lang::Xml
        });
    }
    if toml(t) {
        return Some(Lang::Toml);
    }
    if yaml(t) {
        return Some(Lang::Yaml);
    }
    None
}

/// Judged on the opening only, so a document longer than the sample, cut off
/// before its closing bracket, is still recognised.
fn json(t: &str) -> bool {
    let mut chars = t.chars();
    let open = chars.next();
    let next = chars.find(|c| !c.is_whitespace());
    match (open, next) {
        // Quoted keys are what separate JSON from a JavaScript object literal.
        (Some('{'), Some('"')) => t.contains("\":") || t.contains("\" :"),
        (Some('['), Some(c)) => matches!(c, '{' | '[' | '"' | ']' | '-') || c.is_ascii_digit(),
        _ => false,
    }
}

/// Strict on purpose: every line has to be a section, a pair, a comment or the
/// continuation of an array. One line of prose and it is not TOML.
fn toml(t: &str) -> bool {
    let mut section = false;
    let mut pairs = 0;
    for line in content_lines(t).filter(|l| !l.starts_with('#')) {
        if line.len() >= 2 && line.starts_with('[') && line.ends_with(']') {
            let inner = &line[1..line.len() - 1];
            if inner
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-.\"[] ".contains(c))
            {
                section = true;
                continue;
            }
            return false;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-.\"".contains(c))
                && !value.trim().is_empty()
            {
                pairs += 1;
                continue;
            }
            return false;
        }
        let continuation = line.starts_with(|c: char| c.is_ascii_digit() || "\"'-]}".contains(c))
            || line.ends_with(',')
            || line.ends_with('[');
        if !continuation {
            return false;
        }
    }
    section && pairs >= 1
}

/// Every line a `key: value`, a list item or an indented continuation, and
/// enough of them. Two `key: value` lines alone are an e-mail header as much
/// as YAML, so it takes three, or two with nesting, or a `---` document start.
fn yaml(t: &str) -> bool {
    let mut keys = 0;
    let mut nested = false;
    let mut opened = false;
    for line in t
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
    {
        let body = line.trim_start();
        let indented = body.len() < line.len();
        if body == "---" {
            continue;
        }
        if body.ends_with(';') || body.ends_with('{') || body.ends_with('}') {
            return false;
        }
        if opened && indented {
            nested = true;
        }
        opened = false;
        let item = body.strip_prefix("- ").unwrap_or(body);
        if let Some((key, rest)) = item.split_once(':') {
            let key_ok = !key.is_empty()
                && !key.contains("  ")
                && key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-.\"' ".contains(c));
            if key_ok && (rest.is_empty() || rest.starts_with(' ')) {
                keys += 1;
                opened = rest.trim().is_empty();
                continue;
            }
        }
        if body.starts_with("- ") || indented {
            continue;
        }
        return false;
    }
    keys >= 3 || (keys >= 2 && nested) || (t.starts_with("---") && keys >= 1)
}

// ---------------------------------------------------------- pass 3: scores

/// Programming languages, recognised by vocabulary. Each clue is weighted by
/// how specific it is — `$this->` names PHP, `let` names nothing much.
fn scored(t: &str) -> Option<Lang> {
    let js = javascript(t);
    let ts_only = typescript_only(t);
    let mut scores = [
        (Lang::Rust, rust(t)),
        (Lang::Python, python(t)),
        (Lang::JavaScript, js),
        // TypeScript is JavaScript plus types: it only competes once the types
        // are there, and then it wins over plain JavaScript by their weight.
        (
            Lang::TypeScript,
            if ts_only >= 4 { js + ts_only } else { 0 },
        ),
        (Lang::Php, php(t)),
        (Lang::Sql, sql(t)),
        (Lang::Shell, shell(t)),
        (Lang::Go, go(t)),
        (Lang::Css, css(t)),
        (Lang::Html, 2 * known_tags(t) + count(t, "</")),
    ];
    scores.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    let (best, top) = scores[0];
    let second = scores[1].1;
    (top >= THRESHOLD && top >= second + MARGIN).then_some(best)
}

fn rust(t: &str) -> u32 {
    3 * starting(t, "fn ")
        + 3 * count(t, "pub fn ")
        + 2 * starting(t, "let ")
        + 3 * count(t, "let mut ")
        + 3 * starting(t, "impl")
        + 3 * starting(t, "use std::")
        + starting(t, "use ")
        + count(t, "::")
        + 3 * count(t, "&str")
        + 2 * count(t, "Option<")
        + 2 * count(t, "Result<")
        + 3 * count(t, "println!")
        + 4 * count(t, "#[derive")
        + 3 * count(t, ".unwrap()")
        + 3 * starting(t, "pub struct")
        + 2 * count(t, "Some(")
        + 2 * starting(t, "match ")
        + 2 * count(t, "&mut ")
}

fn python(t: &str) -> u32 {
    let from_import = cap(content_lines(t)
        .filter(|l| l.starts_with("from ") && l.contains(" import "))
        .count());
    4 * block_openers(t, "def ")
        + 4 * block_openers(t, "class ")
        + 3 * from_import
        + starting(t, "import ")
        + 2 * count(t, "self.")
        + 3 * starting(t, "elif ")
        + 4 * count(t, "__init__")
        + 2 * block_openers(t, "if ")
        + 2 * block_openers(t, "for ")
        + 2 * block_openers(t, "while ")
        + 3 * words(t, "lambda")
        + words(t, "None")
        + words(t, "True")
        + words(t, "False")
}

fn javascript(t: &str) -> u32 {
    2 * count(t, "function ")
        + 2 * starting(t, "const ")
        + starting(t, "let ")
        + 2 * count(t, "=>")
        + 3 * count(t, "===")
        + 3 * count(t, "!==")
        + 4 * count(t, "console.log")
        + 3 * count(t, "require(")
        + 2 * starting(t, "export ")
        + 3 * count(t, "document.")
        + 2 * words(t, "async")
        + words(t, "await")
        + 3 * words(t, "undefined")
        + 2 * (count(t, " from '") + count(t, " from \""))
}

fn typescript_only(t: &str) -> u32 {
    4 * count(t, ": string")
        + 4 * count(t, ": number")
        + 4 * count(t, ": boolean")
        + 3 * starting(t, "interface ")
        + 2 * count(t, ": void")
        + 2 * starting(t, "type ")
        + 2 * count(t, "export interface")
}

fn php(t: &str) -> u32 {
    let variables = cap(t
        .match_indices('$')
        .filter(|(i, _)| {
            t[i + 1..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        })
        .count());
    let use_namespaced = cap(content_lines(t)
        .filter(|l| l.starts_with("use ") && l.contains('\\'))
        .count());
    // `->` is Rust's return arrow too: it only counts where variables are sigiled.
    let arrows = if variables > 0 { count(t, "->") } else { 0 };
    5 * count(t, "$this->")
        + 5 * count(t, "public function")
        + 3 * count(t, "private function")
        + 3 * count(t, "protected function")
        + 3 * starting(t, "namespace ")
        + 4 * use_namespaced
        + 2 * words(t, "echo")
        + 3 * count(t, "array(")
        + variables
        + arrows
        + 2 * count(t, "::class")
}

const SQL_UPPER: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "ORDER BY",
    "GROUP BY",
    "INSERT INTO",
    "UPDATE",
    "DELETE FROM",
    "CREATE TABLE",
    "ALTER TABLE",
    "VALUES",
    "LIMIT",
];
const SQL_LOWER: &[&str] = &[
    "select",
    "from",
    "where",
    "join",
    "order by",
    "group by",
    "insert into",
    "update",
    "delete from",
    "create table",
    "alter table",
    "values",
    "limit",
];

/// Upper-case keywords count on their own. Lower-case ones are ordinary English
/// words — "select the file from the list where…" — so they only count next to
/// something no sentence has: a `;`, a `*` or an ` = `.
fn sql(t: &str) -> u32 {
    let upper: u32 = SQL_UPPER.iter().map(|k| 2 * words(t, k)).sum();
    let guarded = t.contains(';') || t.contains('*') || t.contains(" = ");
    let lower: u32 = if guarded {
        SQL_LOWER.iter().map(|k| words(t, k)).sum()
    } else {
        0
    };
    let paired = (words(t, "SELECT") > 0 && words(t, "FROM") > 0)
        || (guarded && words(t, "select") > 0 && words(t, "from") > 0);
    upper + lower + if paired { 2 } else { 0 }
}

const SHELL_COMMANDS: &[&str] = &[
    "cd",
    "ls",
    "mkdir",
    "rm",
    "cp",
    "mv",
    "chmod",
    "chown",
    "grep",
    "curl",
    "wget",
    "git",
    "npm",
    "npx",
    "yarn",
    "pnpm",
    "cargo",
    "docker",
    "kubectl",
    "ssh",
    "scp",
    "tar",
    "cat",
    "echo",
    "apt",
    "apt-get",
    "brew",
    "pip",
    "make",
    "systemctl",
    "source",
];

fn shell(t: &str) -> u32 {
    let lines: Vec<&str> = content_lines(t).collect();
    let exact = |word: &str| cap(lines.iter().filter(|l| **l == word).count());
    let commands = cap(lines
        .iter()
        .filter(|l| {
            l.split_whitespace()
                .next()
                .is_some_and(|w| SHELL_COMMANDS.contains(&w))
        })
        .count());
    let thens = cap(lines
        .iter()
        .filter(|l| l.ends_with("; then") || **l == "then")
        .count());
    let dos = cap(lines.iter().filter(|l| l.ends_with("; do")).count());
    let exports = cap(lines
        .iter()
        .filter(|l| l.starts_with("export ") && l.contains('='))
        .count());
    3 * exact("fi")
        + 3 * exact("done")
        + 2 * thens
        + 2 * dos
        + 2 * exports
        + 3 * starting(t, "sudo ")
        + 2 * commands
        + 2 * count(t, "${")
        + count(t, " | ")
        + count(t, " && ")
        + 2 * starting(t, "$ ")
}

fn go(t: &str) -> u32 {
    3 * starting(t, "func ")
        + 3 * count(t, ":=")
        + 4 * starting(t, "package ")
        + 4 * count(t, "fmt.")
        + 4 * count(t, "import (")
        + 4 * count(t, "go func")
        + 2 * count(t, "chan ")
        + 3 * starting(t, "defer ")
        + 5 * count(t, "err != nil")
}

fn css(t: &str) -> u32 {
    let selectors = cap(content_lines(t).filter(|l| css_selector(l)).count());
    let declarations = cap(content_lines(t).filter(|l| css_declaration(l)).count());
    let hex_colours = cap(t
        .match_indices('#')
        .filter(|(i, _)| {
            let rest = &t[i + 1..];
            let digits = rest.chars().take_while(|c| c.is_ascii_hexdigit()).count();
            matches!(digits, 3 | 6) && rest[digits..].starts_with(';')
        })
        .count());
    2 * selectors
        + declarations
        + 4 * count(t, "@media")
        + 2 * (count(t, "px;") + count(t, "rem;"))
        + 2 * hex_colours
}

const HTML_TAGS: &[&str] = &[
    "html", "head", "body", "div", "span", "p", "a", "ul", "ol", "li", "table", "tr", "td", "th",
    "img", "br", "h1", "h2", "h3", "h4", "h5", "h6", "button", "input", "form", "header", "footer",
    "nav", "section", "main", "article", "script", "style", "meta", "link", "label", "select",
    "option", "textarea",
];

/// `name: value;` — a CSS declaration, or a TypeScript field, which scores its
/// own way on top.
fn css_declaration(line: &str) -> bool {
    let Some(body) = line.strip_suffix(';') else {
        return false;
    };
    let Some((property, value)) = body.split_once(':') else {
        return false;
    };
    let property = property.trim();
    !property.is_empty()
        && property.chars().all(|c| c.is_ascii_lowercase() || c == '-')
        && !value.trim().is_empty()
}

/// A line opening a rule. Not any line ending in `{`: `impl Foo {` or
/// `interface User {` end that way too, so it has to start like a selector —
/// a class, an id, a pseudo-class, an attribute, or an HTML tag.
fn css_selector(line: &str) -> bool {
    let Some(head) = line.strip_suffix('{') else {
        return false;
    };
    let head = head.trim();
    if head.is_empty() || head.contains('(') || head.contains('=') {
        return false;
    }
    if head.starts_with(|c: char| ".#*:[".contains(c)) {
        return true;
    }
    let word: String = head
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    HTML_TAGS.contains(&word.as_str())
}

/// Opening tags of known HTML elements, read after each `<`.
fn known_tags(t: &str) -> u32 {
    cap(t
        .match_indices('<')
        .filter(|(i, _)| {
            let name: String = t[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            HTML_TAGS.contains(&name.to_ascii_lowercase().as_str())
        })
        .count())
}

// ---------------------------------------------------------------- helpers

fn cap(n: usize) -> u32 {
    n.min(CAP) as u32
}

fn content_lines(t: &str) -> impl Iterator<Item = &str> {
    t.lines().map(str::trim).filter(|l| !l.is_empty())
}

fn count(t: &str, needle: &str) -> u32 {
    cap(t.matches(needle).count())
}

fn starting(t: &str, prefix: &str) -> u32 {
    cap(content_lines(t).filter(|l| l.starts_with(prefix)).count())
}

/// Lines that start with `prefix` and open a block with a trailing colon, as
/// Python's `def`, `class`, `if` and `for` do.
fn block_openers(t: &str, prefix: &str) -> u32 {
    cap(content_lines(t)
        .filter(|l| l.starts_with(prefix) && l.ends_with(':'))
        .count())
}

/// Occurrences of `word` standing as a whole word, not inside a longer one.
fn words(t: &str, word: &str) -> u32 {
    let bytes = t.as_bytes();
    let boundary = |b: Option<&u8>| b.is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_'));
    cap(t
        .match_indices(word)
        .filter(|(i, _)| {
            let before = i.checked_sub(1).and_then(|j| bytes.get(j));
            boundary(before) && boundary(bytes.get(i + word.len()))
        })
        .count())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is(lang: Lang, text: &str) {
        assert_eq!(detect(text), Some(lang), "should read as {lang:?}:\n{text}");
    }

    fn unsure(text: &str) {
        assert_eq!(detect(text), None, "should not claim a language:\n{text}");
    }

    #[test]
    fn rust() {
        is(
            Lang::Rust,
            "use std::collections::HashMap;\n\nfn count(words: &[&str]) -> HashMap<&str, usize> {\n    let mut map = HashMap::new();\n    for w in words {\n        *map.entry(*w).or_insert(0) += 1;\n    }\n    map\n}",
        );
        is(
            Lang::Rust,
            "fn main() {\n    let watcher = copycopy_platform::start(None)?;\n}",
        );
    }

    #[test]
    fn python() {
        is(
            Lang::Python,
            "import os\n\nclass Config:\n    def __init__(self, path):\n        self.path = path\n\n    def load(self):\n        if os.path.exists(self.path):\n            return open(self.path).read()\n        return None",
        );
        is(Lang::Python, "#!/usr/bin/env python3\nprint('hi')");
    }

    #[test]
    fn javascript_and_typescript() {
        is(
            Lang::JavaScript,
            "const button = document.querySelector('#save');\nbutton.addEventListener('click', async () => {\n  const res = await fetch('/api/save');\n  if (res.status === 200) console.log('saved');\n});",
        );
        is(
            Lang::TypeScript,
            "interface User {\n  id: number;\n  name: string;\n}\n\nexport function greet(user: User): string {\n  return `Hello ${user.name}`;\n}",
        );
    }

    #[test]
    fn php() {
        is(
            Lang::Php,
            "namespace App\\Http\\Controllers;\n\nuse App\\Models\\User;\n\nclass UserController extends Controller\n{\n    public function show(int $id)\n    {\n        $user = User::findOrFail($id);\n        return view('user.show', ['user' => $user]);\n    }\n}",
        );
        is(Lang::Php, "<?php\necho 'hello';");
    }

    #[test]
    fn sql() {
        is(
            Lang::Sql,
            "SELECT u.id, u.name\nFROM users u\nJOIN orders o ON o.user_id = u.id\nWHERE o.total > 100\nORDER BY u.name;",
        );
        is(Lang::Sql, "select * from users where id = 1;");
    }

    #[test]
    fn shell() {
        is(
            Lang::Shell,
            "if [ -f .env ]; then\n  export $(cat .env | xargs)\nfi\ncargo build --release && ./target/release/app",
        );
        is(Lang::Shell, "#!/bin/bash\nset -e");
    }

    #[test]
    fn go() {
        is(
            Lang::Go,
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tx := 42\n\tfmt.Println(x)\n}",
        );
    }

    #[test]
    fn data_formats() {
        is(
            Lang::Json,
            "{\"name\": \"copycopy\", \"version\": \"0.1.0\"}",
        );
        is(Lang::Json, "[{\"id\": 1}, {\"id\": 2}]");
        is(
            Lang::Yaml,
            "services:\n  web:\n    image: nginx\n    ports:\n      - \"80:80\"",
        );
        is(
            Lang::Toml,
            "[package]\nname = \"copycopy\"\nversion = \"0.1.0\"\n\n[dependencies]\niced = { version = \"0.14\", features = [\"canvas\"] }",
        );
    }

    #[test]
    fn markup_and_style() {
        is(Lang::Html, "<div class=\"card\">\n  <p>Hello</p>\n</div>");
        is(
            Lang::Html,
            "<html><body><!--StartFragment--><img class=\"resize\" src=\"https://picsum.photos/536/354\">",
        );
        is(
            Lang::Xml,
            "<?xml version=\"1.0\"?>\n<note><to>Tove</to></note>",
        );
        is(Lang::Xml, "<note><to>Tove</to></note>");
        is(
            Lang::Css,
            ".card {\n  color: #333;\n  padding: 12px;\n}\n\n@media (max-width: 600px) {\n  .card { padding: 4px; }\n}",
        );
    }

    #[test]
    fn prose_is_left_alone() {
        unsure("Bonjour tout le monde");
        unsure("j'ai -> une flèche");
        unsure("Please select the file from the list where you saved it.");
        unsure("Sélectionnez un fichier : puis validez.");
        unsure("Rendez-vous demain à 10h, n'oublie pas le code 1234.");
        unsure("Subject: Hello\nFrom: Theo");
        unsure("https://github.com/iced-rs/iced/blob/master/core/src/text.rs#L181");
    }

    #[test]
    fn a_huge_text_costs_its_sample_only() {
        let log = "2026-09-13 INFO request served\n".repeat(100_000);
        assert!(sample(&log).len() <= SAMPLE_BYTES);
        unsure(&log);
    }
}
