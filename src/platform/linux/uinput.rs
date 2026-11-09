//! Keystroke synthesis for Wayland sessions: a virtual keyboard created with uinput.
//!
//! The compositor treats it like a real keyboard and applies the user's layout to it, so characters are
//! typed with the keys that produce them in that layout (see `xkb::Keymap::reverse_map`). Characters absent
//! from the layout are typed with Ctrl+Shift+U and their code, which GTK and IBus understand.

use std::io;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode};
use parking_lot::Mutex;

use super::evdev_input::{self, VIRTUAL_KEYBOARD_NAME};
use super::xkb::Stroke;
use crate::platform::Keyboard;
use crate::shortcut::{Key, Shortcut};

static DEVICE: Mutex<Option<VirtualDevice>> = Mutex::new(None);

/// Pause between two events: some applications drop keys sent faster than this.
const EVENT_DELAY: Duration = Duration::from_millis(4);

const KEY_ESC: u16 = 1;
const KEY_BACKSPACE: u16 = 14;
const KEY_TAB: u16 = 15;
const KEY_ENTER: u16 = 28;
const KEY_LEFTCTRL: u16 = 29;
const KEY_LEFTSHIFT: u16 = 42;
const KEY_LEFTALT: u16 = 56;
const KEY_SPACE: u16 = 57;
const KEY_RIGHTALT: u16 = 100;
const KEY_LEFT: u16 = 105;
const KEY_INSERT: u16 = 110;
const KEY_LEFTMETA: u16 = 125;
const KEY_V: u16 = 47;
const KEY_U: u16 = 22;

pub fn create() -> io::Result<()> {
    let builder = match VirtualDevice::builder() {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => evdev_input::with_dac_override(VirtualDevice::builder),
        r => r,
    }?;
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in 1..=248 {
        keys.insert(KeyCode::new(code));
    }
    let device = builder.name(VIRTUAL_KEYBOARD_NAME).with_keys(&keys)?.build()?;
    *DEVICE.lock() = Some(device);
    // the compositor needs a moment to add the new keyboard to the seat
    std::thread::sleep(Duration::from_millis(200));
    Ok(())
}

pub fn is_available() -> bool {
    DEVICE.lock().is_some()
}

fn send(code: u16, down: bool) {
    if let Some(d) = DEVICE.lock().as_mut()
        && let Err(e) = d.emit(&[InputEvent::new(EventType::KEY.0, code, i32::from(down))])
    {
        log::warn!("Virtual keyboard: {e}");
    }
    std::thread::sleep(EVENT_DELAY);
}

fn tap(code: u16) {
    send(code, true);
    send(code, false);
}

/// Press a key with modifiers held.
fn chord(mods: &[u16], code: u16) {
    for m in mods {
        send(*m, true);
    }
    tap(code);
    for m in mods.iter().rev() {
        send(*m, false);
    }
}

fn type_stroke(s: Stroke, invert_shift: bool) {
    let mut mods = Vec::with_capacity(2);
    if s.shift != invert_shift {
        mods.push(KEY_LEFTSHIFT);
    }
    if s.altgr {
        mods.push(KEY_RIGHTALT);
    }
    chord(&mods, s.code);
}

/// Caps Lock inverts the case of letters typed by the virtual keyboard too.
fn affected_by_caps_lock(c: char) -> bool {
    c.is_alphabetic() && (c.is_lowercase() || c.is_uppercase()) && evdev_input::caps_lock()
}

fn type_char(c: char) {
    match c {
        '\n' => tap(KEY_ENTER),
        '\t' => tap(KEY_TAB),
        _ => match evdev_input::stroke_for(c) {
            Some(s) => type_stroke(s, affected_by_caps_lock(c)),
            None => type_unicode(c),
        },
    }
}

/// Ctrl+Shift+U, the hexadecimal code, Space: the Unicode input of GTK and IBus.
fn type_unicode(c: char) {
    let u = evdev_input::stroke_for('u').map(|s| s.code).unwrap_or(KEY_U);
    chord(&[KEY_LEFTCTRL, KEY_LEFTSHIFT], u);
    for h in format!("{:x}", c as u32).chars() {
        if let Some(s) = evdev_input::stroke_for(h) {
            type_stroke(s, false);
        }
    }
    tap(KEY_SPACE);
}

