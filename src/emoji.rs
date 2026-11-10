//! Emoji shortcodes (`|smirk_cat|` → 😼), using the GitHub/gemoji shortcode list embedded in the `emojis` crate.

use std::sync::OnceLock;

pub fn find(shortcode: &str) -> Option<&'static str> {
    if shortcode.is_empty() || shortcode.chars().any(char::is_whitespace) {
        return None;
    }
    emojis::get_by_shortcode(shortcode).map(|e| e.as_str())
}

#[derive(Debug, Clone)]
pub struct EmojiEntry {
    pub emoji: &'static str,
    /// The shortcodes, the main one first.
    pub codes: Vec<&'static str>,
    pub name: &'static str,
    pub group: emojis::Group,
}

impl EmojiEntry {
    pub fn shortcode(&self) -> &'static str {
        self.codes[0]
    }
}

/// All the emojis that have at least one shortcode, in the Unicode order (computed once).
pub fn all() -> &'static [EmojiEntry] {
    static ALL: OnceLock<Vec<EmojiEntry>> = OnceLock::new();
    ALL.get_or_init(|| {
        emojis::iter()
            .filter(|e| e.shortcode().is_some())
            .map(|e| EmojiEntry { emoji: e.as_str(), codes: e.shortcodes().collect(), name: e.name(), group: e.group() })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn shortcodes() {
        assert_eq!(super::find("smirk_cat"), Some("😼"));
        assert_eq!(super::find("santa"), Some("🎅"));
        assert_eq!(super::find("not_an_emoji"), None);
        assert!(super::all().len() > 1000);
        assert_eq!(super::all().iter().find(|e| e.emoji == "😼").unwrap().shortcode(), "smirk_cat");
    }
}
