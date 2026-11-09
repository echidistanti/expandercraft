//! Keyboard monitoring for Wayland sessions: the keyboards and mice are read directly from `/dev/input`.
//!
//! Wayland does not let a program see the keys typed in other windows, so ExpanderCraft reads the input
//! devices like Espanso or keyd do. This needs read access to `/dev/input/event*`: either the
//! `cap_dac_override` capability on the executable (raised only while opening a device) or membership of
//! the `input` group. Key codes are translated with the session's layout (see `xkb` and `layout`).
//! Shortcuts are reported but not swallowed: the active application receives them too.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use evdev::{Device, EventSummary, KeyCode, LedCode};
use parking_lot::{Mutex, RwLock};

use super::xkb::{Keymap, LayoutNames, Output, Stroke, Translator};
use crate::platform::{InputEvent, SharedSink};
use crate::shortcut::{Key, Shortcut};

/// Name of our uinput device, so that it is not read back.
pub const VIRTUAL_KEYBOARD_NAME: &str = "ExpanderCraft virtual keyboard";

/// Why the evdev backend could not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    /// The input devices or /dev/uinput exist but cannot be opened.
    Permission,
    /// No keyboard found (containers, remote sessions...).
    NoKeyboard,
    /// libxkbcommon or the layout description could not be loaded.
    NoLayout,
}

#[derive(Default)]
struct Modifiers {
    /// Pressed modifier keys (evdev codes), all keyboards together.
    pressed: HashSet<u16>,
}

const KEY_LEFTCTRL: u16 = 29;
const KEY_RIGHTCTRL: u16 = 97;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_RIGHTSHIFT: u16 = 54;
const KEY_LEFTALT: u16 = 56;
const KEY_RIGHTALT: u16 = 100;
const KEY_LEFTMETA: u16 = 125;
const KEY_RIGHTMETA: u16 = 126;
const KEY_CAPSLOCK: u16 = 58;

impl Modifiers {
    fn is_modifier(code: u16) -> bool {
        matches!(
            code,
            KEY_LEFTCTRL | KEY_RIGHTCTRL | KEY_LEFTSHIFT | KEY_RIGHTSHIFT | KEY_LEFTALT | KEY_RIGHTALT | KEY_LEFTMETA | KEY_RIGHTMETA
        )
    }
    fn ctrl(&self) -> bool {
        self.pressed.contains(&KEY_LEFTCTRL) || self.pressed.contains(&KEY_RIGHTCTRL)
    }
    fn shift(&self) -> bool {
        self.pressed.contains(&KEY_LEFTSHIFT) || self.pressed.contains(&KEY_RIGHTSHIFT)
    }
    /// Left Alt only: the right Alt key is AltGr in most layouts.
    fn alt(&self) -> bool {
        self.pressed.contains(&KEY_LEFTALT)
    }
    fn meta(&self) -> bool {
        self.pressed.contains(&KEY_LEFTMETA) || self.pressed.contains(&KEY_RIGHTMETA)
    }
}

