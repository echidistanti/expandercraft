//! Linux backend.
//!
//! * X11 sessions: `rdev` (XRecord) to read the keyboard, `enigo` (XTest) to type, see `x11`.
//! * Wayland sessions: the input devices are read with evdev (`evdev_input`), text is typed with a uinput
//!   virtual keyboard (`uinput`), the layout comes from libxkbcommon (`xkb`, `layout`) and the active
//!   application from the compositor (`compositor`). Without access to the input devices, ExpanderCraft
//!   falls back to the X11 backend, which only sees X11 (XWayland) applications.
//!
//! The user interface itself always runs on X11 when it is available (XWayland), because Wayland does not
//! let a program hide its windows (tray) or move the focus.

mod compositor;
mod evdev_input;
mod layout;
mod uinput;
mod x11;
mod xkb;

use std::time::Duration;

use parking_lot::Mutex;

pub use super::unix_common::acquire_single_instance;
use super::unix_common::{self, ArboardClipboard};
use super::{Clipboard, Keyboard, PlatformStatus, SharedSink, WindowToken};
use crate::shortcut::{Key, Shortcut};
use crate::variables::DateKind;

/// Why the Wayland backend is not used, if it was tried.
static WAYLAND_PROBLEM: Mutex<Option<evdev_input::StartError>> = Mutex::new(None);

pub fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").map(|s| s.eq_ignore_ascii_case("wayland")).unwrap_or(false)
        || std::env::var_os("WAYLAND_DISPLAY").map(|v| !v.is_empty()).unwrap_or(false)
}

fn wayland_backend() -> bool {
    evdev_input::is_active()
}

pub fn start_input_monitor(sink: SharedSink) -> anyhow::Result<()> {
    if is_wayland_session() {
        match evdev_input::start(sink.clone(), &sink.keyboard_layout()) {
            Ok(()) => {
                compositor::start();
                return Ok(());
            }
            Err(e) => {
                log::warn!("Wayland keyboard monitoring unavailable ({e:?}): only X11 applications will be supported.");
                *WAYLAND_PROBLEM.lock() = Some(e);
            }
        }
    }
    x11::start_input_monitor(sink)
}

/// The keyboard layout preference changed (empty = automatic).
pub fn keyboard_layout_changed(layout: &str) {
    evdev_input::set_layout(layout);
}

/// The layout used to read and type text in a Wayland session, and where it was found.
pub fn keyboard_layout_in_use() -> Option<(String, &'static str)> {
    evdev_input::current_layout()
}

