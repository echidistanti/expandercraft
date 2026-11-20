//! The preferences window. Changes are applied immediately, like in Beeftext.

use egui::RichText;

use super::dialogs::{AppListEditor, AppListKind, Confirm, ConfirmAction, Dialog};
use super::style::{self, muted};
use super::{ExpanderApp, RecordTarget};
use crate::i18n::tr;
use crate::library::{CaseSensitivity, MatchingMode};
use crate::prefs::{Language, Preferences, Theme};
use crate::shortcut::Shortcut;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrefsTab {
    #[default]
    Behavior,
    Appearance,
    Combos,
    Emojis,
    Advanced,
}

#[derive(Default)]
pub struct PrefsWindow {
    pub tab: PrefsTab,
}

enum Extra {
    Record(RecordTarget),
    AppList(AppListKind),
    EmojiList,
    ResetPrefs,
    ChangeComboFolder,
    ResetComboFolder,
    OpenLog,
}

impl ExpanderApp {
    pub(super) fn prefs_ui(&mut self, ctx: &egui::Context) {
        let Some(mut window) = self.prefs.take() else { return };
        let mut open = true;
        let before = self.shared.prefs.read().clone();
        let mut p = before.clone();
        let mut extras: Vec<Extra> = Vec::new();
        let recording = self.recording;
        let portable = self.shared.paths.portable;
        let data_dir = self.shared.paths.data_dir.clone();

        egui::Window::new(tr("prefs-title"))
            .id(egui::Id::unique("prefs_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(600.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(600.0);
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (PrefsTab::Behavior, tr("prefs-tab-behavior")),
                        (PrefsTab::Appearance, tr("prefs-tab-appearance")),
                        (PrefsTab::Combos, tr("prefs-tab-combos")),
                        (PrefsTab::Emojis, tr("prefs-tab-emojis")),
                        (PrefsTab::Advanced, tr("prefs-tab-advanced")),
                    ] {
                        ui.selectable_value(&mut window.tab, tab, label);
                    }
                });
                ui.separator();
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(470.0).auto_shrink([false, true]).show(ui, |ui| match window.tab {
                    PrefsTab::Behavior => behavior_tab(ui, &mut p, recording, portable, &mut extras),
                    PrefsTab::Appearance => appearance_tab(ui, &mut p),
                    PrefsTab::Combos => combos_tab(ui, &mut p, &data_dir, &mut extras),
                    PrefsTab::Emojis => emojis_tab(ui, &mut p, &mut extras),
                    PrefsTab::Advanced => advanced_tab(ui, &mut p, &mut extras),
                });
            });

        if p != before {
            self.apply_prefs_change(&before, p);
        }
        for e in extras {
            match e {
                Extra::Record(t) => {
                    if self.recording == Some(t) {
                        self.stop_recording();
                    } else {
                        self.start_recording(t);
                    }
                }
                Extra::AppList(kind) => self.dialog = Some(Dialog::AppList(AppListEditor::new(&self.shared, kind))),
                Extra::EmojiList => self.dialog = Some(Dialog::EmojiList(Default::default())),
                Extra::ResetPrefs => {
                    self.dialog =
                        Some(Dialog::Confirm(Confirm::new(tr("prefs-confirm-reset").to_string(), ConfirmAction::ResetPreferences)))
                }
                Extra::ChangeComboFolder => {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        self.move_combo_list(dir.display().to_string());
                    }
                }
                Extra::ResetComboFolder => self.move_combo_list(String::new()),
                Extra::OpenLog => {
                    if let Some(p) = crate::logger::log_path() {
                        crate::platform::open_path(&p);
                    }
                }
            }
        }
        if open {
            self.prefs = Some(window);
        } else if self.recording.is_some() && self.recording != Some(RecordTarget::SnippetVariable) {
            self.stop_recording();
        }
    }

    fn apply_prefs_change(&mut self, before: &Preferences, mut p: Preferences) {
        if p.auto_start_at_login != before.auto_start_at_login
            && let Err(e) = crate::platform::set_autostart(p.auto_start_at_login)
        {
            p.auto_start_at_login = before.auto_start_at_login;
            self.error(format!("{}\n{e:#}", tr("msg-autostart-failed")));
        }
        if p.language != before.language {
            crate::i18n::set_language(p.language);
        }
        if p.write_debug_log != before.write_debug_log {
            crate::logger::set_file_logging(p.write_debug_log);
        }
        *self.shared.prefs.write() = p;
        self.commit_prefs();
        let _ = self.shared.engine_tx.send(crate::engine::EngineMsg::ResetBuffer);
    }

    fn move_combo_list(&mut self, folder: String) {
        let old_path = self.shared.library_path();
        self.shared.prefs.write().combo_list_folder = folder;
        self.commit_prefs();
        let new_path = self.shared.library_path();
        if new_path == old_path {
            return;
        }
        if new_path.exists() {
            // a combo list already exists in the new folder: use it
            self.replace_library_from(&new_path.clone());
        } else {
            self.commit_library();
            self.info(tr!("msg-combo-list-moved", path = new_path.display().to_string()));
        }
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).strong().color(style::accent(ui)));
    ui.add_space(2.0);
}