struct State {
    translator: Translator,
    mods: Modifiers,
    app_cache: Option<(Instant, Option<String>)>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
/// Which keys type each character, for the uinput keyboard.
static STROKES: OnceLock<RwLock<HashMap<char, Stroke>>> = OnceLock::new();
/// The layout in use, and where it comes from.
static LAYOUT: OnceLock<Mutex<(LayoutNames, &'static str)>> = OnceLock::new();
static OPEN_DEVICES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);

fn open_devices() -> &'static Mutex<HashSet<PathBuf>> {
    OPEN_DEVICES.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// The keys to press to type a character with the current layout.
pub fn stroke_for(c: char) -> Option<Stroke> {
    STROKES.get()?.read().get(&c).copied()
}

/// Are Ctrl, Alt, AltGr, Shift or Meta physically held down?
pub fn modifiers_held() -> bool {
    STATE.get().map(|s| !s.lock().mods.pressed.is_empty()).unwrap_or(false)
}

/// Is Caps Lock on (as far as we know)?
pub fn caps_lock() -> bool {
    STATE.get().map(|s| s.lock().translator.caps_lock()).unwrap_or(false)
}

/// The layout in use, in the notation of the preferences, and its source.
pub fn current_layout() -> Option<(String, &'static str)> {
    LAYOUT.get().map(|l| {
        let l = l.lock();
        (l.0.display(), l.1)
    })
}

fn choose_layout(override_: &str) -> Option<(Keymap, LayoutNames, &'static str)> {
    let (names, source) = match LayoutNames::parse(override_) {
        Some(n) => (n, "preferences"),
        None => super::layout::detect(),
    };
    match Keymap::new(&names) {
        Some(k) => Some((k, names, source)),
        None => {
            log::warn!("Keyboard layout '{}' could not be compiled: using 'us'.", names.display());
            let us = LayoutNames::layout("us", "");
            Keymap::new(&us).map(|k| (k, us, "default"))
        }
    }
}

/// Change the layout (from the preferences; empty = automatic).
pub fn set_layout(override_: &str) {
    let Some(state) = STATE.get() else { return };
    // a layout being typed in the preferences ("fr(", "f"...) is ignored until it compiles
    if LayoutNames::parse(override_).is_some_and(|n| Keymap::new(&n).is_none()) {
        return;
    }
    let Some((keymap, names, source)) = choose_layout(override_) else { return };
    if LAYOUT.get().is_some_and(|l| l.lock().0 == names) {
        return;
    }
    log::info!("Keyboard layout: {} ({source})", names.display());
    if let Some(s) = STROKES.get() {
        *s.write() = keymap.reverse_map();
    }
    if let Some(t) = Translator::new(keymap) {
        state.lock().translator = t;
    }
    if let Some(l) = LAYOUT.get() {
        *l.lock() = (names, source);
    }
}

pub fn start(sink: SharedSink, layout_override: &str) -> Result<(), StartError> {
    privileges::drop_effective();
    let (keymap, names, source) = choose_layout(layout_override).ok_or(StartError::NoLayout)?;
    log::info!("Keyboard layout: {} ({source})", names.display());
    let _ = STROKES.set(RwLock::new(keymap.reverse_map()));
    let translator = Translator::new(keymap).ok_or(StartError::NoLayout)?;
    let _ = LAYOUT.set(Mutex::new((names, source)));
    let _ = STATE.set(Mutex::new(State { translator, mods: Modifiers::default(), app_cache: None }));

    let scan = scan_devices(&sink);
    if scan.keyboards == 0 {
        return Err(if scan.denied > 0 { StartError::Permission } else { StartError::NoKeyboard });
    }
    if let Err(e) = super::uinput::create() {
        log::warn!("The virtual keyboard could not be created: {e}");
        return Err(if e.kind() == io::ErrorKind::PermissionDenied || e.kind() == io::ErrorKind::NotFound {
            StartError::Permission
        } else {
            StartError::NoKeyboard
        });
    }
    ACTIVE.store(true, Ordering::Relaxed);
    log::info!("Keyboard monitoring: evdev ({} keyboard(s))", scan.keyboards);

    // keyboards plugged in later
    std::thread::Builder::new()
        .name("expandercraft-evdev-scan".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(2));
                scan_devices(&sink);
            }
        })
        .map_err(|_| StartError::NoKeyboard)?;
    Ok(())
}

#[derive(Default)]
struct ScanReport {
    keyboards: usize,
    denied: usize,
}

fn scan_devices(sink: &SharedSink) -> ScanReport {
    let mut report = ScanReport::default();
    let Ok(dir) = std::fs::read_dir("/dev/input") else { return report };
    let mut paths: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("event")))
        .collect();
    paths.sort();
    for path in paths {
        if open_devices().lock().contains(&path) {
            continue;
        }
        let device = match open_device(&path) {
            Ok(d) => d,
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    report.denied += 1;
                }
                continue;
            }
        };
        let Some(kind) = classify(&device) else { continue };
        if kind == Kind::Keyboard {
            report.keyboards += 1;
            sync_caps_lock(&device);
        }
        log::info!("Reading {} ({:?}: {})", path.display(), kind, device.name().unwrap_or("?"));
        open_devices().lock().insert(path.clone());
        let sink = sink.clone();
        let spawned = std::thread::Builder::new().name("expandercraft-evdev".into()).spawn(move || read_device(path, device, sink));
        if spawned.is_err() {
            log::warn!("Could not start a device reader thread");
        }
    }
    report
}

