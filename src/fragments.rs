//! Splitting of an evaluated snippet into fragments: text, key presses (`#{key:}`), shortcuts (`#{shortcut:}`)
//! and pauses (`#{delay:}`), plus handling of the `#{cursor}` variable.

use unicode_segmentation::UnicodeSegmentation;

use crate::shortcut::{Key, Shortcut};

pub const CURSOR_VARIABLE: &str = "#{cursor}";

#[derive(Debug, Clone, PartialEq)]
pub enum Fragment {
    Text(String),
    Key { key: Key, repeat: u32 },
    Shortcut(Shortcut),
    Delay(u64),
}

/// Try to read a special variable (`#{key:}`, `#{shortcut:}`, `#{delay:}`) at the start of `s`.
/// Returns the length in bytes of the variable and the fragment (None if the variable is recognized but invalid).
fn parse_special(s: &str) -> Option<(usize, Option<Fragment>)> {
    let close = s.find('}')?;
    let inner = &s[2..close];
    if inner.contains('\n') {
        return None;
    }
    let len = close + 1;
    if let Some(ms) = inner.strip_prefix("delay:") {
        if ms.is_empty() || !ms.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let ms: u64 = ms.parse().unwrap_or(0);
        return Some((len, (ms > 0).then_some(Fragment::Delay(ms))));
    }
    if let Some(param) = inner.strip_prefix("key:") {
        let (name, repeat) = match param.split_once(':') {
            Some((n, r)) => {
                if r.is_empty() || !r.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                (n, r.parse::<u32>().unwrap_or(1))
            }
            None => (param, 1),
        };
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        let fragment = Key::from_variable_name(name).map(|key| Fragment::Key { key, repeat: repeat.min(1000) });
        if fragment.is_none() {
            log::warn!("Unknown key in #{{key:}} variable: '{name}'");
        }
        return Some((len, fragment));
    }
    if let Some(param) = inner.strip_prefix("shortcut:") {
        if param.is_empty() {
            return None;
        }
        let fragment = Shortcut::parse(param).filter(Shortcut::is_valid).map(Fragment::Shortcut);
        if fragment.is_none() {
            log::warn!("Invalid shortcut in #{{shortcut:}} variable: '{param}'");
        }
        return Some((len, fragment));
    }
    None
}

fn find_cursor(text: &str, from: usize) -> Option<usize> {
    let pat = CURSOR_VARIABLE.as_bytes();
    let bytes = text.as_bytes();
    if bytes.len() < pat.len() {
        return None;
    }
    (from..=bytes.len() - pat.len()).find(|&i| text.is_char_boundary(i) && bytes[i..i + pat.len()].eq_ignore_ascii_case(pat))
}

fn remove_cursors(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    while let Some(i) = find_cursor(text, pos) {
        out.push_str(&text[pos..i]);
        pos = i + CURSOR_VARIABLE.len();
    }
    out.push_str(&text[pos..]);
    out
}

/// Remove the special variables, to count the characters that will actually be printed.
fn strip_specials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    while let Some(rel) = text[pos..].find("#{") {
        let start = pos + rel;
        out.push_str(&text[pos..start]);
        match parse_special(&text[start..]) {
            Some((len, _)) => pos = start + len,
            None => {
                out.push_str("#{");
                pos = start + 2;
            }
        }
    }
    out.push_str(&text[pos..]);
    out
}

/// Number of user-perceived characters (a `\r\n` pair, an emoji with modifiers... count as one).
pub fn printable_len(text: &str) -> usize {
    text.graphemes(true).count()
}

/// Remove the `#{cursor}` variables and compute how many times the left arrow key must be pressed after the
/// substitution to put the text cursor at the position of the first `#{cursor}`.
pub fn extract_cursor(text: &str) -> (String, Option<usize>) {
    let Some(first) = find_cursor(text, 0) else {
        return (text.to_string(), None);
    };
    let after = &text[first + CURSOR_VARIABLE.len()..];
    let shift = printable_len(&strip_specials(&remove_cursors(after)));
    (remove_cursors(text), Some(shift))
}

/// Split a snippet (already evaluated, cursor removed) into fragments.
pub fn split_fragments(text: &str) -> Vec<Fragment> {
    let mut result: Vec<Fragment> = Vec::new();
    let mut current = String::new();
    let mut pos = 0;
    while let Some(rel) = text[pos..].find("#{") {
        let start = pos + rel;
        match parse_special(&text[start..]) {
            Some((len, fragment)) => {
                current.push_str(&text[pos..start]);
                if let Some(f) = fragment {
                    if !current.is_empty() {
                        result.push(Fragment::Text(std::mem::take(&mut current)));
                    }
                    result.push(f);
                }
                pos = start + len;
            }
            None => {
                current.push_str(&text[pos..start + 2]);
                pos = start + 2;
            }
        }
    }
    current.push_str(&text[pos..]);
    if !current.is_empty() {
        result.push(Fragment::Text(current));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split() {
        let f = split_fragments("abc#{delay:500}def");
        assert_eq!(f, vec![Fragment::Text("abc".into()), Fragment::Delay(500), Fragment::Text("def".into())]);
        let f = split_fragments("1#{key:tab}2#{key:up:10}");
        assert_eq!(
            f,
            vec![
                Fragment::Text("1".into()),
                Fragment::Key { key: Key::Tab, repeat: 1 },
                Fragment::Text("2".into()),
                Fragment::Key { key: Key::Up, repeat: 10 }
            ]
        );
        let f = split_fragments("Before#{shortcut:Ctrl+Shift+J}After");
        assert_eq!(f.len(), 3);
        assert!(matches!(f[1], Fragment::Shortcut(s) if s.ctrl && s.shift && s.key == Key::Char('J')));
        assert_eq!(split_fragments("#{unknown} #{delay:x}"), vec![Fragment::Text("#{unknown} #{delay:x}".into())]);
        // recognized but invalid variables are dropped, like in Beeftext
        assert_eq!(split_fragments("a#{key:nokey}b#{delay:0}c"), vec![Fragment::Text("abc".into())]);
        assert!(split_fragments("").is_empty());
    }

    #[test]
    fn cursor() {
        assert_eq!(extract_cursor("Hello #{cursor}world"), ("Hello world".into(), Some(5)));
        assert_eq!(extract_cursor("<b>#{CURSOR}</b>#{key:tab}"), ("<b></b>#{key:tab}".into(), Some(4)));
        assert_eq!(extract_cursor("a#{cursor}b#{cursor}c"), ("abc".into(), Some(2)));
        assert_eq!(extract_cursor("no cursor"), ("no cursor".into(), None));
        assert_eq!(extract_cursor("x#{cursor}\r\n👍🏽é"), ("x\r\n👍🏽é".into(), Some(3)));
    }
}