fn shortcut_button(
    ui: &mut egui::Ui,
    sc: &mut Shortcut,
    default: Shortcut,
    target: RecordTarget,
    recording: Option<RecordTarget>,
    extras: &mut Vec<Extra>,
) {
    let label = if recording == Some(target) { tr("prefs-recording").to_string() } else { sc.localized() };
    let b = egui::Button::new(RichText::new(label).monospace()).min_size(egui::vec2(220.0, 0.0)).selected(recording == Some(target));
    if ui.add(b).on_hover_text(tr("prefs-record-hint")).clicked() {
        extras.push(Extra::Record(target));
    }
    if *sc != default && ui.small_button(tr("app-reset")).clicked() {
        *sc = default;
    }
}

fn file_field(ui: &mut egui::Ui, value: &mut String, enabled: bool, folder: bool, filter: Option<(&str, &[&str])>) {
    ui.add_enabled(enabled, egui::TextEdit::singleline(value).desired_width(340.0));
    if ui.add_enabled(enabled, egui::Button::new(tr("app-browse"))).clicked() {
        let mut d = rfd::FileDialog::new();
        if let Some((name, ext)) = filter {
            d = d.add_filter(name, ext);
        }
        let picked = if folder { d.pick_folder() } else { d.pick_file() };
        if let Some(p) = picked {
            *value = p.display().to_string();
        }
    }
}

fn behavior_tab(ui: &mut egui::Ui, p: &mut Preferences, recording: Option<RecordTarget>, portable: bool, extras: &mut Vec<Extra>) {
    section(ui, tr("prefs-section-general"));
    ui.checkbox(&mut p.enabled, tr("prefs-enabled"));
    ui.add_enabled(!portable, egui::Checkbox::new(&mut p.auto_start_at_login, tr("prefs-autostart")));
    if portable {
        ui.label(muted(ui, tr("prefs-portable-unavailable")).small());
    }
    ui.checkbox(&mut p.play_sound_on_combo, tr("prefs-play-sound"));
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.add_enabled(p.play_sound_on_combo, egui::Checkbox::new(&mut p.use_custom_sound, tr("prefs-custom-sound")));
        let enabled = p.play_sound_on_combo && p.use_custom_sound;
        file_field(ui, &mut p.custom_sound_path, enabled, false, Some(("WAV", &["wav"])));
    });

    section(ui, tr("prefs-section-substitution"));
    ui.radio_value(&mut p.use_automatic_substitution, true, tr("prefs-automatic"));
    ui.radio_value(&mut p.use_automatic_substitution, false, tr("prefs-manual"));
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.add_enabled_ui(!p.use_automatic_substitution, |ui| {
            ui.label(tr("prefs-trigger-shortcut"));
            shortcut_button(ui, &mut p.combo_trigger_shortcut, Shortcut::default_trigger(), RecordTarget::Trigger, recording, extras);
        });
    });
    ui.add_enabled(p.use_automatic_substitution, egui::Checkbox::new(&mut p.combo_triggers_on_space, tr("prefs-trigger-on-space")));
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.add_enabled(
            p.use_automatic_substitution && p.combo_triggers_on_space,
            egui::Checkbox::new(&mut p.keep_final_space_character, tr("prefs-keep-space")),
        );
    });

    section(ui, tr("prefs-section-picker"));
    ui.checkbox(&mut p.combo_picker_enabled, tr("prefs-enable-picker"));
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.add_enabled_ui(p.combo_picker_enabled, |ui| {
            ui.label(tr("prefs-shortcut"));
            shortcut_button(ui, &mut p.combo_picker_shortcut, Shortcut::default_picker(), RecordTarget::Picker, recording, extras);
        });
    });

    section(ui, tr("prefs-section-toggle"));
    ui.checkbox(&mut p.enable_app_toggle_shortcut, tr("prefs-use-toggle"));
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        ui.add_enabled_ui(p.enable_app_toggle_shortcut, |ui| {
            ui.label(tr("prefs-shortcut"));
            shortcut_button(ui, &mut p.app_toggle_shortcut, Shortcut::default_app_toggle(), RecordTarget::AppToggle, recording, extras);
        });
    });
}