fn open_device(path: &Path) -> io::Result<Device> {
    match Device::open(path) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => privileges::with_dac_override(|| Device::open(path)),
        r => r,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Keyboard,
    Pointer,
}

fn classify(device: &Device) -> Option<Kind> {
    if device.name() == Some(VIRTUAL_KEYBOARD_NAME) {
        return None;
    }
    let keys = device.supported_keys()?;
    if keys.contains(KeyCode::KEY_A) && keys.contains(KeyCode::KEY_SPACE) && keys.contains(KeyCode::KEY_ENTER) {
        Some(Kind::Keyboard)
    } else if keys.contains(KeyCode::BTN_LEFT) || keys.contains(KeyCode::BTN_TOUCH) {
        Some(Kind::Pointer)
    } else {
        None
    }
}

/// Align our keyboard state with the Caps Lock LED, which the compositor keeps up to date.
fn sync_caps_lock(device: &Device) {
    let Ok(leds) = device.get_led_state() else { return };
    let Some(state) = STATE.get() else { return };
    let mut s = state.lock();
    if leds.contains(LedCode::LED_CAPSL) != s.translator.caps_lock() {
        s.translator.press(KEY_CAPSLOCK, false);
        s.translator.release(KEY_CAPSLOCK);
        s.translator.reset_compose();
    }
}

fn read_device(path: PathBuf, mut device: Device, sink: SharedSink) {
    loop {
        match device.fetch_events() {
            Ok(events) => {
                for ev in events {
                    if let EventSummary::Key(_, code, value) = ev.destructure() {
                        handle_key(&sink, code.0, value);
                    }
                }
            }
            Err(e) => {
                log::info!("Stopped reading {}: {e}", path.display());
                break;
            }
        }
    }
    open_devices().lock().remove(&path);
}

fn is_mouse_button(code: u16) -> bool {
    (0x110..=0x117).contains(&code) || code == 0x14a // BTN_LEFT..BTN_TASK, BTN_TOUCH
}

fn handle_key(sink: &SharedSink, code: u16, value: i32) {
    if is_mouse_button(code) {
        if value == 1 && sink.is_enabled() && !sink.is_suspended() {
            sink.on_event(InputEvent::Breaker);
        }
        return;
    }
    let Some(state) = STATE.get() else { return };
    let mut s = state.lock();
    if value == 0 {
        s.mods.pressed.remove(&code);
        s.translator.release(code);
        return;
    }
    let repeat = value == 2;
    if Modifiers::is_modifier(code) {
        if !repeat {
            s.mods.pressed.insert(code);
            s.translator.press(code, false);
        }
        return;
    }
    // always update the keyboard state (Caps Lock, dead keys), even when the event is ignored
    let output = s.translator.press(code, repeat);
    if sink.is_suspended() {
        return;
    }
    let app = match &s.app_cache {
        Some((t, name)) if t.elapsed() < Duration::from_millis(400) => name.clone(),
        _ => {
            let name = super::active_app_name();
            s.app_cache = Some((Instant::now(), name.clone()));
            name
        }
    };
    if app.as_deref().is_some_and(|a| sink.is_excluded_app(a)) {
        return;
    }
    let (ctrl, alt, meta) = (s.mods.ctrl(), s.mods.alt(), s.mods.meta());
    if (ctrl || alt || meta)
        && let Some(key) = shortcut_key(code, s.translator.keymap())
    {
        let sc = Shortcut { ctrl, alt, shift: s.mods.shift(), meta, key };
        if sink.wants_shortcut(&sc) {
            drop(s);
            sink.on_event(InputEvent::Shortcut(sc));
            return;
        }
    }
    drop(s);
    if !sink.is_enabled() {
        return;
    }
    match code {
        // arrows, page up/down, home/end, insert/delete
        102..=111 => return sink.on_event(InputEvent::Breaker),
        14 => return sink.on_event(InputEvent::Backspace),
        // Enter, keypad Enter, Esc, Tab
        28 | 96 | 1 | 15 => return sink.on_event(InputEvent::Breaker),
        _ => {}
    }
    if ctrl || meta {
        return sink.on_event(InputEvent::Breaker);
    }
    match output {
        Output::Text(text) => {
            for c in text.chars() {
                if c == '\u{8}' {
                    sink.on_event(InputEvent::Backspace);
                } else if c.is_control() {
                    sink.on_event(InputEvent::Breaker);
                } else if c.is_whitespace() {
                    sink.on_event(if sink.space_is_trigger() { InputEvent::Char(c) } else { InputEvent::Breaker });
                } else {
                    sink.on_event(InputEvent::Char(c));
                }
            }
        }
        Output::Cancelled => sink.on_event(InputEvent::Breaker),
        Output::None | Output::Composing => {}
    }
}

