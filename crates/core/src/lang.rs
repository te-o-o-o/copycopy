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
    C,
    Cpp,
    Java,
    CSharp,
    Kotlin,
    Swift,
    Ruby,
    Lua,
    Markdown,
    Dockerfile,
    Diff,
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
            Lang::C => "c",
            Lang::Cpp => "c++",
            Lang::Java => "java",
            Lang::CSharp => "c#",
            Lang::Kotlin => "kotlin",
            Lang::Swift => "swift",
            Lang::Ruby => "ruby",
            Lang::Lua => "lua",
            Lang::Markdown => "markdown",
            Lang::Dockerfile => "dockerfile",
            Lang::Diff => "diff",
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
            Lang::C => "c",
            Lang::Cpp => "cpp",
            Lang::Java => "java",
            Lang::CSharp => "cs",
            Lang::Kotlin => "kt",
            Lang::Swift => "swift",
            Lang::Ruby => "rb",
            Lang::Lua => "lua",
            Lang::Markdown => "md",
            Lang::Dockerfile => "dockerfile",
            Lang::Diff => "diff",
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

/// The same, when the copy came from a tool that only ever holds code: the
/// application has already said "this is code", so less evidence settles which.
const TOOL_THRESHOLD: u32 = 3;
const TOOL_MARGIN: u32 = 2;
/// What the application suggests is worth this much, on top of a language that
/// already scored. Never on its own: PhpStorm also holds a README.
const HINTED: u32 = 3;

pub fn detect(text: &str) -> Option<Lang> {
    detect_from(text, "")
}

/// `source` is the application the copy came from, as the platform names it —
/// "PhpStorm" on macOS, "phpstorm64" on Windows. Empty when unknown.
pub fn detect_from(text: &str, source: &str) -> Option<Lang> {
    let sample = sample(text).trim();
    if sample.is_empty() {
        return None;
    }
    signature(sample)
        .or_else(|| structure(sample))
        .or_else(|| scored(sample, Hint::of(source)))
}

// --------------------------------------------------------- the application

/// What the application a copy came from says about it.
#[derive(Default)]
struct Hint {
    /// The languages it is made for.
    langs: &'static [Lang],
    /// It only ever holds code: an editor, an IDE, a database client.
    tool: bool,
}

use Lang::*;

/// Matched against the lower-cased application name. First match wins, so the
/// longer names come before the names they contain.
const TOOLS: &[(&str, &[Lang], bool)] = &[
    (
        "phpstorm",
        &[Php, JavaScript, TypeScript, Html, Css, Sql],
        true,
    ),
    ("webstorm", &[JavaScript, TypeScript, Html, Css], true),
    ("pycharm", &[Python], true),
    ("rustrover", &[Rust, Toml], true),
    ("goland", &[Go], true),
    ("clion", &[C, Cpp], true),
    ("rider", &[CSharp], true),
    ("datagrip", &[Sql], true),
    ("intellij", &[Java, Kotlin], true),
    ("idea64", &[Java, Kotlin], true),
    ("android studio", &[Kotlin, Java], true),
    ("studio64", &[Kotlin, Java], true),
    ("xcode", &[Swift], true),
    ("visual studio code", &[], true),
    ("vscode", &[], true),
    ("visual studio", &[CSharp, Cpp], true),
    ("devenv", &[CSharp, Cpp], true),
    ("sublime", &[], true),
    ("cursor", &[], true),
    ("windsurf", &[], true),
    ("neovim", &[], true),
    ("nvim", &[], true),
    ("emacs", &[], true),
    ("bbedit", &[], true),
    ("notepad++", &[], true),
    ("tableplus", &[Sql], true),
    ("dbeaver", &[Sql], true),
    ("sequel", &[Sql], true),
    ("pgadmin", &[Sql], true),
    ("beekeeper", &[Sql], true),
    ("heidisql", &[Sql], true),
    ("workbench", &[Sql], true),
    ("postman", &[Json], true),
    ("insomnia", &[Json], true),
    // Terminals hold commands, and their output too: a log, a listing, a
    // stack trace. So a nudge towards the shell, never a lower bar.
    ("terminal", &[Shell], false),
    ("iterm", &[Shell], false),
    ("ghostty", &[Shell], false),
    ("wezterm", &[Shell], false),
    ("kitty", &[Shell], false),
    ("alacritty", &[Shell], false),
    ("konsole", &[Shell], false),
    ("tilix", &[Shell], false),
];

