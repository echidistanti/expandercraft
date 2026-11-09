//! macOS backend.
//!
//! * Input monitoring with a `CGEventTap` (requires the Accessibility / Input Monitoring permission).
//!   Text is obtained with `CGEventKeyboardGetUnicodeString`, which avoids the Text Input Sources APIs that
//!   must run on the main thread.
//! * Keystrokes are synthesized with `CGEventPost`; synthetic events carry a marker in the
//!   `EVENT_SOURCE_USER_DATA` field so that the tap ignores them.
//! * Paste is Cmd+V.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::time::Duration;

use core_foundation::base::TCFType;
use core_foundation::runloop::{CFRunLoop, kCFRunLoopCommonModes};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, CGKeyCode, CallbackResult,
    EventField,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use foreign_types::ForeignType;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

pub use super::unix_common::acquire_single_instance;
use super::unix_common::{self, ArboardClipboard};
use super::{Clipboard, InputEvent, Keyboard, PlatformStatus, SharedSink, WindowToken};
use crate::shortcut::{Key, Shortcut};
use crate::variables::DateKind;

const INJECTED_MARKER: i64 = 0x4543_5246;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGEventKeyboardGetUnicodeString(event: *mut c_void, max_len: std::ffi::c_ulong, actual_len: *mut std::ffi::c_ulong, buf: *mut u16);
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
}

static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static INPUT_OK: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------------------------------------------------
// Input monitoring
// ---------------------------------------------------------------------------------------------------------------------

pub fn start_input_monitor(sink: SharedSink) -> anyhow::Result<()> {
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("ExpanderCraft is not trusted for accessibility: keyboard monitoring may not work.");
    }
    let (tx, rx) = crossbeam_channel::bounded::<bool>(1);
    std::thread::Builder::new().name("expandercraft-tap".into()).spawn(move || {
        let events = vec![
            CGEventType::KeyDown,
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::OtherMouseDown,
            CGEventType::ScrollWheel,
        ];
        let tap = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::Default,
            events,
            move |_proxy, event_type, event| handle_event(&sink, event_type, event),
        );
        let Ok(tap) = tap else {
            let _ = tx.send(false);
            return;
        };
        TAP_PORT.store(tap.mach_port().as_concrete_TypeRef() as *mut c_void, Ordering::SeqCst);
        let Ok(source) = tap.mach_port().create_runloop_source(0) else {
            let _ = tx.send(false);
            return;
        };
        CFRunLoop::get_current().add_source(&source, unsafe { kCFRunLoopCommonModes });
        tap.enable();
        let _ = tx.send(true);
        CFRunLoop::run_current();
        drop(tap);
    })?;
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(true) => {
            INPUT_OK.store(true, Ordering::SeqCst);
            Ok(())
        }
        _ => anyhow::bail!("could not create the event tap (is the Accessibility permission granted?)"),
    }
}

