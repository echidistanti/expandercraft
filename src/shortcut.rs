//! Keyboard shortcuts (`Ctrl+Alt+Shift+Enter`) and the keys usable in `#{key:}` / `#{shortcut:}` variables.

use std::fmt;

/// A platform-independent key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A character key. Letters are stored in uppercase.
    Char(char),
    /// F1 to F24.
    F(u8),
    Enter,
    Space,
    Tab,
    Backspace,
    Escape,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
    Up,
    Down,
    PrintScreen,
    Pause,
    NumLock,
    CapsLock,
    ScrollLock,
    VolumeMute,
    VolumeUp,
    VolumeDown,
    MediaNext,
    MediaPrevious,
    MediaStop,
    MediaPlayPause,
    MediaSelect,
    /// The Windows / Command / Super key.
    Meta,
    Control,
    Alt,
    Shift,
}

impl Key {
    pub fn is_modifier(self) -> bool {
        matches!(self, Key::Meta | Key::Control | Key::Alt | Key::Shift | Key::CapsLock)
    }

    /// The key named in a `#{key:<name>}` variable (case-insensitive), as in Beeftext.
    pub fn from_variable_name(name: &str) -> Option<Key> {
        let n = name.to_lowercase();
        f_key(&n).or_else(|| named_key(&n, true))
    }

    /// The names shown in the "Insert variable" menu.
    pub fn variable_names() -> impl Iterator<Item = String> {
        NAMED_KEYS.iter().filter(|e| !e.2.is_empty()).map(|e| e.2.to_string()).chain((1..=12).map(|i| format!("f{i}")))
    }

    pub fn display_name(self) -> String {
        match self {
            Key::Char(c) => c.to_uppercase().to_string(),
            Key::F(n) => format!("F{n}"),
            k => NAMED_KEYS.iter().find(|e| e.0 == k).map(|e| e.1).unwrap_or_default().into(),
        }
    }

    /// The name of the key in the language of the user interface (the English name is used in variables and files).
    pub fn localized_name(self) -> String {
        let id = match self {
            Key::Shift => "key-shift",
            Key::Enter => "key-enter",
            Key::Space => "key-space",
            Key::Backspace => "key-backspace",
            Key::Escape => "key-escape",
            Key::Insert => "key-insert",
            Key::Delete => "key-delete",
            Key::Home => "key-home",
            Key::End => "key-end",
            Key::PageUp => "key-page-up",
            Key::PageDown => "key-page-down",
            Key::Left => "key-left",
            Key::Right => "key-right",
            Key::Up => "key-up",
            Key::Down => "key-down",
            Key::PrintScreen => "key-print",
            _ => return self.display_name(),
        };
        crate::i18n::tr(id).to_string()
    }

    /// The key of a shortcut written in the settings file or in a `#{shortcut:}` variable (case-insensitive).
    pub fn from_display_name(s: &str) -> Option<Key> {
        let l = s.trim().to_lowercase();
        if let Some(k) = f_key(&l).or_else(|| named_key(&l, false)) {
            return Some(k);
        }
        let mut chars = s.trim().chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => Some(Key::Char(c.to_uppercase().next().unwrap_or(c))),
            _ => None,
        }
    }
}

const META_NAME: &str = "Win";

/// Every key with a name: (key, name shown and written in the settings, name in `#{key:}` variables, other
/// accepted names). All the names are accepted in both places, compared in lowercase. The menu lists the keys
/// in this order.
const NAMED_KEYS: &[(Key, &str, &str, &[&str])] = &[
    (Key::Space, "Space", "space", &[]),
    (Key::Tab, "Tab", "tab", &[]),
    (Key::Enter, "Enter", "enter", &["return"]),
    (Key::Backspace, "Backspace", "backspace", &[]),
    (Key::Insert, "Ins", "insert", &[]),
    (Key::Delete, "Del", "delete", &[]),
    (Key::Home, "Home", "home", &[]),
    (Key::End, "End", "end", &[]),
    (Key::PageUp, "PgUp", "pageUp", &[]),
    (Key::PageDown, "PgDown", "pageDown", &["pgdn"]),
    (Key::Up, "Up", "up", &[]),
    (Key::Down, "Down", "down", &[]),
    (Key::Left, "Left", "left", &[]),
    (Key::Right, "Right", "right", &[]),
    (Key::Escape, "Esc", "escape", &[]),
    (Key::PrintScreen, "Print", "printScreen", &[]),
    (Key::Pause, "Pause", "pause", &[]),
    (Key::NumLock, "NumLock", "numLock", &[]),
    (Key::CapsLock, "CapsLock", "", &[]),
    (Key::ScrollLock, "ScrollLock", "", &[]),
    (Key::VolumeMute, "VolumeMute", "volumeMute", &[]),
    (Key::VolumeUp, "VolumeUp", "volumeUp", &[]),
    (Key::VolumeDown, "VolumeDown", "volumeDown", &[]),
    (Key::MediaNext, "MediaNext", "mediaNextTrack", &[]),
    (Key::MediaPrevious, "MediaPrevious", "mediaPreviousTrack", &[]),
    (Key::MediaStop, "MediaStop", "mediaStop", &[]),
    (Key::MediaPlayPause, "MediaPlay", "mediaPlayPause", &[]),
    (Key::MediaSelect, "MediaSelect", "mediaSelect", &[]),
    (Key::Meta, META_NAME, "windows", &["meta", "command", "super"]),
    (Key::Control, "Ctrl", "control", &["ctrl"]),
    (Key::Alt, "Alt", "alt", &[]),
    (Key::Shift, "Shift", "shift", &[]),
];

