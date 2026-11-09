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

    /// The names accepted by the `#{key:<name>}` variable (case-insensitive), as in Beeftext.
    pub fn from_variable_name(name: &str) -> Option<Key> {
        let n = name.to_lowercase();
        let k = match n.as_str() {
            "space" => Key::Space,
            "tab" => Key::Tab,
            "enter" | "return" => Key::Enter,
            "insert" => Key::Insert,
            "delete" => Key::Delete,
            "backspace" => Key::Backspace,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" => Key::PageUp,
            "pagedown" => Key::PageDown,
            "up" => Key::Up,
            "down" => Key::Down,
            "left" => Key::Left,
            "right" => Key::Right,
            "escape" | "esc" => Key::Escape,
            "printscreen" => Key::PrintScreen,
            "pause" => Key::Pause,
            "numlock" => Key::NumLock,
            "volumemute" => Key::VolumeMute,
            "volumeup" => Key::VolumeUp,
            "volumedown" => Key::VolumeDown,
            "medianexttrack" => Key::MediaNext,
            "mediaprevioustrack" => Key::MediaPrevious,
            "mediastop" => Key::MediaStop,
            "mediaplaypause" => Key::MediaPlayPause,
            "mediaselect" => Key::MediaSelect,
            "windows" | "meta" | "command" | "super" => Key::Meta,
            "control" | "ctrl" => Key::Control,
            "alt" => Key::Alt,
            "shift" => Key::Shift,
            _ => {
                if let Some(num) = n.strip_prefix('f')
                    && let Ok(i) = num.parse::<u8>()
                    && (1..=24).contains(&i)
                {
                    return Some(Key::F(i));
                }
                return None;
            }
        };
        Some(k)
    }

    /// The list of names shown in the "Insert variable" menu.
    pub const VARIABLE_NAMES: &'static [&'static str] = &[
        "space",
        "tab",
        "enter",
        "backspace",
        "insert",
        "delete",
        "home",
        "end",
        "pageUp",
        "pageDown",
        "up",
        "down",
        "left",
        "right",
        "escape",
        "printScreen",
        "pause",
        "numLock",
        "volumeMute",
        "volumeUp",
        "volumeDown",
        "mediaNextTrack",
        "mediaPreviousTrack",
        "mediaStop",
        "mediaPlayPause",
        "mediaSelect",
        "windows",
        "control",
        "alt",
        "shift",
        "f1",
        "f2",
        "f3",
        "f4",
        "f5",
        "f6",
        "f7",
        "f8",
        "f9",
        "f10",
        "f11",
        "f12",
    ];

    pub fn display_name(self) -> String {
        match self {
            Key::Char(c) => c.to_uppercase().to_string(),
            Key::F(n) => format!("F{n}"),
            Key::Enter => "Enter".into(),
            Key::Space => "Space".into(),
            Key::Tab => "Tab".into(),
            Key::Backspace => "Backspace".into(),
            Key::Escape => "Esc".into(),
            Key::Insert => "Ins".into(),
            Key::Delete => "Del".into(),
            Key::Home => "Home".into(),
            Key::End => "End".into(),
            Key::PageUp => "PgUp".into(),
            Key::PageDown => "PgDown".into(),
            Key::Left => "Left".into(),
            Key::Right => "Right".into(),
            Key::Up => "Up".into(),
            Key::Down => "Down".into(),
            Key::PrintScreen => "Print".into(),
            Key::Pause => "Pause".into(),
            Key::NumLock => "NumLock".into(),
            Key::CapsLock => "CapsLock".into(),
            Key::ScrollLock => "ScrollLock".into(),
            Key::VolumeMute => "VolumeMute".into(),
            Key::VolumeUp => "VolumeUp".into(),
            Key::VolumeDown => "VolumeDown".into(),
            Key::MediaNext => "MediaNext".into(),
            Key::MediaPrevious => "MediaPrevious".into(),
            Key::MediaStop => "MediaStop".into(),
            Key::MediaPlayPause => "MediaPlay".into(),
            Key::MediaSelect => "MediaSelect".into(),
            Key::Meta => meta_name().into(),
            Key::Control => "Ctrl".into(),
            Key::Alt => "Alt".into(),
            Key::Shift => "Shift".into(),
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

    pub fn from_display_name(s: &str) -> Option<Key> {
        let l = s.trim().to_lowercase();
        let k = match l.as_str() {
            "enter" | "return" => Key::Enter,
            "space" => Key::Space,
            "tab" => Key::Tab,
            "backspace" => Key::Backspace,
            "esc" | "escape" => Key::Escape,
            "ins" | "insert" => Key::Insert,
            "del" | "delete" => Key::Delete,
            "home" => Key::Home,
            "end" => Key::End,
            "pgup" | "pageup" => Key::PageUp,
            "pgdown" | "pgdn" | "pagedown" => Key::PageDown,
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "print" | "printscreen" => Key::PrintScreen,
            "pause" => Key::Pause,
            "numlock" => Key::NumLock,
            "scrolllock" => Key::ScrollLock,
            "volumemute" => Key::VolumeMute,
            "volumeup" => Key::VolumeUp,
            "volumedown" => Key::VolumeDown,
            "medianext" => Key::MediaNext,
            "mediaprevious" => Key::MediaPrevious,
            "mediastop" => Key::MediaStop,
            "mediaplay" => Key::MediaPlayPause,
            "mediaselect" => Key::MediaSelect,
            _ => {
                if let Some(num) = l.strip_prefix('f')
                    && let Ok(i) = num.parse::<u8>()
                    && (1..=24).contains(&i)
                {
                    return Some(Key::F(i));
                }
                let mut chars = s.trim().chars();
                return match (chars.next(), chars.next()) {
                    (Some(c), None) => Some(Key::Char(c.to_uppercase().next().unwrap_or(c))),
                    _ => None,
                };
            }
        };
        Some(k)
    }
}

fn meta_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else if cfg!(windows) {
        "Win"
    } else {
        "Super"
    }
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
            write!(f, "{}+", meta_name())?;
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
    }

    #[test]
    fn variable_names() {
        assert_eq!(Key::from_variable_name("pageUp"), Some(Key::PageUp));
        assert_eq!(Key::from_variable_name("F12"), Some(Key::F(12)));
        assert_eq!(Key::from_variable_name("f25"), None);
        assert_eq!(Key::from_variable_name("insert"), Some(Key::Insert));
        for n in Key::VARIABLE_NAMES {
            assert!(Key::from_variable_name(n).is_some(), "{n}");
        }
    }
}