fn handle_event(sink: &SharedSink, event_type: CGEventType, event: &CGEvent) -> CallbackResult {
    match event_type {
        CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
            let port = TAP_PORT.load(Ordering::SeqCst);
            if !port.is_null() {
                unsafe { CGEventTapEnable(port, true) };
            }
            return CallbackResult::Keep;
        }
        CGEventType::LeftMouseDown | CGEventType::RightMouseDown | CGEventType::OtherMouseDown | CGEventType::ScrollWheel => {
            if sink.is_enabled() && !sink.is_suspended() {
                sink.on_event(InputEvent::Breaker);
            }
            return CallbackResult::Keep;
        }
        CGEventType::KeyDown => {}
        _ => return CallbackResult::Keep,
    }
    if event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == INJECTED_MARKER || sink.is_suspended() {
        return CallbackResult::Keep;
    }
    if let Some(app) = active_app_name()
        && sink.is_excluded_app(&app)
    {
        return CallbackResult::Keep;
    }
    let flags = event.get_flags();
    let ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
    let alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
    let shift = flags.contains(CGEventFlags::CGEventFlagShift);
    let cmd = flags.contains(CGEventFlags::CGEventFlagCommand);
    let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;

    if (ctrl || alt || cmd)
        && let Some(key) = keycode_to_key(keycode)
    {
        let sc = Shortcut { ctrl, alt, shift, meta: cmd, key };
        if sink.wants_shortcut(&sc) {
            sink.on_event(InputEvent::Shortcut(sc));
            return CallbackResult::Drop;
        }
    }
    if !sink.is_enabled() {
        return CallbackResult::Keep;
    }
    match keycode {
        0x7B..=0x7E | 0x73 | 0x74 | 0x77 | 0x79 | 0x75 | 0x72 => {
            sink.on_event(InputEvent::Breaker);
            return CallbackResult::Keep;
        }
        0x33 => {
            sink.on_event(InputEvent::Backspace);
            return CallbackResult::Keep;
        }
        _ => {}
    }
    if ctrl || cmd {
        sink.on_event(InputEvent::Breaker);
        return CallbackResult::Keep;
    }
    let mut buf = [0u16; 8];
    let mut len: std::ffi::c_ulong = 0;
    unsafe { CGEventKeyboardGetUnicodeString(event.as_ptr() as *mut c_void, buf.len() as _, &mut len, buf.as_mut_ptr()) };
    let text = String::from_utf16_lossy(&buf[..(len as usize).min(buf.len())]);
    for c in text.chars() {
        if c == '\u{8}' || c == '\u{7f}' {
            sink.on_event(InputEvent::Backspace);
        } else if c.is_control() {
            sink.on_event(InputEvent::Breaker);
        } else if c.is_whitespace() {
            sink.on_event(if sink.space_is_trigger() { InputEvent::Char(c) } else { InputEvent::Breaker });
        } else {
            sink.on_event(InputEvent::Char(c));
        }
    }
    CallbackResult::Keep
}

/// ANSI virtual key codes (kVK_*) → keys.
const KEYCODES: &[(u16, Key)] = &[
    (0x00, Key::Char('A')),
    (0x0B, Key::Char('B')),
    (0x08, Key::Char('C')),
    (0x02, Key::Char('D')),
    (0x0E, Key::Char('E')),
    (0x03, Key::Char('F')),
    (0x05, Key::Char('G')),
    (0x04, Key::Char('H')),
    (0x22, Key::Char('I')),
    (0x26, Key::Char('J')),
    (0x28, Key::Char('K')),
    (0x25, Key::Char('L')),
    (0x2E, Key::Char('M')),
    (0x2D, Key::Char('N')),
    (0x1F, Key::Char('O')),
    (0x23, Key::Char('P')),
    (0x0C, Key::Char('Q')),
    (0x0F, Key::Char('R')),
    (0x01, Key::Char('S')),
    (0x11, Key::Char('T')),
    (0x20, Key::Char('U')),
    (0x09, Key::Char('V')),
    (0x0D, Key::Char('W')),
    (0x07, Key::Char('X')),
    (0x10, Key::Char('Y')),
    (0x06, Key::Char('Z')),
    (0x1D, Key::Char('0')),
    (0x12, Key::Char('1')),
    (0x13, Key::Char('2')),
    (0x14, Key::Char('3')),
    (0x15, Key::Char('4')),
    (0x17, Key::Char('5')),
    (0x16, Key::Char('6')),
    (0x1A, Key::Char('7')),
    (0x1C, Key::Char('8')),
    (0x19, Key::Char('9')),
    (0x18, Key::Char('=')),
    (0x1B, Key::Char('-')),
    (0x21, Key::Char('[')),
    (0x1E, Key::Char(']')),
    (0x27, Key::Char('\'')),
    (0x29, Key::Char(';')),
    (0x2A, Key::Char('\\')),
    (0x2B, Key::Char(',')),
    (0x2C, Key::Char('/')),
    (0x2F, Key::Char('.')),
    (0x32, Key::Char('`')),
    (0x24, Key::Enter),
    (0x4C, Key::Enter),
    (0x30, Key::Tab),
    (0x31, Key::Space),
    (0x33, Key::Backspace),
    (0x35, Key::Escape),
    (0x72, Key::Insert),
    (0x75, Key::Delete),
    (0x73, Key::Home),
    (0x77, Key::End),
    (0x74, Key::PageUp),
    (0x79, Key::PageDown),
    (0x7B, Key::Left),
    (0x7C, Key::Right),
    (0x7D, Key::Down),
    (0x7E, Key::Up),
    (0x7A, Key::F(1)),
    (0x78, Key::F(2)),
    (0x63, Key::F(3)),
    (0x76, Key::F(4)),
    (0x60, Key::F(5)),
    (0x61, Key::F(6)),
    (0x62, Key::F(7)),
    (0x64, Key::F(8)),
    (0x65, Key::F(9)),
    (0x6D, Key::F(10)),
    (0x67, Key::F(11)),
    (0x6F, Key::F(12)),
    (0x69, Key::F(13)),
    (0x6B, Key::F(14)),
    (0x71, Key::F(15)),
    (0x6A, Key::F(16)),
    (0x40, Key::F(17)),
    (0x4F, Key::F(18)),
    (0x50, Key::F(19)),
    (0x5A, Key::F(20)),
    (0x48, Key::VolumeUp),
    (0x49, Key::VolumeDown),
    (0x4A, Key::VolumeMute),
    (0x37, Key::Meta),
    (0x3B, Key::Control),
    (0x3A, Key::Alt),
    (0x38, Key::Shift),
    (0x39, Key::CapsLock),
];