/// Called before quitting.
pub fn shutdown() {
    if wayland_backend() {
        compositor::stop();
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Keystroke synthesis and clipboard
// ---------------------------------------------------------------------------------------------------------------------

/// Types with the uinput keyboard when the Wayland backend runs, with XTest otherwise. The keyboard is
/// created before the input monitor, hence the choice at each call.
struct LinuxKeyboard {
    x11: x11::X11Keyboard,
    uinput: uinput::UinputKeyboard,
}

impl LinuxKeyboard {
    fn get(&mut self) -> &mut dyn Keyboard {
        if uinput::is_available() { &mut self.uinput } else { &mut self.x11 }
    }
}

impl Keyboard for LinuxKeyboard {
    fn erase(&mut self, count: usize) {
        self.get().erase(count);
    }
    fn type_text(&mut self, text: &str, delay_ms: u64) {
        self.get().type_text(text, delay_ms);
    }
    fn paste(&mut self, use_shift_insert: bool) {
        self.get().paste(use_shift_insert);
    }
    fn press_key(&mut self, key: Key, repeat: u32, delay_ms: u64) {
        self.get().press_key(key, repeat, delay_ms);
    }
    fn press_shortcut(&mut self, shortcut: &Shortcut) {
        self.get().press_shortcut(shortcut);
    }
    fn move_left(&mut self, count: usize) {
        self.get().move_left(count);
    }
}

pub fn new_keyboard() -> Box<dyn Keyboard> {
    Box::new(LinuxKeyboard { x11: x11::X11Keyboard::new(), uinput: uinput::UinputKeyboard })
}

pub fn new_clipboard() -> Box<dyn Clipboard> {
    Box::new(ArboardClipboard)
}

// ---------------------------------------------------------------------------------------------------------------------
// Windows and applications
// ---------------------------------------------------------------------------------------------------------------------

pub fn active_app_name() -> Option<String> {
    if wayland_backend()
        && let Some(app) = compositor::active_app()
    {
        return app.map(|a| a.name);
    }
    x11::active_app_name()
}

/// Only X11 windows can be re-activated; on Wayland the focus goes back by itself when our window closes.
pub fn foreground_window() -> Option<WindowToken> {
    let token = x11::foreground_window()?;
    if wayland_backend()
        && let Some(app) = compositor::active_app()
    {
        // the X11 active window can be stale while a Wayland window has the focus: never re-activate it then
        let focused_pid = app.and_then(|a| a.pid);
        if focused_pid.is_none() || focused_pid != x11::token_pid(token) {
            return None;
        }
    }
    Some(token)
}

pub fn activate_window(token: WindowToken) {
    x11::activate_window(token);
}

pub fn is_own_process_foreground() -> bool {
    if wayland_backend()
        && let Some(app) = compositor::active_app()
    {
        return app.and_then(|a| a.pid).is_some_and(|p| p == std::process::id());
    }
    x11::is_own_process_foreground()
}

// ---------------------------------------------------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------------------------------------------------

pub fn post_substitution_grace() -> Duration {
    if wayland_backend() {
        // our virtual keyboard is not read back
        Duration::from_millis(30)
    } else {
        // XRecord delivers our own synthetic events asynchronously
        Duration::from_millis(250)
    }
}

pub fn powershell_exe() -> &'static str {
    "pwsh"
}

pub fn configure_background_command(_cmd: &mut std::process::Command) {}

pub fn system_date(dt: &chrono::DateTime<chrono::Local>, kind: DateKind, locale: chrono::Locale) -> String {
    match kind {
        DateKind::Date => crate::qtdate::default_date(dt, locale),
        DateKind::Time => crate::qtdate::default_time(dt, locale),
        DateKind::DateTime => crate::qtdate::default_date_time(dt, locale),
    }
}

pub fn play_wav_bytes(bytes: &'static [u8]) {
    if let Some(p) = unix_common::notification_wav_file(bytes) {
        play_wav_file(&p.display().to_string());
    }
}

pub fn play_wav_file(path: &str) {
    unix_common::play_with_commands(path, &["paplay", "pw-play", "aplay"]);
}

fn autostart_file() -> Option<std::path::PathBuf> {
    directories::BaseDirs::new().map(|b| b.config_dir().join("autostart").join("expandercraft.desktop"))
}

pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    let path = autostart_file().ok_or_else(|| anyhow::anyhow!(crate::i18n::tr("error-no-config-folder")))?;
    if enabled {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=ExpanderCraft\nComment=Text expander\nExec=\"{}\" --background\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            unix_common::current_exe_string()
        );
        std::fs::write(&path, content)?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// The command that gives the executable access to the input devices.
pub fn setcap_command() -> String {
    format!("sudo setcap cap_dac_override+p '{}'", unix_common::current_exe_string())
}

pub fn platform_status() -> PlatformStatus {
    if wayland_backend() {
        return PlatformStatus { input_ok: true, ..Default::default() };
    }
    let x11_ok = *x11::input_ok().lock();
    match *WAYLAND_PROBLEM.lock() {
        Some(problem) => PlatformStatus {
            input_ok: false,
            message: Some(crate::i18n::tr("platform-wayland").to_string()),
            details: Some(match problem {
                evdev_input::StartError::Permission => tr!("platform-wayland-permission", command = setcap_command()),
                evdev_input::StartError::NoKeyboard | evdev_input::StartError::NoLayout => {
                    crate::i18n::tr("platform-wayland-no-keyboard").to_string()
                }
            }),
        },
        None if !x11_ok => {
            PlatformStatus { input_ok: false, message: Some(crate::i18n::tr("platform-x11-required").to_string()), details: None }
        }
        None => PlatformStatus { input_ok: true, ..Default::default() },
    }
}

/// The tray icon needs a GTK main loop on Linux: run it in a dedicated thread.
pub fn spawn_tray_thread(build: impl FnOnce() -> Option<Box<dyn std::any::Any>> + Send + 'static) {
    std::thread::Builder::new()
        .name("expandercraft-tray".into())
        .spawn(move || {
            if gtk::init().is_err() {
                log::warn!("GTK could not be initialized: no tray icon.");
                return;
            }
            let _tray = build();
            gtk::main();
        })
        .ok();
}