/// The key of a shortcut: named keys by code, other keys by the character they type without modifiers.
fn shortcut_key(code: u16, keymap: &Keymap) -> Option<Key> {
    let k = match code {
        28 | 96 => Key::Enter,
        57 => Key::Space,
        15 => Key::Tab,
        14 => Key::Backspace,
        1 => Key::Escape,
        110 => Key::Insert,
        111 => Key::Delete,
        102 => Key::Home,
        107 => Key::End,
        104 => Key::PageUp,
        109 => Key::PageDown,
        105 => Key::Left,
        106 => Key::Right,
        103 => Key::Up,
        108 => Key::Down,
        99 => Key::PrintScreen,
        119 => Key::Pause,
        69 => Key::NumLock,
        58 => Key::CapsLock,
        70 => Key::ScrollLock,
        59..=68 => Key::F((code - 58) as u8),
        87 => Key::F(11),
        88 => Key::F(12),
        183..=194 => Key::F((code - 183 + 13) as u8),
        113 => Key::VolumeMute,
        114 => Key::VolumeDown,
        115 => Key::VolumeUp,
        163 => Key::MediaNext,
        165 => Key::MediaPrevious,
        166 => Key::MediaStop,
        164 => Key::MediaPlayPause,
        // keypad, whatever the Num Lock state
        82 => Key::Char('0'),
        79 => Key::Char('1'),
        80 => Key::Char('2'),
        81 => Key::Char('3'),
        75 => Key::Char('4'),
        76 => Key::Char('5'),
        77 => Key::Char('6'),
        71 => Key::Char('7'),
        72 => Key::Char('8'),
        73 => Key::Char('9'),
        74 => Key::Char('-'),
        78 => Key::Char('+'),
        55 => Key::Char('*'),
        98 => Key::Char('/'),
        _ => {
            let c = keymap.base_char(code)?;
            Key::Char(c.to_uppercase().next().unwrap_or(c))
        }
    };
    Some(k)
}

/// The `cap_dac_override` capability, when the executable has it (`setcap cap_dac_override+p`), is only
/// raised on the thread opening a device, then lowered again.
mod privileges {
    use caps::{CapSet, Capability};

    pub fn drop_effective() {
        if caps::has_cap(None, CapSet::Effective, Capability::CAP_DAC_OVERRIDE).unwrap_or(false) {
            let _ = caps::drop(None, CapSet::Effective, Capability::CAP_DAC_OVERRIDE);
        }
    }