fn keycode_to_key(code: u16) -> Option<Key> {
    KEYCODES.iter().find(|(c, _)| *c == code).map(|(_, k)| *k)
}

fn key_to_keycode(key: Key) -> Option<u16> {
    let key = match key {
        Key::Char(c) => Key::Char(c.to_ascii_uppercase()),
        k => k,
    };
    KEYCODES.iter().find(|(_, k)| *k == key).map(|(c, _)| *c)
}

// ---------------------------------------------------------------------------------------------------------------------
// Keystroke synthesis
// ---------------------------------------------------------------------------------------------------------------------

pub struct MacKeyboard;

fn source() -> Option<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()
}

fn post_key(code: CGKeyCode, down: bool, flags: CGEventFlags) {
    let Some(src) = source() else { return };
    if let Ok(ev) = CGEvent::new_keyboard_event(src, code, down) {
        ev.set_flags(flags);
        ev.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, INJECTED_MARKER);
        ev.post(CGEventTapLocation::HID);
    }
}

fn tap_key(code: CGKeyCode, flags: CGEventFlags) {
    post_key(code, true, flags);
    post_key(code, false, flags);
}

fn post_text(chunk: &[u16]) {
    let Some(src) = source() else { return };
    for down in [true, false] {
        if let Ok(ev) = CGEvent::new_keyboard_event(src.clone(), 0, down) {
            ev.set_flags(CGEventFlags::CGEventFlagNull);
            ev.set_string_from_utf16_unchecked(chunk);
            ev.set_integer_value_field(EventField::EVENT_SOURCE_USER_DATA, INJECTED_MARKER);
            ev.post(CGEventTapLocation::HID);
        }
    }
}

impl Keyboard for MacKeyboard {
    fn erase(&mut self, count: usize) {
        for _ in 0..count {
            tap_key(0x33, CGEventFlags::CGEventFlagNull);
        }
    }

