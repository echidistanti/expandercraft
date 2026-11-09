//! Lists of applications (excluded, "sensitive", emoji-excluded), matched by process name with wildcards.

use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessList {
    pub patterns: Vec<String>,
}

impl ProcessList {
    pub fn new(patterns: &[&str]) -> Self {
        Self { patterns: patterns.iter().map(|s| s.to_string()).collect() }
    }

    /// Load the list from a JSON array of strings. If the file does not exist, `defaults` is used.
    pub fn load(path: &Path, defaults: &[&str]) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Vec<String>>(&text) {
                Ok(patterns) => Self { patterns },
                Err(e) => {
                    log::warn!("Invalid application list file '{}': {e}", path.display());
                    Self::new(defaults)
                }
            },
            Err(_) => Self::new(defaults),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::library::write_atomic(path, serde_json::to_string_pretty(&self.patterns)?.as_bytes())
    }

    /// Case-insensitive match of the process name against the patterns (`*` and `?` wildcards).
    pub fn matches(&self, process_name: &str) -> bool {
        if process_name.is_empty() {
            return false;
        }
        let name: Vec<char> = process_name.to_lowercase().chars().collect();
        self.patterns.iter().any(|p| {
            let p = p.trim();
            !p.is_empty() && wildcard_match(&p.to_lowercase().chars().collect::<Vec<_>>(), &name)
        })
    }
}

fn wildcard_match(pattern: &[char], text: &[char]) -> bool {
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

/// Default "sensitive" applications: they do not support pasting with Ctrl+V, so text is typed instead.
pub const DEFAULT_SENSITIVE_APPS: &[&str] = &["mintty.exe", "putty.exe", "powershell.exe", "kitty*.exe", "ConEmu*.exe"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards() {
        let l = ProcessList::new(DEFAULT_SENSITIVE_APPS);
        assert!(l.matches("PuTTY.exe"));
        assert!(l.matches("kitty_portable.exe"));
        assert!(l.matches("ConEmu64.exe"));
        assert!(!l.matches("notepad.exe"));
        assert!(!l.matches(""));
        let l = ProcessList::new(&["note?ad.exe", "*code*"]);
        assert!(l.matches("notepad.exe"));
        assert!(l.matches("VSCode.exe"));
        assert!(!l.matches("notpad.exe"));
    }
}
