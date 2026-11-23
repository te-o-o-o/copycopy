//! Masks email addresses and secrets in what copycopy copies back. Each value
//! becomes a placeholder naming what was there, so the reader still knows the
//! shape of the text.
//!
//! Hand-written scanners rather than the regex crate: a handful of shapes, each
//! a few lines, and every boundary is an ASCII byte, so slicing stays on UTF-8
//! character boundaries.
//!
//! What counts as a secret:
//! - a PEM private key block;
//! - a token with a well-known prefix (OpenAI/Anthropic, Stripe, GitHub,
//!   GitLab, Slack, AWS, Google, Hugging Face, npm);
//! - a JWT;
//! - the value after `Bearer `;
//! - the password in `scheme://user:password@host`;
//! - the value of a `password`, `secret`, `token`, `api_key`… key.
//!
//! ponytail: shapes, not entropy — a random string with no prefix and no key
//! name in front of it goes through. Add an entropy check if that bites.

pub const EMAIL: &str = "[email]";
pub const SECRET: &str = "[secret]";

/// `text` with the chosen kinds of values replaced by their placeholder.
pub fn mask(text: &str, emails: bool, secrets: bool) -> String {
    let mut out = text.to_string();
    // Secrets first: `user:password@host.com` would otherwise read as an
    // address and leave the user name in plain sight.
    if secrets {
        out = splice(&out, secret_ranges(&out), SECRET);
    }
    if emails {
        out = splice(&out, email_ranges(&out), EMAIL);
    }
    out
}

/// Replaces each range with `with`. Ranges may overlap and come in any order.
fn splice(s: &str, mut ranges: Vec<(usize, usize)>, with: &str) -> String {
    if ranges.is_empty() {
        return s.to_string();
    }
    ranges.sort_unstable();
    let mut out = String::with_capacity(s.len());
    let mut at = 0;
    for (start, end) in ranges {
        if end <= at {
            continue;
        }
        let start = start.max(at);
        out.push_str(&s[at..start]);
        // Two overlapping ranges make one placeholder, not two.
        if !out.ends_with(with) || start > at {
            out.push_str(with);
        }
        at = end;
    }
    out.push_str(&s[at..]);
    out
}

fn token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Where the run of bytes accepted by `ok` starting at `from` ends.
fn run(b: &[u8], from: usize, ok: impl Fn(u8) -> bool) -> usize {
    from + b[from..].iter().take_while(|&&c| ok(c)).count()
}

fn starts_word(b: &[u8], i: usize) -> bool {
    i == 0 || !token_byte(b[i - 1])
}

fn email_ranges(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let local = |c: u8| c.is_ascii_alphanumeric() || b"._%+-".contains(&c);
    let domain = |c: u8| c.is_ascii_alphanumeric() || c == b'.' || c == b'-';
    let mut found = Vec::new();
    for at in s.match_indices('@').map(|(i, _)| i) {
        let start = at - b[..at].iter().rev().take_while(|&&c| local(c)).count();
        let mut end = run(b, at + 1, domain);
        // A sentence ends with a full stop, not the address.
        while end > at + 1 && matches!(b[end - 1], b'.' | b'-') {
            end -= 1;
        }
        let host = &s[at + 1..end];
        let tld = host.rsplit('.').next().unwrap_or("");
        if start < at
            && host.contains('.')
            && tld.len() >= 2
            && tld.bytes().all(|c| c.is_ascii_alphabetic())
        {
            found.push((start, end));
        }
    }
    found
}

/// Prefixes of tokens issued by well-known services, with the shortest length
/// a real one has. The length keeps `sk-learn` and the like out.
const PREFIXES: &[(&str, usize)] = &[
    ("sk-", 24),
    ("sk_live_", 24),
    ("sk_test_", 24),
    ("rk_live_", 24),
    ("ghp_", 30),
    ("gho_", 30),
    ("ghu_", 30),
    ("ghs_", 30),
    ("ghr_", 30),
    ("github_pat_", 30),
    ("glpat-", 20),
    ("xoxb-", 20),
    ("xoxp-", 20),
    ("xoxa-", 20),
    ("xoxs-", 20),
    ("AKIA", 20),
    ("ASIA", 20),
    ("AIza", 35),
    ("hf_", 30),
    ("npm_", 30),
];