    pub fn with_dac_override<R>(f: impl FnOnce() -> std::io::Result<R>) -> std::io::Result<R> {
        let permitted = caps::has_cap(None, CapSet::Permitted, Capability::CAP_DAC_OVERRIDE).unwrap_or(false);
        if !permitted || caps::raise(None, CapSet::Effective, Capability::CAP_DAC_OVERRIDE).is_err() {
            return f();
        }
        let r = f();
        let _ = caps::drop(None, CapSet::Effective, Capability::CAP_DAC_OVERRIDE);
        r
    }
}

pub use privileges::with_dac_override;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_keys_follow_the_layout() {
        let Some(it) = Keymap::new(&LayoutNames::layout("it", "")) else { return };
        assert_eq!(shortcut_key(48, &it), Some(Key::Char('B'))); // KEY_B
        assert_eq!(shortcut_key(28, &it), Some(Key::Enter));
        assert_eq!(shortcut_key(60, &it), Some(Key::F(2)));
        let Some(fr) = Keymap::new(&LayoutNames::layout("fr", "")) else { return };
        assert_eq!(shortcut_key(16, &fr), Some(Key::Char('A'))); // KEY_Q on AZERTY
    }

    struct Recorder(Mutex<Vec<InputEvent>>);

    impl crate::platform::InputSink for Recorder {
        fn on_event(&self, event: InputEvent) {
            self.0.lock().push(event);
        }
        fn wants_shortcut(&self, sc: &Shortcut) -> bool {
            *sc == Shortcut::default_picker()
        }
        fn is_enabled(&self) -> bool {
            true
        }
        fn is_suspended(&self) -> bool {
            false
        }
        fn is_excluded_app(&self, _: &str) -> bool {
            false
        }
        fn space_is_trigger(&self) -> bool {
            true
        }
    }

    /// Key events as read from an Italian keyboard, through the whole pipeline.
    #[test]
    fn italian_keyboard_events() {
        let Some(keymap) = Keymap::new(&LayoutNames::layout("it", "")) else { return };
        let _ = STATE.set(Mutex::new(State { translator: Translator::new(keymap).unwrap(), mods: Modifiers::default(), app_cache: None }));
        let rec = std::sync::Arc::new(Recorder(Mutex::new(Vec::new())));
        let sink: SharedSink = rec.clone();
        let tap = |code: u16| {
            handle_key(&sink, code, 1);
            handle_key(&sink, code, 0);
        };
        // "Città" + space: Shift+c, i, t, t, à
        handle_key(&sink, KEY_LEFTSHIFT, 1);
        tap(46);
        handle_key(&sink, KEY_LEFTSHIFT, 0);
        for code in [23, 20, 20, 40, 57] {
            tap(code);
        }
        // backspace with auto-repeat, AltGr+ò = @, then the picker shortcut Ctrl+Alt+Shift+Enter, then a click
        handle_key(&sink, 14, 1);
        handle_key(&sink, 14, 2);
        handle_key(&sink, 14, 0);
        handle_key(&sink, KEY_RIGHTALT, 1);
        tap(39);
        handle_key(&sink, KEY_RIGHTALT, 0);
        for m in [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT] {
            handle_key(&sink, m, 1);
        }
        assert!(modifiers_held());
        tap(28);
        for m in [KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT] {
            handle_key(&sink, m, 0);
        }
        assert!(!modifiers_held());
        tap(0x110);
        use InputEvent::*;
        assert_eq!(
            *rec.0.lock(),
            vec![
                Char('C'),
                Char('i'),
                Char('t'),
                Char('t'),
                Char('à'),
                Char(' '),
                Backspace,
                Backspace,
                Char('@'),
                Shortcut(crate::shortcut::Shortcut::default_picker()),
                Breaker
            ]
        );
    }

    #[test]
    fn modifier_codes() {
        assert!(Modifiers::is_modifier(KEY_RIGHTALT));
        assert!(!Modifiers::is_modifier(KEY_CAPSLOCK));
        let mut m = Modifiers::default();
        m.pressed.insert(KEY_RIGHTALT);
        assert!(!m.alt(), "AltGr is not Alt");
        m.pressed.insert(KEY_RIGHTCTRL);
        assert!(m.ctrl());
    }
}