/// Languages are shown in their own language, so that the user can find theirs.
fn language_label(l: Language) -> String {
    match l {
        Language::Fixed(lang) => lang.native_name().to_string(),
        Language::System => tr!("prefs-system-language", language = crate::i18n::system_language().native_name()),
    }
}

fn appearance_tab(ui: &mut egui::Ui, p: &mut Preferences) {
    section(ui, tr("prefs-section-language"));
    egui::ComboBox::from_id_salt("pref_language").selected_text(language_label(p.language)).width(220.0).show_ui(ui, |ui| {
        for l in std::iter::once(Language::System).chain(crate::i18n::Lang::ALL.map(Language::Fixed)) {
            ui.selectable_value(&mut p.language, l, language_label(l));
        }
    });
    section(ui, tr("prefs-section-theme"));
    ui.horizontal(|ui| {
        ui.radio_value(&mut p.theme, Theme::System, tr("prefs-theme-system"));
        ui.radio_value(&mut p.theme, Theme::Light, tr("prefs-theme-light"));
        ui.radio_value(&mut p.theme, Theme::Dark, tr("prefs-theme-dark"));
    });
}

fn combos_tab(ui: &mut egui::Ui, p: &mut Preferences, data_dir: &std::path::Path, extras: &mut Vec<Extra>) {
    section(ui, tr("prefs-section-defaults"));
    egui::Grid::new("combo_defaults").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(tr("action-matching-mode"));
        egui::ComboBox::from_id_salt("pref_matching")
            .selected_text(if p.default_matching_mode == MatchingMode::Loose { tr("matching-loose") } else { tr("matching-strict") })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut p.default_matching_mode, MatchingMode::Strict, tr("matching-strict"));
                ui.selectable_value(&mut p.default_matching_mode, MatchingMode::Loose, tr("matching-loose"));
            });
        ui.end_row();
        ui.label(tr("action-case-sensitivity"));
        egui::ComboBox::from_id_salt("pref_case")
            .selected_text(if p.default_case_sensitivity == CaseSensitivity::CaseInsensitive {
                tr("case-insensitive")
            } else {
                tr("case-sensitive")
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut p.default_case_sensitivity, CaseSensitivity::CaseSensitive, tr("case-sensitive"));
                ui.selectable_value(&mut p.default_case_sensitivity, CaseSensitivity::CaseInsensitive, tr("case-insensitive"));
            });
        ui.end_row();
    });
    ui.label(muted(ui, tr("prefs-matching-help")).small());

    section(ui, tr("prefs-section-warnings"));
    ui.checkbox(&mut p.warn_about_short_keyword, tr("prefs-warn-short"));
    ui.checkbox(&mut p.warn_about_empty_keyword, tr("prefs-warn-empty"));

    section(ui, tr("prefs-section-combo-file"));
    let folder = p.combo_list_folder_path(data_dir);
    ui.label(RichText::new(folder.join(crate::library::LIBRARY_FILE_NAME).display().to_string()).monospace().small());
    ui.horizontal(|ui| {
        if ui.button(tr("prefs-change-folder")).clicked() {
            extras.push(Extra::ChangeComboFolder);
        }
        if !p.combo_list_folder.is_empty() && ui.button(tr("prefs-default-folder")).clicked() {
            extras.push(Extra::ResetComboFolder);
        }
    });
    ui.label(muted(ui, tr("prefs-sync-folder-hint")).small());
}