/// Key names whose value is a secret, matched inside the key, lower-cased:
/// `DB_PASSWORD` and `githubToken` both count.
const KEYS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "access_key",
    "private_key",
];

fn secret_ranges(s: &str) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut found = Vec::new();
    private_keys(s, &mut found);
    for i in 0..b.len() {
        // ASCII patterns only: a byte inside a multi-byte character is
        // never a start, and slicing there would panic.
        if !b[i].is_ascii() || !starts_word(b, i) {
            continue;
        }
        let rest = &s[i..];
        for &(prefix, min) in PREFIXES {
            if rest.starts_with(prefix) {
                let end = run(b, i, token_byte);
                if end - i >= min {
                    found.push((i, end));
                }
            }
        }
        // A JWT: three base64url segments, the first one starting `eyJ`.
        if rest.starts_with("eyJ") {
            let end = run(b, i, |c| token_byte(c) || c == b'.');
            if s[i..end].matches('.').count() == 2 && end - i >= 30 {
                found.push((i, end));
            }
        }
        if rest.len() > 7 && rest.as_bytes()[..7].eq_ignore_ascii_case(b"bearer ") {
            let from = i + 7;
            let end = run(b, from, |c| token_byte(c) || b".~+/=".contains(&c));
            if end - from >= 16 {
                found.push((from, end));
            }
        }
    }
    url_passwords(s, &mut found);
    key_values(s, &mut found);
    found
}

fn private_keys(s: &str, found: &mut Vec<(usize, usize)>) {
    let mut from = 0;
    while let Some(start) = s[from..].find("-----BEGIN ").map(|i| from + i) {
        let Some(head) = s[start..].find("PRIVATE KEY-----").map(|i| start + i) else {
            break;
        };
        // A preview cut short may hold the start of a key and not its end.
        let end = s[head..]
            .find("-----END ")
            .and_then(|e| s[head + e..].find("KEY-----").map(|k| head + e + k + 8))
            .unwrap_or(s.len());
        found.push((start, end));
        from = end;
    }
}

fn url_passwords(s: &str, found: &mut Vec<(usize, usize)>) {
    for (i, _) in s.match_indices("://") {
        let auth = i + 3;
        let end = auth
            + s[auth..]
                .find(|c: char| c == '/' || c.is_whitespace())
                .unwrap_or(s.len() - auth);
        let part = &s[auth..end];
        if let Some(at) = part.rfind('@') {
            if let Some(colon) = part[..at].find(':') {
                if colon + 1 < at {
                    found.push((auth + colon + 1, auth + at));
                }
            }
        }
    }
}

fn key_values(s: &str, found: &mut Vec<(usize, usize)>) {
    let b = s.as_bytes();
    let ident = |c: u8| token_byte(c) || c == b'.';
    let mut i = 0;
    while i < b.len() {
        if !ident(b[i]) {
            i += 1;
            continue;
        }
        let end = run(b, i, ident);
        let key = s[i..end].to_ascii_lowercase();
        i = end;
        if !KEYS.iter().any(|k| key.contains(k)) {
            continue;
        }
        // `key = v`, `"key": "v"`, `key := v`, `key => v`.
        let mut j = run(b, end, |c| {
            c == b'"' || c == b'\'' || c == b' ' || c == b'\t'
        });
        if j < b.len() && b[j] == b':' {
            j += 1;
        }
        if j < b.len() && b[j] == b'=' {
            j += 1;
            if j < b.len() && b[j] == b'>' {
                j += 1;
            }
        } else if j == end || b[j - 1] != b':' {
            continue;
        }
        j = run(b, j, |c| c == b' ' || c == b'\t');
        let (from, to) = match b.get(j) {
            Some(&q @ (b'"' | b'\'' | b'`')) => {
                let to = j
                    + 1
                    + s[j + 1..]
                        .find(|c: char| c as u32 == q as u32 || c == '\n')
                        .unwrap_or(s.len() - j - 1);
                (j + 1, to)
            }
            _ => (
                j,
                j + s[j..]
                    .find(|c: char| c.is_whitespace() || ",;\"'".contains(c))
                    .unwrap_or(s.len() - j),
            ),
        };
        // Short values are counts and flags (`max_tokens: 512`), not secrets.
        if to - from >= 6 {
            found.push((from, to));
        }
        i = to.max(end);
    }
}
