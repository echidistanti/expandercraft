//! State shared between the input monitor, the substitution engine and the user interface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::NaiveDateTime;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::{Mutex, RwLock};
use uuid::Uuid;

use crate::engine::EngineMsg;
use crate::i18n::tr;
use crate::library::{self, Library};
use crate::paths::AppPaths;
use crate::platform::{InputEvent, InputSink};
use crate::prefs::Preferences;
use crate::process_list::{DEFAULT_SENSITIVE_APPS, ProcessList};
use crate::shortcut::Shortcut;

/// Messages sent to the user interface.
pub enum UiMsg {
    ShowMainWindow,
    ShowPicker,
    NewCombo,
    ShowPreferences,
    PromptInput { description: String, reply: Sender<Option<String>> },
    ShortcutRecorded(Shortcut),
    EnabledChanged,
    LibraryChanged,
    Quit,
}

/// Sends messages to the UI and wakes it up (the UI may be hidden in the system tray).
pub struct UiBridge {
    tx: Sender<UiMsg>,
    ctx: OnceLock<egui::Context>,
}

impl UiBridge {
    pub fn send(&self, msg: UiMsg) {
        let _ = self.tx.send(msg);
        self.wake();
    }

    pub fn wake(&self) {
        if let Some(ctx) = self.ctx.get() {
            ctx.request_repaint();
        }
    }

    pub fn set_context(&self, ctx: egui::Context) {
        let _ = self.ctx.set(ctx);
    }
}

pub struct AppShared {
    pub paths: AppPaths,
    pub library: RwLock<Library>,
    pub prefs: RwLock<Preferences>,
    pub excluded_apps: RwLock<ProcessList>,
    pub sensitive_apps: RwLock<ProcessList>,
    pub emoji_excluded_apps: RwLock<ProcessList>,
    pub emoji_last_use: Mutex<HashMap<String, NaiveDateTime>>,
    /// Errors that occurred while loading, displayed by the UI at startup.
    pub startup_errors: Mutex<Vec<String>>,

    // fast-path flags read by the input hook
    enabled: AtomicBool,
    space_trigger: AtomicBool,
    pub substituting: AtomicBool,
    suspended_until: Mutex<Option<Instant>>,
    pub recording_shortcut: AtomicBool,
    hook_shortcuts: RwLock<Vec<Shortcut>>,

    pub engine_tx: Sender<EngineMsg>,
    pub ui: UiBridge,
}

impl AppShared {
    pub fn new(paths: AppPaths, prefs: Preferences, engine_tx: Sender<EngineMsg>) -> (Self, Receiver<UiMsg>) {
        let (ui_tx, ui_rx) = crossbeam_channel::unbounded();
        let shared = Self {
            excluded_apps: RwLock::new(ProcessList::load(&paths.excluded_apps_file(), &[])),
            sensitive_apps: RwLock::new(ProcessList::load(&paths.sensitive_apps_file(), DEFAULT_SENSITIVE_APPS)),
            emoji_excluded_apps: RwLock::new(ProcessList::load(&paths.emoji_excluded_apps_file(), &[])),
            emoji_last_use: Mutex::new(load_emoji_last_use(&paths.emoji_last_use_file())),
            paths,
            library: RwLock::new(Library::default()),
            prefs: RwLock::new(prefs),
            startup_errors: Mutex::new(Vec::new()),
            enabled: AtomicBool::new(true),
            space_trigger: AtomicBool::new(false),
            substituting: AtomicBool::new(false),
            suspended_until: Mutex::new(None),
            recording_shortcut: AtomicBool::new(false),
            hook_shortcuts: RwLock::new(Vec::new()),
            engine_tx,
            ui: UiBridge { tx: ui_tx, ctx: OnceLock::new() },
        };
        shared.refresh_hook_config();
        (shared, ui_rx)
    }

    /// Must be called after every change of the preferences.
    pub fn refresh_hook_config(&self) {
        let p = self.prefs.read();
        self.enabled.store(p.enabled, Ordering::Relaxed);
        self.space_trigger.store(p.triggers_on_space(), Ordering::Relaxed);
        let mut list = Vec::new();
        if p.enable_app_toggle_shortcut && p.app_toggle_shortcut.is_valid() {
            list.push(p.app_toggle_shortcut);
        }
        if p.enabled {
            if p.combo_picker_enabled && p.combo_picker_shortcut.is_valid() {
                list.push(p.combo_picker_shortcut);
            }
            if !p.use_automatic_substitution && p.combo_trigger_shortcut.is_valid() {
                list.push(p.combo_trigger_shortcut);
            }
        }
        *self.hook_shortcuts.write() = list;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.prefs.write().enabled = enabled;
        self.refresh_hook_config();
        self.save_prefs();
        let _ = self.engine_tx.send(EngineMsg::ResetBuffer);
        self.ui.send(UiMsg::EnabledChanged);
    }

    pub fn begin_substitution(&self) {
        self.substituting.store(true, Ordering::SeqCst);
    }

    pub fn end_substitution(&self) {
        let grace = crate::platform::post_substitution_grace();
        if !grace.is_zero() {
            *self.suspended_until.lock() = Some(Instant::now() + grace);
        }
        self.substituting.store(false, Ordering::SeqCst);
    }

    // ---- persistence ------------------------------------------------------------------------------------------------

    pub fn library_path(&self) -> PathBuf {
        self.prefs.read().combo_list_folder_path(&self.paths.data_dir).join(library::LIBRARY_FILE_NAME)
    }

    pub fn backup_dir(&self) -> PathBuf {
        let p = self.prefs.read();
        if p.use_custom_backup_location && !p.custom_backup_location.trim().is_empty() {
            PathBuf::from(&p.custom_backup_location)
        } else {
            self.paths.default_backup_dir()
        }
    }