/// The virtual keyboard cannot release keys held on the physical one: wait (a little) for the user to
/// release the modifiers of the shortcut that triggered the substitution.
fn wait_for_modifiers() {
    let start = Instant::now();
    while evdev_input::modifiers_held() && start.elapsed() < Duration::from_millis(1500) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn key_code(key: Key) -> Option<u16> {
    let code = match key {
        Key::Char(c) => {
            return evdev_input::stroke_for(c.to_lowercase().next().unwrap_or(c)).or_else(|| evdev_input::stroke_for(c)).map(|s| s.code);
        }
        Key::F(n @ 1..=10) => 58 + n as u16,
        Key::F(11) => 87,
        Key::F(12) => 88,
        Key::F(n @ 13..=24) => 183 + (n as u16 - 13),
        Key::F(_) => return None,
        Key::Enter => KEY_ENTER,
        Key::Space => KEY_SPACE,
        Key::Tab => KEY_TAB,
        Key::Backspace => KEY_BACKSPACE,
        Key::Escape => KEY_ESC,
        Key::Insert => KEY_INSERT,
        Key::Delete => 111,
        Key::Home => 102,
        Key::End => 107,
        Key::PageUp => 104,
        Key::PageDown => 109,
        Key::Left => KEY_LEFT,
        Key::Right => 106,
        Key::Up => 103,
        Key::Down => 108,
        Key::PrintScreen => 99,
        Key::Pause => 119,
        Key::NumLock => 69,
        Key::CapsLock => 58,
        Key::ScrollLock => 70,
        Key::VolumeMute => 113,
        Key::VolumeUp => 115,
        Key::VolumeDown => 114,
        Key::MediaNext => 163,
        Key::MediaPrevious => 165,
        Key::MediaStop => 166,
        Key::MediaPlayPause => 164,
        Key::MediaSelect => 161,
        Key::Meta => KEY_LEFTMETA,
        Key::Control => KEY_LEFTCTRL,
        Key::Alt => KEY_LEFTALT,
        Key::Shift => KEY_LEFTSHIFT,
    };
    Some(code)
}

/// Keystroke synthesis through the uinput virtual keyboard.
pub struct UinputKeyboard;

impl Keyboard for UinputKeyboard {
    fn erase(&mut self, count: usize) {
        wait_for_modifiers();
        for _ in 0..count {
            tap(KEY_BACKSPACE);
        }
    }

    fn type_text(&mut self, text: &str, delay_ms: u64) {
        wait_for_modifiers();
        for c in text.replace("\r\n", "\n").chars() {
            type_char(c);
            if delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
    }

    fn paste(&mut self, use_shift_insert: bool) {
        wait_for_modifiers();
        if use_shift_insert {
            chord(&[KEY_LEFTSHIFT], KEY_INSERT);
        } else {
            let v = evdev_input::stroke_for('v').map(|s| s.code).unwrap_or(KEY_V);
            chord(&[KEY_LEFTCTRL], v);
        }
    }

    fn press_key(&mut self, key: Key, repeat: u32, delay_ms: u64) {
        wait_for_modifiers();
        let Some(code) = key_code(key) else { return };
        for i in 0..repeat {
            tap(code);
            if i + 1 < repeat && delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
    }

    fn press_shortcut(&mut self, sc: &Shortcut) {
        wait_for_modifiers();
        let Some(code) = key_code(sc.key) else { return };
        let mods: Vec<u16> = [(sc.ctrl, KEY_LEFTCTRL), (sc.alt, KEY_LEFTALT), (sc.shift, KEY_LEFTSHIFT), (sc.meta, KEY_LEFTMETA)]
            .into_iter()
            .filter_map(|(on, k)| on.then_some(k))
            .collect();
        chord(&mods, code);
    }

    fn move_left(&mut self, count: usize) {
        wait_for_modifiers();
        for _ in 0..count {
            tap(KEY_LEFT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_key_codes() {
        assert_eq!(key_code(Key::F(1)), Some(59));
        assert_eq!(key_code(Key::F(10)), Some(68));
        assert_eq!(key_code(Key::F(12)), Some(88));
        assert_eq!(key_code(Key::F(13)), Some(183));
        assert_eq!(key_code(Key::Enter), Some(KEY_ENTER));
    }
}
