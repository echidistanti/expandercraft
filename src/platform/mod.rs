//! Platform abstraction layer.
//!
//! Each backend (Windows, macOS, Linux) provides the same set of free functions and implements the
//! [`Keyboard`] and [`Clipboard`] traits. The rest of the application only talks to this module.
//!
//! | Feature                    | Windows                 | macOS                          | Linux                        |
//! |----------------------------|-------------------------|--------------------------------|------------------------------|
//! | Keyboard/mouse monitoring  | `WH_KEYBOARD_LL` hooks  | `CGEventTap`                   | X11: `rdev`; Wayland: evdev  |
//! | Keystroke synthesis        | `SendInput`             | `CGEventPost`                  | X11: XTest; Wayland: uinput  |
//! | Clipboard backup           | all formats (Win32)     | text (`arboard`)               | text (`arboard`)             |
//! | Active application         | process image name      | `NSWorkspace` frontmost app    | X11 / KWin / Sway / Hyprland |
//! | Auto-start                 | `HKCU\...\Run`          | LaunchAgent                    | XDG autostart                |

use std::sync::Arc;

use crate::shortcut::{Key, Shortcut};

/// An event reported by the input monitor.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// A printable character was typed.
    Char(char),
    Backspace,
    /// A "combo breaker": a non printable key, a mouse click, etc. The typed text buffer is reset.
    Breaker,
    /// A shortcut (Ctrl, Alt or Meta + key) that the sink asked to receive.
    Shortcut(Shortcut),
}

/// The receiver of input events. Called from the input monitoring thread: implementations must be fast.
pub trait InputSink: Send + Sync {
    fn on_event(&self, event: InputEvent);
    /// Should this shortcut be reported (and swallowed, where the platform allows it)?
    fn wants_shortcut(&self, shortcut: &Shortcut) -> bool;
    /// Is text expansion enabled? Shortcuts are still reported when disabled.
    fn is_enabled(&self) -> bool;
    /// True while a substitution is in progress: events are ignored.
    fn is_suspended(&self) -> bool;
    fn is_excluded_app(&self, app_name: &str) -> bool;
    /// Is the space character a trigger (rather than a combo breaker)?
    fn space_is_trigger(&self) -> bool;
    /// The keyboard layout chosen in the preferences (Linux, Wayland), empty for automatic detection.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn keyboard_layout(&self) -> String {
        String::new()
    }
}

/// Keystroke synthesis.
pub trait Keyboard: Send {
    /// Erase `count` characters with backspace (pressed modifiers are temporarily released).
    fn erase(&mut self, count: usize);
    /// Type text, character by character.
    fn type_text(&mut self, text: &str, delay_ms: u64);
    /// Trigger the paste command of the active application (Ctrl+V, Shift+Insert, or Cmd+V on macOS).
    fn paste(&mut self, use_shift_insert: bool);
    fn press_key(&mut self, key: Key, repeat: u32, delay_ms: u64);
    fn press_shortcut(&mut self, shortcut: &Shortcut);
    fn move_left(&mut self, count: usize);
}

/// Opaque clipboard content saved before pasting a snippet.
pub struct ClipboardBackup(pub Box<dyn std::any::Any + Send>);

pub trait Clipboard: Send {
    fn text(&mut self) -> Option<String>;
    /// Put text in the clipboard (when possible, flagged so that clipboard history managers ignore it).
    fn set_text(&mut self, text: &str) -> bool;
    fn backup(&mut self) -> Option<ClipboardBackup>;
    fn restore(&mut self, backup: ClipboardBackup);
    fn clear(&mut self);
}

/// Identifies a window (or application) that can be re-activated later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowToken(pub u64);

#[derive(Debug, Clone, Default)]
pub struct PlatformStatus {
    /// Is the global input monitoring working?
    pub input_ok: bool,
    /// A short message for the user (missing permission, limited support...), shown in the status bar.
    pub message: Option<String>,
    /// A longer explanation, shown at startup when the input monitoring does not work.
    pub details: Option<String>,
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use self::linux::*;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
compile_error!("ExpanderCraft supports Windows, macOS and Linux.");

#[cfg(not(windows))]
mod unix_common;

/// Open a file or folder with the default application of the system.
pub fn open_path(path: &std::path::Path) {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };
    if let Err(e) = cmd.spawn() {
        log::warn!("Could not open '{}': {e}", path.display());
    }
}

pub type SharedSink = Arc<dyn InputSink>;