/// Names too short to search inside another: matched whole.
const TOOLS_EXACT: &[(&str, &[Lang], bool)] = &[
    ("code", &[], true),
    ("zed", &[], true),
    ("vim", &[], true),
    ("warp", &[Shell], false),
];

impl Hint {
    fn of(source: &str) -> Hint {
        let name = source.trim().to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        TOOLS
            .iter()
            .find(|(needle, ..)| name.contains(needle))
            .or_else(|| TOOLS_EXACT.iter().find(|(needle, ..)| name == *needle))
            .map(|&(_, langs, tool)| Hint { langs, tool })
            .unwrap_or_default()
    }
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
    if diff(t) {
        return Some(Lang::Diff);
    }
    if dockerfile(t) {
        return Some(Lang::Dockerfile);
    }
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

/// A patch: a `diff --git` header, or the `---`/`+++` pair with a hunk.
fn diff(t: &str) -> bool {
    if t.starts_with("diff --git ") {
        return true;
    }
    let has = |prefix: &str| t.lines().any(|l| l.starts_with(prefix));
    has("--- ") && has("+++ ") && has("@@ ")
}

const DOCKER: &[&str] = &[
    "FROM",
    "RUN",
    "CMD",
    "LABEL",
    "EXPOSE",
    "ENV",
    "ADD",
    "COPY",
    "ENTRYPOINT",
    "VOLUME",
    "USER",
    "WORKDIR",
    "ARG",
    "ONBUILD",
    "STOPSIGNAL",
    "HEALTHCHECK",
    "SHELL",
];

/// Opens on `FROM` or `ARG`, and every line is an instruction, a comment or the
/// continuation of the one above. SQL has a `FROM` too, never on every line.
fn dockerfile(t: &str) -> bool {
    let instruction = |l: &str| {
        l.split_once(' ')
            .is_some_and(|(word, _)| DOCKER.contains(&word))
    };
    let mut lines = content_lines(t).filter(|l| !l.starts_with('#'));
    let Some(first) = lines.next() else {
        return false;
    };
    if !(first.starts_with("FROM ") || first.starts_with("ARG ")) {
        return false;
    }
    let mut instructions = 1;
    let mut continued = first.ends_with('\\');
    for line in lines {
        if instruction(line) {
            instructions += 1;
        } else if !continued {
            return false;
        }
        continued = line.ends_with('\\');
    }
    instructions >= 2
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
fn scored(t: &str, hint: Hint) -> Option<Lang> {
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
        (Lang::C, c(t)),
        (Lang::Cpp, cpp(t)),
        (Lang::Java, java(t)),
        (Lang::CSharp, csharp(t)),
        (Lang::Kotlin, kotlin(t)),
        (Lang::Swift, swift(t)),
        (Lang::Ruby, ruby(t)),
        (Lang::Lua, lua(t)),
        (Lang::Markdown, markdown(t)),
    ];
    for (lang, score) in &mut scores {
        if *score > 0 && hint.langs.contains(lang) {
            *score += HINTED;
        }
    }
    scores.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    let (best, top) = scores[0];
    let second = scores[1].1;
    let (threshold, margin) = if hint.tool {
        (TOOL_THRESHOLD, TOOL_MARGIN)
    } else {
        (THRESHOLD, MARGIN)
    };
    (top >= threshold && top >= second + margin).then_some(best)
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
        + 4 * starting(t, "if let ")
        + 2 * count(t, ".iter()")
        + 3 * count(t, ".collect()")
        + 3 * count(t, "Vec<")
        + 3 * count(t, "&self")
}

fn python(t: &str) -> u32 {
    let from_import = cap(content_lines(t)
        .filter(|l| l.starts_with("from ") && l.contains(" import "))
        .count());
    // `import os` alone on its line: no quotes, braces or semicolon, which is
    // what JavaScript, Go and Java imports all carry.
    let bare_import = cap(content_lines(t)
        .filter(|l| {
            l.strip_prefix("import ").is_some_and(|rest| {
                rest.chars()
                    .all(|c| c.is_ascii_alphanumeric() || " _.,".contains(c))
            })
        })
        .count());
    let with_as = cap(content_lines(t)
        .filter(|l| l.starts_with("with ") && l.contains(" as ") && l.ends_with(':'))
        .count());
    let comprehension = cap(t
        .match_indices('[')
        .filter(|(i, _)| {
            let rest = &t[i + 1..];
            let inner = &rest[..rest.find(']').unwrap_or(0)];
            inner.contains(" for ") && inner.contains(" in ")
        })
        .count());
    5 * block_openers(t, "def ")
        + 4 * block_openers(t, "class ")
        + 4 * bare_import
        + 5 * with_as
        + 5 * comprehension
        + 3 * count(t, "range(")
        + count(t, "print(")
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
    // `const x = …;`: JavaScript closes its statements, Rust's `let` does too,
    // Python and Kotlin never do.
    let closed = cap(content_lines(t)
        .filter(|l| (l.starts_with("const ") || l.starts_with("let ")) && l.ends_with(';'))
        .count());
    2 * count(t, "function ")
        + 3 * starting(t, "function ")
        + 2 * starting(t, "const ")
        + starting(t, "let ")
        + closed
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
    // `$user`, not `$HOME`: an all-capitals name is the shell's environment.
    let variables = cap(t
        .match_indices('$')
        .filter(|(i, _)| {
            let name: String = t[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && name.chars().any(|c| c.is_ascii_lowercase())
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
    let statement = |a: &str, b: &str| {
        words(t, a) > 0 && words(t, b) > 0
            || (guarded
                && words(t, &a.to_ascii_lowercase()) > 0
                && words(t, &b.to_ascii_lowercase()) > 0)
    };
    let statements = [
        ("INSERT INTO", "VALUES"),
        ("UPDATE", "SET"),
        ("DELETE FROM", "WHERE"),
    ]
    .iter()
    .filter(|(a, b)| statement(a, b))
    .count() as u32;
    upper + lower + if paired { 2 } else { 0 } + 3 * statements + 3 * words(t, "CREATE TABLE")
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
    "sed",
    "awk",
    "rsync",
    "gh",
    "terraform",
    "helm",
    "bun",
    "deno",
    "node",
    "python3",
    "rustup",
    "uv",
    "ps",
    "kill",
    "tail",
    "head",
    "xargs",
    "ln",
    "go",
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
    // One command on its own, with a flag, a pipe or a path: what gets copied
    // from a README. Without one of those, `cat is asleep` would qualify.
    let one_command = lines.len() == 1
        && commands == 1
        && [" -", " | ", "./", "/", "~"]
            .iter()
            .any(|m| lines[0].contains(m));
    // `$HOME`, `$PATH`: upper-case variables are the environment's.
    let env_vars = cap(t
        .match_indices('$')
        .filter(|(i, _)| {
            let name: String = t[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || *c == '_')
                .collect();
            name.len() >= 2
        })
        .count());
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
        + if one_command { 5 } else { 0 }
        + env_vars
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
    let inline_rules = cap(content_lines(t).filter(|l| css_inline_rule(l)).count());
    2 * selectors
        + 6 * inline_rules
        + declarations
        + 4 * count(t, "@media")
        + 2 * (count(t, "px;") + count(t, "rem;"))
        + 2 * hex_colours
}

fn c(t: &str) -> u32 {
    let headers = cap(content_lines(t)
        .filter(|l| l.starts_with("#include") && (l.ends_with(".h>") || l.ends_with(".h\"")))
        .count());
    5 * headers
        + 3 * count(t, "printf(")
        + 4 * count(t, "int main(")
        + 3 * count(t, "malloc(")
        + 2 * count(t, "sizeof(")
        + 2 * words(t, "NULL")
        + starting(t, "#define ")
}

fn cpp(t: &str) -> u32 {
    // `<vector>`: the standard library's headers have no extension.
    let headers = cap(content_lines(t)
        .filter(|l| l.starts_with("#include <") && !l.ends_with(".h>"))
        .count());
    5 * headers
        + 4 * count(t, "std::")
        + 3 * words(t, "cout")
        + 3 * words(t, "endl")
        + 4 * count(t, "template<")
        + 3 * words(t, "nullptr")
        + 2 * count(t, "auto& ")
        + 3 * starting(t, "using namespace ")
}

fn java(t: &str) -> u32 {
    5 * count(t, "System.out.")
        + 5 * count(t, "public static void main")
        + 5 * starting(t, "import java")
        + 3 * count(t, "String[]")
        + 4 * count(t, "@Override")
        + 3 * count(t, "private final ")
        + 2 * count(t, "public class ")
        + 3 * count(t, "new ArrayList<")
        + 2 * count(t, "throws ")
}

fn csharp(t: &str) -> u32 {
    5 * starting(t, "using System")
        + 5 * count(t, "Console.Write")
        + 5 * count(t, "static void Main")
        + 5 * count(t, "{ get; set; }")
        + 4 * count(t, "async Task")
        + 3 * count(t, "string[] ")
        + 2 * starting(t, "namespace ")
        + 2 * starting(t, "var ")
        + 2 * count(t, "foreach (")
}

fn kotlin(t: &str) -> u32 {
    4 * starting(t, "fun ")
        + 3 * count(t, " fun ")
        + 3 * starting(t, "val ")
        + 5 * starting(t, "data class ")
        + 5 * count(t, "companion object")
        + 4 * count(t, "?.let")
        + 2 * count(t, "when (")
        + 2 * count(t, "listOf(")
        + 2 * count(t, "mutableListOf(")
}

fn swift(t: &str) -> u32 {
    let funcs_returning = cap(content_lines(t)
        .filter(|l| l.contains("func ") && l.contains(") -> "))
        .count());
    let frameworks = cap(content_lines(t)
        .filter(|l| ["import UIKit", "import SwiftUI", "import Foundation"].contains(l))
        .count());
    starting(t, "func ")
        + 3 * funcs_returning
        + 5 * count(t, "guard let ")
        + 5 * frameworks
        + 5 * count(t, "some View")
        + 4 * count(t, "@State")
        + words(t, "nil")
}

fn ruby(t: &str) -> u32 {
    // `def name` with no colon: Python's would end with one.
    let defs = cap(content_lines(t)
        .filter(|l| l.starts_with("def ") && !l.ends_with(':'))
        .count());
    let ends = cap(content_lines(t).filter(|l| *l == "end").count());
    4 * defs
        + 2 * ends
        + 3 * starting(t, "puts ")
        + 3 * count(t, "#{")
        + 5 * count(t, " do |")
        + 3 * starting(t, "require '")
        + 5 * count(t, "attr_accessor")
        + 2 * words(t, "elsif")
}

fn lua(t: &str) -> u32 {
    let ends = cap(content_lines(t).filter(|l| *l == "end").count());
    5 * starting(t, "local function ")
        + 3 * starting(t, "local ")
        + ends
        + 4 * words(t, "elseif")
        + 4 * count(t, "~=")
        + 2 * count(t, " then")
        + words(t, "nil")
}

/// Shell and Python comments open with `# ` too, so a heading alone is a weak
/// clue; the fences, links and lists are what settle it.
fn markdown(t: &str) -> u32 {
    let headings = cap(content_lines(t)
        .filter(|l| {
            let hashes = l.chars().take_while(|c| *c == '#').count();
            (1..=6).contains(&hashes) && l[hashes..].starts_with(' ')
        })
        .count());
    let items = cap(content_lines(t)
        .filter(|l| l.starts_with("- ") || l.starts_with("* "))
        .count());
    3 * count(t, "```") + 2 * headings + 2 * count(t, "](") + count(t, "**") + items
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

/// `a { color: red; }` on one line: a selector, then nothing but declarations.
fn css_inline_rule(line: &str) -> bool {
    let Some((head, rest)) = line.split_once('{') else {
        return false;
    };
    let Some(body) = rest.trim().strip_suffix('}') else {
        return false;
    };
    let decls: Vec<&str> = body
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .collect();
    css_selector(&format!("{}{{", head.trim()))
        && !decls.is_empty()
        && decls.iter().all(|d| css_declaration(&format!("{d};")))
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