    fn type_text(&mut self, text: &str, delay_ms: u64) {
        let text = text.replace("\r\n", "\n");
        for c in text.chars() {
            match c {
                '\n' | '\r' => tap_key(0x24, CGEventFlags::CGEventFlagNull),
                '\t' => tap_key(0x30, CGEventFlags::CGEventFlagNull),
                c => {
                    let mut buf = [0u16; 2];
                    post_text(c.encode_utf16(&mut buf));
                }
            }
            if delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
    }

    fn paste(&mut self, _use_shift_insert: bool) {
        tap_key(0x09, CGEventFlags::CGEventFlagCommand);
    }

    fn press_key(&mut self, key: Key, repeat: u32, delay_ms: u64) {
        let Some(code) = key_to_keycode(key) else { return };
        for i in 0..repeat {
            tap_key(code, CGEventFlags::CGEventFlagNull);
            if i + 1 < repeat && delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
    }

    fn press_shortcut(&mut self, sc: &Shortcut) {
        let Some(code) = key_to_keycode(sc.key) else { return };
        let mut flags = CGEventFlags::CGEventFlagNull;
        if sc.ctrl {
            flags |= CGEventFlags::CGEventFlagControl;
        }
        if sc.alt {
            flags |= CGEventFlags::CGEventFlagAlternate;
        }
        if sc.shift {
            flags |= CGEventFlags::CGEventFlagShift;
        }
        if sc.meta {
            flags |= CGEventFlags::CGEventFlagCommand;
        }
        tap_key(code, flags);
    }

    fn move_left(&mut self, count: usize) {
        for _ in 0..count {
            tap_key(0x7B, CGEventFlags::CGEventFlagNull);
        }
    }
}

pub fn new_keyboard() -> Box<dyn Keyboard> {
    Box::new(MacKeyboard)
}

pub fn new_clipboard() -> Box<dyn Clipboard> {
    Box::new(ArboardClipboard)
}

// ---------------------------------------------------------------------------------------------------------------------
// Applications
// ---------------------------------------------------------------------------------------------------------------------

fn frontmost() -> Option<objc2::rc::Retained<NSRunningApplication>> {
    NSWorkspace::sharedWorkspace().frontmostApplication()
}

pub fn active_app_name() -> Option<String> {
    frontmost().and_then(|app| app.localizedName()).map(|s| s.to_string())
}

pub fn foreground_window() -> Option<WindowToken> {
    frontmost().map(|app| WindowToken(app.processIdentifier() as u64))
}

pub fn activate_window(token: WindowToken) {
    if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(token.0 as i32) {
        #[allow(deprecated)]
        app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
    }
}

pub fn is_own_process_foreground() -> bool {
    frontmost().map(|app| app.processIdentifier() as u32 == std::process::id()).unwrap_or(false)
}

// ---------------------------------------------------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------------------------------------------------

pub fn post_substitution_grace() -> Duration {
    Duration::ZERO
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
    unix_common::play_with_commands(path, &["afplay"]);
}

fn launch_agent_file() -> Option<std::path::PathBuf> {
    directories::BaseDirs::new().map(|b| b.home_dir().join("Library/LaunchAgents/app.expandercraft.plist"))
}

pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    let path = launch_agent_file().ok_or_else(|| anyhow::anyhow!(crate::i18n::tr("error-no-config-folder")))?;
    if enabled {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let exe = unix_common::current_exe_string().replace('&', "&amp;").replace('<', "&lt;");
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>app.expandercraft</string>
    <key>ProgramArguments</key><array><string>{exe}</string><string>--background</string></array>
    <key>RunAtLoad</key><true/>
</dict>
</plist>
"#
        );
        std::fs::write(&path, plist)?;
    } else if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn platform_status() -> PlatformStatus {
    let trusted = unsafe { AXIsProcessTrusted() };
    let ok = INPUT_OK.load(Ordering::SeqCst) && trusted;
    PlatformStatus { input_ok: ok, message: (!ok).then(|| crate::i18n::tr("platform-macos-permissions").to_string()), details: None }
}

#[allow(dead_code)]
fn _assert_send() {
    fn is_send<T: Send>() {}
    is_send::<MacKeyboard>();
    let _ = OnceLock::<()>::new();
}