fn emojis_tab(ui: &mut egui::Ui, p: &mut Preferences, extras: &mut Vec<Extra>) {
    section(ui, tr("prefs-section-emoji"));
    ui.checkbox(&mut p.emoji_shortcodes_enabled, tr("prefs-emoji-enable"));
    ui.add_enabled_ui(p.emoji_shortcodes_enabled, |ui| {
        ui.horizontal(|ui| {
            ui.label(tr("prefs-left-delimiter"));
            ui.add(egui::TextEdit::singleline(&mut p.emoji_left_delimiter).desired_width(50.0).font(egui::TextStyle::Monospace));
            ui.label(tr("prefs-right-delimiter"));
            ui.add(egui::TextEdit::singleline(&mut p.emoji_right_delimiter).desired_width(50.0).font(egui::TextStyle::Monospace));
        });
        p.emoji_left_delimiter.retain(|c| !c.is_whitespace());
        p.emoji_right_delimiter.retain(|c| !c.is_whitespace());
        ui.label(muted(
            ui,
            format!(
                "{} {}smile{} → 😄   {}santa{} → 🎅",
                tr("prefs-example"),
                p.emoji_left_delimiter,
                p.emoji_right_delimiter,
                p.emoji_left_delimiter,
                p.emoji_right_delimiter
            ),
        ));
        if ui.button(tr("prefs-excluded-apps")).clicked() {
            extras.push(Extra::AppList(AppListKind::EmojiExcluded));
        }
    });
    // usable even before enabling the substitution, to see which codes exist
    ui.add_space(4.0);
    if ui.button(tr("prefs-emoji-list")).clicked() {
        extras.push(Extra::EmojiList);
    }
    section(ui, tr("prefs-section-picker"));
    ui.checkbox(&mut p.show_emojis_in_picker, tr("prefs-show-emojis-in-picker"));
}

fn advanced_tab(ui: &mut egui::Ui, p: &mut Preferences, extras: &mut Vec<Extra>) {
    section(ui, tr("prefs-section-text-insertion"));
    ui.checkbox(&mut p.restore_clipboard_after_substitution, tr("prefs-restore-clipboard"));
    ui.checkbox(&mut p.use_shift_insert_for_pasting, tr("prefs-shift-insert"));
    ui.horizontal(|ui| {
        ui.label(tr("prefs-keystroke-delay"));
        ui.add(egui::DragValue::new(&mut p.delay_between_keystrokes_ms).range(0..=500));
    });
    ui.horizontal(|ui| {
        if ui.button(tr("prefs-excluded-apps")).clicked() {
            extras.push(Extra::AppList(AppListKind::Excluded));
        }
        if ui.button(tr("prefs-sensitive-apps")).clicked() {
            extras.push(Extra::AppList(AppListKind::Sensitive));
        }
    });

    section(ui, tr("prefs-section-backup"));
    ui.checkbox(&mut p.auto_backup, tr("prefs-auto-backup"));
    ui.horizontal(|ui| {
        ui.add_enabled(p.auto_backup, egui::Checkbox::new(&mut p.use_custom_backup_location, tr("prefs-custom-location")));
        let enabled = p.auto_backup && p.use_custom_backup_location;
        file_field(ui, &mut p.custom_backup_location, enabled, true, None);
    });

    section(ui, "PowerShell");
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.use_custom_powershell, tr("prefs-custom-executable"));
        let enabled = p.use_custom_powershell;
        file_field(ui, &mut p.custom_powershell_path, enabled, false, None);
    });
    ui.label(muted(ui, tr("prefs-powershell-hint")).small());

    section(ui, tr("prefs-section-diagnostics"));
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.write_debug_log, tr("prefs-write-log"));
        if ui.button(tr("menu-open-log")).clicked() {
            extras.push(Extra::OpenLog);
        }
    });
    ui.add_space(10.0);
    if ui.button(RichText::new(tr("prefs-reset")).color(style::DANGER)).clicked() {
        extras.push(Extra::ResetPrefs);
    }
}
