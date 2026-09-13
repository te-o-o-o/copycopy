//! Web links inside copied text.
//!
//! Only `http://` and `https://` are ever recognised. Clipboard content is not
//! trusted: `file:`, `javascript:`, `ms-settings:` or an application's own
//! scheme can open a local file or start a program on a single click, so they
//! are never turned into something clickable.

use std::ops::Range;

/// Byte ranges of the web links found in `text`, in order.
///
/// A link runs from its scheme to the first whitespace or quote. Trailing
/// punctuation that belongs to the sentence rather than the address — a full
/// stop, a comma, a closing parenthesis with no opening one inside the link —
/// is left out, so "see https://example.com." does not link the dot.
pub fn find(text: &str) -> Vec<Range<usize>> {
    let mut links = Vec::new();
    let mut from = 0;
    while let Some(offset) = next_scheme(&text[from..]) {
        let start = from + offset;
        let mut end = start
            + text[start..]
                .find(|c: char| c.is_whitespace() || "<>\"'`".contains(c))
                .unwrap_or(text.len() - start);
        end = start + trimmed_len(&text[start..end]);
        if has_host(&text[start..end]) {
            links.push(start..end);
        }
        from = end.max(start + 1);
    }
    links
}

/// Whether `link` may be handed to the system to open. Checked again right
/// before opening, whatever produced the string.
pub fn is_openable(link: &str) -> bool {
    (link.starts_with("http://") || link.starts_with("https://"))
        && !link.chars().any(|c| c.is_whitespace() || c.is_control())
        && has_host(link)
}

/// Where the next `http://` or `https://` begins, when it starts a word: in
/// "xhttps://", the scheme is part of something else.
fn next_scheme(text: &str) -> Option<usize> {
    let mut from = 0;
    loop {
        let found = text[from..].find("http")?;
        let at = from + found;
        let rest = &text[at..];
        let starts_word = text[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if starts_word && (rest.starts_with("http://") || rest.starts_with("https://")) {
            return Some(at);
        }
        from = at + 4;
    }
}

/// Length of `candidate` once the sentence's punctuation is peeled off its end.
fn trimmed_len(candidate: &str) -> usize {
    let mut link = candidate;
    loop {
        let Some(last) = link.chars().next_back() else {
            return 0;
        };
        let drop = match last {
            '.' | ',' | ';' | ':' | '!' | '?' => true,
            // A closing bracket stays when the link opened one itself, as in
            // https://en.wikipedia.org/wiki/Rust_(programming_language).
            ')' => link.matches('(').count() < link.matches(')').count(),
            ']' => link.matches('[').count() < link.matches(']').count(),
            '}' => link.matches('{').count() < link.matches('}').count(),
            _ => false,
        };
        if !drop {
            return link.len();
        }
        link = &link[..link.len() - last.len_utf8()];
    }
}

/// A scheme alone, `https://`, is not a link.
fn has_host(link: &str) -> bool {
    let after = link
        .strip_prefix("https://")
        .or_else(|| link.strip_prefix("http://"))
        .unwrap_or("");
    after
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() || c == '[')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<&str> {
        find(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn a_bare_url() {
        assert_eq!(
            found("https://github.com/iced-rs/iced"),
            ["https://github.com/iced-rs/iced"]
        );
    }

    #[test]
    fn links_inside_a_sentence() {
        assert_eq!(
            found("Voir https://a.example/x et http://b.example/y, merci."),
            ["https://a.example/x", "http://b.example/y"]
        );
    }

    #[test]
    fn sentence_punctuation_is_left_out() {
        assert_eq!(
            found("Lien : https://example.com."),
            ["https://example.com"]
        );
        assert_eq!(found("(voir https://example.com)"), ["https://example.com"]);
        assert_eq!(
            found("[doc](https://example.com/doc)"),
            ["https://example.com/doc"]
        );
    }

    #[test]
    fn brackets_the_link_opened_itself_are_kept() {
        assert_eq!(
            found("https://en.wikipedia.org/wiki/Rust_(programming_language)"),
            ["https://en.wikipedia.org/wiki/Rust_(programming_language)"]
        );
    }

    #[test]
    fn query_strings_and_fragments_survive() {
        assert_eq!(
            found("https://example.com/search?q=a&lang=fr#top"),
            ["https://example.com/search?q=a&lang=fr#top"]
        );
    }

    #[test]
    fn other_schemes_are_never_links() {
        assert!(found("file:///etc/passwd").is_empty());
        assert!(found("javascript:alert(1)").is_empty());
        assert!(found("ms-settings:privacy").is_empty());
        assert!(found("ftp://example.com").is_empty());
        assert!(found("xhttps://example.com").is_empty());
        assert!(found("https://").is_empty());
    }

    #[test]
    fn quotes_end_a_link() {
        assert_eq!(
            found("href=\"https://example.com/a\">"),
            ["https://example.com/a"]
        );
    }

    #[test]
    fn only_web_links_may_be_opened() {
        assert!(is_openable("https://example.com"));
        assert!(!is_openable("file:///C:/Windows/System32/calc.exe"));
        assert!(!is_openable("https://example.com/a b"));
        assert!(!is_openable("https://example.com/\u{7}"));
        assert!(!is_openable("https://"));
    }
}
