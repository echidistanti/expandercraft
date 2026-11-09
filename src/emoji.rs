//! Emoji shortcodes (`|smirk_cat|` → 😼), using the GitHub/gemoji shortcode list embedded in the `emojis` crate.

pub fn find(shortcode: &str) -> Option<&'static str> {
    if shortcode.is_empty() || shortcode.chars().any(char::is_whitespace) {
        return None;
    }
    emojis::get_by_shortcode(shortcode).map(|e| e.as_str())
}

#[derive(Debug, Clone)]
pub struct EmojiEntry {
    pub shortcode: &'static str,
    pub emoji: &'static str,
    pub name: &'static str,
}

/// All the emojis that have at least one shortcode.
pub fn all() -> Vec<EmojiEntry> {
    emojis::iter().filter_map(|e| e.shortcode().map(|shortcode| EmojiEntry { shortcode, emoji: e.as_str(), name: e.name() })).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn shortcodes() {
        assert_eq!(super::find("smirk_cat"), Some("😼"));
        assert_eq!(super::find("santa"), Some("🎅"));
        assert_eq!(super::find("not_an_emoji"), None);
        assert!(super::all().len() > 1000);
    }
}