/// `name` must be in lowercase. A shortcut cannot have a modifier as its key: `allow_modifiers` is false for them.
fn named_key(name: &str, allow_modifiers: bool) -> Option<Key> {
    NAMED_KEYS
        .iter()
        .filter(|e| allow_modifiers || !e.0.is_modifier())
        .find(|(_, shown, variable, others)| {
            shown.eq_ignore_ascii_case(name) || variable.eq_ignore_ascii_case(name) || others.contains(&name)
        })
        .map(|e| e.0)
}

/// F1 to F24, from a lowercase name.
fn f_key(name: &str) -> Option<Key> {
    let i: u8 = name.strip_prefix('f')?.parse().ok()?;
    (1..=24).contains(&i).then_some(Key::F(i))
}

/// A keyboard shortcut. A valid shortcut has at least one of Ctrl, Alt or Meta, and a non-modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: Key,
}

impl Shortcut {
    pub const fn new(ctrl: bool, alt: bool, shift: bool, meta: bool, key: Key) -> Self {
        Self { ctrl, alt, shift, meta, key }
    }

    pub fn is_valid(&self) -> bool {
        (self.ctrl || self.alt || self.meta) && !self.key.is_modifier()
    }

    pub fn default_picker() -> Self {
        Self::new(true, true, true, false, Key::Enter)
    }

    pub fn default_trigger() -> Self {
        Self::new(true, true, true, false, Key::Char('B'))
    }

    pub fn default_app_toggle() -> Self {
        Self::new(true, true, true, false, Key::Char('V'))
    }

    /// Parse a shortcut such as `Ctrl+Shift+J`, `Ctrl++` or `Win+Alt+F5`. Case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        // split on '+', but a trailing "++" (or a lone "+") means the '+' key
        let mut tokens: Vec<String> = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '+' {
                if current.trim().is_empty() {
                    // "+" used as the key
                    tokens.push("+".into());
                    current.clear();
                    i += 1;
                    if i < chars.len() && chars[i] == '+' {
                        i += 1;
                    }
                    continue;
                }
                tokens.push(std::mem::take(&mut current));
            } else {
                current.push(c);
            }
            i += 1;
        }
        if !current.trim().is_empty() {
            tokens.push(current);
        }
        let (key_token, mods) = tokens.split_last()?;
        let mut sc = Shortcut::new(false, false, false, false, Key::from_display_name(key_token)?);
        for m in mods {
            match m.trim().to_lowercase().as_str() {
                "ctrl" | "control" | "ctl" => sc.ctrl = true,
                "alt" | "option" | "opt" => sc.alt = true,
                "shift" => sc.shift = true,
                "meta" | "win" | "windows" | "super" | "cmd" | "command" => sc.meta = true,
                _ => return None,
            }
        }
        Some(sc)
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.meta {
            write!(f, "{META_NAME}+")?;
        }
        f.write_str(&self.key.display_name())
    }
}

impl Shortcut {
    /// The shortcut as shown to the user, with key names in the language of the user interface.
    pub fn localized(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.ctrl {
            parts.push(Key::Control.localized_name());
        }
        if self.alt {
            parts.push(Key::Alt.localized_name());
        }
        if self.shift {
            parts.push(Key::Shift.localized_name());
        }
        if self.meta {
            parts.push(Key::Meta.localized_name());
        }
        parts.push(self.key.localized_name());
        parts.join("+")
    }
}

impl serde::Serialize for Shortcut {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Shortcut {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Shortcut::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid shortcut '{s}'")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format() {
        let s = Shortcut::parse("Ctrl+Alt+Shift+Enter").unwrap();
        assert_eq!(s, Shortcut::default_picker());
        assert_eq!(s.to_string(), "Ctrl+Alt+Shift+Enter");
        let s = Shortcut::parse("ctrl+shift+j").unwrap();
        assert_eq!(s.key, Key::Char('J'));
        assert!(s.ctrl && s.shift && !s.alt);
        let s = Shortcut::parse("Ctrl++").unwrap();
        assert_eq!(s.key, Key::Char('+'));
        let s = Shortcut::parse("Win+F5").unwrap();
        assert!(s.meta);
        assert_eq!(s.key, Key::F(5));
        assert!(Shortcut::parse("Ctrl+Foo").is_none());
        assert!(Shortcut::parse("Hyper+A").is_none());
        assert!(!Shortcut::parse("Shift+A").unwrap().is_valid());
        // what is written is read back, for every key
        for key in NAMED_KEYS.iter().map(|e| e.0).filter(|k| !k.is_modifier()).chain([Key::F(24), Key::Char('+')]) {
            let sc = Shortcut::new(true, false, false, false, key);
            assert_eq!(Shortcut::parse(&sc.to_string()), Some(sc), "{sc}");
        }
    }

    #[test]
    fn variable_names() {
        assert_eq!(Key::from_variable_name("pageUp"), Some(Key::PageUp));
        assert_eq!(Key::from_variable_name("F12"), Some(Key::F(12)));
        assert_eq!(Key::from_variable_name("f25"), None);
        assert_eq!(Key::from_variable_name("insert"), Some(Key::Insert));
        for n in Key::variable_names() {
            assert!(Key::from_variable_name(&n).is_some(), "{n}");
        }
        assert_eq!(Key::variable_names().count(), 42);
        // the names of the other place are accepted too, but a modifier is never the key of a shortcut
        assert_eq!(Key::from_variable_name("pgup"), Some(Key::PageUp));
        assert_eq!(Key::from_display_name("mediaPlayPause"), Some(Key::MediaPlayPause));
        assert_eq!(Key::from_variable_name("command"), Some(Key::Meta));
        assert_eq!(Key::from_display_name("ctrl"), None);
        assert!(Shortcut::parse("Ctrl+CapsLock").is_none());
    }
}