    /// Load the library from disk (called at startup).
    pub fn load_library(&self) {
        let path = self.library_path();
        let mut lib = Library::default();
        let mut needs_save = false;
        if path.exists() {
            match Library::load(&path) {
                Ok(parsed) => {
                    needs_save = parsed.older_format;
                    if parsed.skipped > 0 {
                        self.startup_errors.lock().push(tr!("msg-invalid-combos-ignored", count = parsed.skipped));
                    }
                    lib = parsed.library;
                }
                Err(e) => {
                    log::error!("Could not load the combo list: {e:#}");
                    // keep a copy of the unreadable file, so that it is not overwritten
                    let broken = path.with_extension(format!("broken-{}.json", chrono::Local::now().format("%Y%m%d%H%M%S")));
                    let _ = std::fs::copy(&path, &broken);
                    self.startup_errors.lock().push(format!(
                        "{}\n{e:#}\n{}",
                        tr("msg-library-load-failed"),
                        tr!("msg-library-copy-saved", path = broken.display().to_string())
                    ));
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(self.paths.last_use_file()) {
            lib.apply_last_use_json(&text);
        }
        needs_save |= lib.ensure_correct_grouping(tr("app-default-group"));
        needs_save |= lib.localize_default_group_names(&crate::i18n::translations("app-default-group"), tr("app-default-group"));
        *self.library.write() = lib;
        if needs_save && let Err(e) = self.save_library() {
            log::warn!("Could not save the combo list after loading it: {e:#}");
        }
    }

    /// After a change of language, rename the groups that still have the default name of the previous one.
    pub fn localize_default_group_names(&self) {
        let renamed =
            self.library.write().localize_default_group_names(&crate::i18n::translations("app-default-group"), tr("app-default-group"));
        if renamed && let Err(e) = self.save_library() {
            log::warn!("Could not save the combo list after renaming the default group: {e:#}");
        }
    }

    /// Save the library (with an automatic backup of the previous version if enabled).
    pub fn save_library(&self) -> anyhow::Result<()> {
        let path = self.library_path();
        if self.prefs.read().auto_backup {
            crate::backup::archive(&path, &self.backup_dir());
        }
        let lib = self.library.read();
        lib.save(&path, true)?;
        let _ = library::write_atomic(&self.paths.last_use_file(), lib.last_use_json().as_bytes());
        Ok(())
    }

    pub fn save_last_use(&self) {
        let text = self.library.read().last_use_json();
        if let Err(e) = library::write_atomic(&self.paths.last_use_file(), text.as_bytes()) {
            log::warn!("Could not save the last use file: {e:#}");
        }
    }

    pub fn mark_combo_used(&self, uuid: &Uuid) {
        if let Some(c) = self.library.write().combo_mut(uuid) {
            c.last_use = Some(library::now());
        }
        self.save_last_use();
        self.ui.send(UiMsg::LibraryChanged);
    }

    pub fn mark_emoji_used(&self, shortcode: &str) {
        let mut map = self.emoji_last_use.lock();
        map.insert(shortcode.to_string(), library::now());
        let items: HashMap<&String, String> = map.iter().map(|(k, v)| (k, library::format_json_date(v))).collect();
        if let Ok(text) = serde_json::to_string_pretty(&items) {
            let _ = library::write_atomic(&self.paths.emoji_last_use_file(), text.as_bytes());
        }
    }

    pub fn save_prefs(&self) {
        if let Err(e) = self.prefs.read().save(&self.paths.settings_file()) {
            log::error!("Could not save the preferences: {e:#}");
        }
    }

    pub fn save_app_lists(&self) {
        let r = [
            self.excluded_apps.read().save(&self.paths.excluded_apps_file()),
            self.sensitive_apps.read().save(&self.paths.sensitive_apps_file()),
            self.emoji_excluded_apps.read().save(&self.paths.emoji_excluded_apps_file()),
        ];
        for e in r.into_iter().filter_map(Result::err) {
            log::error!("Could not save an application list: {e:#}");
        }
    }
}

fn load_emoji_last_use(path: &std::path::Path) -> HashMap<String, NaiveDateTime> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<HashMap<String, String>>(&t).ok())
        .map(|m| m.into_iter().filter_map(|(k, v)| library::parse_json_date(&v).map(|d| (k, d))).collect())
        .unwrap_or_default()
}

impl InputSink for AppShared {
    fn on_event(&self, event: InputEvent) {
        let _ = self.engine_tx.send(EngineMsg::Input(event));
    }

    fn wants_shortcut(&self, shortcut: &Shortcut) -> bool {
        if self.recording_shortcut.load(Ordering::Relaxed) {
            return shortcut.is_valid();
        }
        self.hook_shortcuts.try_read().map(|l| l.contains(shortcut)).unwrap_or(false)
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn is_suspended(&self) -> bool {
        if self.substituting.load(Ordering::Relaxed) {
            return true;
        }
        let mut until = self.suspended_until.lock();
        match *until {
            Some(t) if Instant::now() < t => true,
            Some(_) => {
                *until = None;
                false
            }
            None => false,
        }
    }

    fn is_excluded_app(&self, app_name: &str) -> bool {
        self.excluded_apps.try_read().map(|l| l.matches(app_name)).unwrap_or(false)
    }

    fn space_is_trigger(&self) -> bool {
        self.space_trigger.load(Ordering::Relaxed)
    }
}

/// Small helper used by the engine and the UI to wait a little.
pub fn sleep_ms(ms: u64) {
    if ms > 0 {
        std::thread::sleep(Duration::from_millis(ms));
    }
}
