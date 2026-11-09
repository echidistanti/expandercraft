//! Application preferences, stored as JSON in the data folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::library::{CaseSensitivity, MatchingMode};
use crate::shortcut::Shortcut;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    /// Follow the system language.
    #[default]
    Auto,
    English,
    Italian,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    /// Is ExpanderCraft enabled (substitutions are performed)?
    pub enabled: bool,
    /// Set after the first launch.
    pub already_launched: bool,

    // --- Behavior
    pub auto_start_at_login: bool,
    pub play_sound_on_combo: bool,
    pub use_custom_sound: bool,
    pub custom_sound_path: String,
    /// If false, combos are only triggered with the combo trigger shortcut.
    pub use_automatic_substitution: bool,
    pub combo_trigger_shortcut: Shortcut,
    /// The combo is triggered by typing a space after the keyword.
    pub combo_triggers_on_space: bool,
    /// When triggering on space, keep the space after the snippet.
    pub keep_final_space_character: bool,
    pub combo_picker_enabled: bool,
    pub combo_picker_shortcut: Shortcut,
    pub enable_app_toggle_shortcut: bool,
    pub app_toggle_shortcut: Shortcut,

    // --- Appearance
    pub theme: Theme,
    pub language: Language,

    // --- Combos
    pub default_matching_mode: MatchingMode,
    pub default_case_sensitivity: CaseSensitivity,
    pub warn_about_short_keyword: bool,
    pub warn_about_empty_keyword: bool,
    /// Custom folder for the combo list file. Empty means the data folder.
    pub combo_list_folder: String,

    // --- Emojis
    pub emoji_shortcodes_enabled: bool,
    pub emoji_left_delimiter: String,
    pub emoji_right_delimiter: String,
    pub show_emojis_in_picker: bool,

    // --- Advanced
    pub delay_between_keystrokes_ms: u64,
    pub restore_clipboard_after_substitution: bool,
    pub use_shift_insert_for_pasting: bool,
    pub auto_backup: bool,
    pub use_custom_backup_location: bool,
    pub custom_backup_location: String,
    pub use_custom_powershell: bool,
    pub custom_powershell_path: String,
    pub write_debug_log: bool,

    // --- Misc
    pub last_import_export_path: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            already_launched: false,
            auto_start_at_login: false,
            play_sound_on_combo: true,
            use_custom_sound: false,
            custom_sound_path: String::new(),
            use_automatic_substitution: true,
            combo_trigger_shortcut: Shortcut::default_trigger(),
            combo_triggers_on_space: false,
            keep_final_space_character: false,
            combo_picker_enabled: true,
            combo_picker_shortcut: Shortcut::default_picker(),
            enable_app_toggle_shortcut: false,
            app_toggle_shortcut: Shortcut::default_app_toggle(),
            theme: Theme::System,
            language: Language::Auto,
            default_matching_mode: MatchingMode::Strict,
            default_case_sensitivity: CaseSensitivity::CaseSensitive,
            warn_about_short_keyword: true,
            warn_about_empty_keyword: true,
            combo_list_folder: String::new(),
            emoji_shortcodes_enabled: false,
            emoji_left_delimiter: "|".into(),
            emoji_right_delimiter: "|".into(),
            show_emojis_in_picker: false,
            delay_between_keystrokes_ms: 12,
            restore_clipboard_after_substitution: true,
            use_shift_insert_for_pasting: false,
            auto_backup: true,
            use_custom_backup_location: false,
            custom_backup_location: String::new(),
            use_custom_powershell: false,
            custom_powershell_path: String::new(),
            write_debug_log: true,
            last_import_export_path: String::new(),
        }
    }
}

impl Preferences {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("The preferences file is invalid ({e}), using default values.");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::library::write_atomic(path, serde_json::to_string_pretty(self)?.as_bytes())
    }

    /// Reset everything except the values that describe the state of the installation.
    pub fn reset_to_defaults(&mut self) {
        let keep_launched = self.already_launched;
        let keep_folder = self.combo_list_folder.clone();
        *self = Self::default();
        self.already_launched = keep_launched;
        self.combo_list_folder = keep_folder;
    }

    pub fn combo_list_folder_path(&self, data_dir: &Path) -> PathBuf {
        if self.combo_list_folder.trim().is_empty() { data_dir.to_path_buf() } else { PathBuf::from(&self.combo_list_folder) }
    }

    pub fn triggers_on_space(&self) -> bool {
        self.use_automatic_substitution && self.combo_triggers_on_space
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_uses_defaults() {
        let p: Preferences = serde_json::from_str(r#"{"enabled": false, "comboPickerShortcut": "Ctrl+Space"}"#).unwrap();
        assert!(!p.enabled);
        assert_eq!(p.combo_picker_shortcut.to_string(), "Ctrl+Space");
        assert_eq!(p.delay_between_keystrokes_ms, 12);
        let text = serde_json::to_string(&p).unwrap();
        let p2: Preferences = serde_json::from_str(&text).unwrap();
        assert_eq!(p, p2);
    }
}
