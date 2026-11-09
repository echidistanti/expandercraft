//! Linux backend (X11 sessions, and X11/XWayland applications in Wayland sessions).
//!
//! * Input monitoring: `rdev` (XRecord). Shortcuts are reported but cannot be swallowed.
//! * Keystroke synthesis: `enigo` (XTest).
//! * Active application: `_NET_ACTIVE_WINDOW` + `_NET_WM_PID` + `/proc/<pid>/comm`.
//!
//! Native Wayland applications are not visible to X11 clients: a future backend based on evdev/uinput
//! (or the RemoteDesktop/InputCapture portals) is needed for them. See the README.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use enigo::{Direction, Enigo, Keyboard as _, Settings};
use parking_lot::Mutex;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};
use x11rb::rust_connection::RustConnection;

pub use super::unix_common::acquire_single_instance;
use super::unix_common::{self, ArboardClipboard};
use super::{Clipboard, InputEvent, Keyboard, PlatformStatus, SharedSink, WindowToken};
use crate::shortcut::{Key, Shortcut};
use crate::variables::DateKind;

// ---------------------------------------------------------------------------------------------------------------------
// Input monitoring
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Mods {
    ctrl_l: bool,
    ctrl_r: bool,
    alt: bool,
    shift_l: bool,
    shift_r: bool,
    meta_l: bool,
    meta_r: bool,
}

static INPUT_OK: OnceLock<Mutex<bool>> = OnceLock::new();

fn input_ok() -> &'static Mutex<bool> {
    INPUT_OK.get_or_init(|| Mutex::new(false))
}

pub fn start_input_monitor(sink: SharedSink) -> anyhow::Result<()> {
    if std::env::var_os("DISPLAY").is_none() {
        anyhow::bail!("no X11 display available (DISPLAY is not set)");
    }
    *input_ok().lock() = true;
    std::thread::Builder::new().name("expandercraft-input".into()).spawn(move || {
        let mut mods = Mods::default();
        let mut app_cache: Option<(Instant, Option<String>)> = None;
        let result = rdev::listen(move |event| handle_event(&sink, &mut mods, &mut app_cache, event));
        if let Err(e) = result {
            *input_ok().lock() = false;
            log::error!("Keyboard monitoring failed: {e:?}");
        }
    })?;
    Ok(())
}

fn handle_event(sink: &SharedSink, mods: &mut Mods, app_cache: &mut Option<(Instant, Option<String>)>, event: rdev::Event) {
    use rdev::{EventType, Key as K};
    match event.event_type {
        EventType::KeyRelease(k) => {
            set_mod(mods, k, false);
        }
        EventType::ButtonPress(_) | EventType::Wheel { .. } => {
            if sink.is_enabled() && !sink.is_suspended() {
                sink.on_event(InputEvent::Breaker);
            }
        }
        EventType::KeyPress(k) => {
            if set_mod(mods, k, true) {
                return;
            }
            if sink.is_suspended() {
                return;
            }
            let app = match app_cache {
                Some((t, name)) if t.elapsed() < Duration::from_millis(400) => name.clone(),
                _ => {
                    let name = active_app_name();
                    *app_cache = Some((Instant::now(), name.clone()));
                    name
                }
            };
            if app.as_deref().map(|a| sink.is_excluded_app(a)).unwrap_or(false) {
                return;
            }
            let ctrl = mods.ctrl_l || mods.ctrl_r;
            let meta = mods.meta_l || mods.meta_r;
            if (ctrl || mods.alt || meta)
                && let Some(key) = rdev_to_key(k)
            {
                let sc = Shortcut { ctrl, alt: mods.alt, shift: mods.shift_l || mods.shift_r, meta, key };
                if sink.wants_shortcut(&sc) {
                    sink.on_event(InputEvent::Shortcut(sc));
                    return;
                }
            }
            if !sink.is_enabled() {
                return;
            }
            match k {
                K::UpArrow
                | K::DownArrow
                | K::LeftArrow
                | K::RightArrow
                | K::PageUp
                | K::PageDown
                | K::Home
                | K::End
                | K::Insert
                | K::Delete => {
                    sink.on_event(InputEvent::Breaker);
                    return;
                }
                K::Backspace => {
                    sink.on_event(InputEvent::Backspace);
                    return;
                }
                K::CapsLock | K::AltGr | K::Function => return,
                K::Return | K::KpReturn | K::Escape | K::Tab => {
                    sink.on_event(InputEvent::Breaker);
                    return;
                }
                _ => {}
            }
            if ctrl || meta {
                sink.on_event(InputEvent::Breaker);
                return;
            }
            let Some(name) = event.name else { return };
            for c in name.chars() {
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
        _ => {}
    }
}

/// Update the modifier state. Returns true if the key is a modifier.
fn set_mod(m: &mut Mods, k: rdev::Key, down: bool) -> bool {
    use rdev::Key as K;
    match k {
        K::ControlLeft => m.ctrl_l = down,
        K::ControlRight => m.ctrl_r = down,
        K::Alt => m.alt = down,
        K::ShiftLeft => m.shift_l = down,
        K::ShiftRight => m.shift_r = down,
        K::MetaLeft => m.meta_l = down,
        K::MetaRight => m.meta_r = down,
        _ => return false,
    }
    true
}

fn rdev_to_key(k: rdev::Key) -> Option<Key> {
    use rdev::Key as K;
    let c = |c: char| Some(Key::Char(c));
    match k {
        K::KeyA => c('A'),
        K::KeyB => c('B'),
        K::KeyC => c('C'),
        K::KeyD => c('D'),
        K::KeyE => c('E'),
        K::KeyF => c('F'),
        K::KeyG => c('G'),
        K::KeyH => c('H'),
        K::KeyI => c('I'),
        K::KeyJ => c('J'),
        K::KeyK => c('K'),
        K::KeyL => c('L'),
        K::KeyM => c('M'),
        K::KeyN => c('N'),
        K::KeyO => c('O'),
        K::KeyP => c('P'),
        K::KeyQ => c('Q'),
        K::KeyR => c('R'),
        K::KeyS => c('S'),
        K::KeyT => c('T'),
        K::KeyU => c('U'),
        K::KeyV => c('V'),
        K::KeyW => c('W'),
        K::KeyX => c('X'),
        K::KeyY => c('Y'),
        K::KeyZ => c('Z'),
        K::Num0 | K::Kp0 => c('0'),
        K::Num1 | K::Kp1 => c('1'),
        K::Num2 | K::Kp2 => c('2'),
        K::Num3 | K::Kp3 => c('3'),
        K::Num4 | K::Kp4 => c('4'),
        K::Num5 | K::Kp5 => c('5'),
        K::Num6 | K::Kp6 => c('6'),
        K::Num7 | K::Kp7 => c('7'),
        K::Num8 | K::Kp8 => c('8'),
        K::Num9 | K::Kp9 => c('9'),
        K::Minus | K::KpMinus => c('-'),
        K::Equal => c('='),
        K::KpPlus => c('+'),
        K::KpMultiply => c('*'),
        K::KpDivide | K::Slash => c('/'),
        K::Comma => c(','),
        K::Dot => c('.'),
        K::SemiColon => c(';'),
        K::Quote => c('\''),
        K::BackQuote => c('`'),
        K::BackSlash | K::IntlBackslash => c('\\'),
        K::LeftBracket => c('['),
        K::RightBracket => c(']'),
        K::Return | K::KpReturn => Some(Key::Enter),
        K::Space => Some(Key::Space),
        K::Tab => Some(Key::Tab),
        K::Backspace => Some(Key::Backspace),
        K::Escape => Some(Key::Escape),
        K::Insert => Some(Key::Insert),
        K::Delete | K::KpDelete => Some(Key::Delete),
        K::Home => Some(Key::Home),
        K::End => Some(Key::End),
        K::PageUp => Some(Key::PageUp),
        K::PageDown => Some(Key::PageDown),
        K::LeftArrow => Some(Key::Left),
        K::RightArrow => Some(Key::Right),
        K::UpArrow => Some(Key::Up),
        K::DownArrow => Some(Key::Down),
        K::PrintScreen => Some(Key::PrintScreen),
        K::Pause => Some(Key::Pause),
        K::NumLock => Some(Key::NumLock),
        K::CapsLock => Some(Key::CapsLock),
        K::ScrollLock => Some(Key::ScrollLock),
        K::F1 => Some(Key::F(1)),
        K::F2 => Some(Key::F(2)),
        K::F3 => Some(Key::F(3)),
        K::F4 => Some(Key::F(4)),
        K::F5 => Some(Key::F(5)),
        K::F6 => Some(Key::F(6)),
        K::F7 => Some(Key::F(7)),
        K::F8 => Some(Key::F(8)),
        K::F9 => Some(Key::F(9)),
        K::F10 => Some(Key::F(10)),
        K::F11 => Some(Key::F(11)),
        K::F12 => Some(Key::F(12)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Keystroke synthesis
// ---------------------------------------------------------------------------------------------------------------------

pub struct LinuxKeyboard {
    enigo: Option<Enigo>,
}

// Enigo keeps an X11 connection; the keyboard object is only ever used from the engine thread.
unsafe impl Send for LinuxKeyboard {}

impl LinuxKeyboard {
    fn enigo(&mut self) -> Option<&mut Enigo> {
        if self.enigo.is_none() {
            match Enigo::new(&Settings::default()) {
                Ok(e) => self.enigo = Some(e),
                Err(e) => log::error!("Could not initialize keystroke synthesis: {e:?}"),
            }
        }
        self.enigo.as_mut()
    }

    fn click(&mut self, key: enigo::Key) {
        if let Some(e) = self.enigo() {
            let _ = e.key(key, Direction::Click);
        }
    }
}

fn to_enigo(key: Key) -> enigo::Key {
    use enigo::Key as E;
    match key {
        Key::Char(c) => E::Unicode(c.to_ascii_lowercase()),
        Key::F(n) => match n {
            1 => E::F1,
            2 => E::F2,
            3 => E::F3,
            4 => E::F4,
            5 => E::F5,
            6 => E::F6,
            7 => E::F7,
            8 => E::F8,
            9 => E::F9,
            10 => E::F10,
            11 => E::F11,
            12 => E::F12,
            13 => E::F13,
            14 => E::F14,
            15 => E::F15,
            16 => E::F16,
            17 => E::F17,
            18 => E::F18,
            19 => E::F19,
            20 => E::F20,
            21 => E::F21,
            22 => E::F22,
            23 => E::F23,
            _ => E::F24,
        },
        Key::Enter => E::Return,
        Key::Space => E::Space,
        Key::Tab => E::Tab,
        Key::Backspace => E::Backspace,
        Key::Escape => E::Escape,
        Key::Insert => E::Insert,
        Key::Delete => E::Delete,
        Key::Home => E::Home,
        Key::End => E::End,
        Key::PageUp => E::PageUp,
        Key::PageDown => E::PageDown,
        Key::Left => E::LeftArrow,
        Key::Right => E::RightArrow,
        Key::Up => E::UpArrow,
        Key::Down => E::DownArrow,
        Key::PrintScreen => E::Other(0xff61),
        Key::Pause => E::Pause,
        Key::NumLock => E::Numlock,
        Key::CapsLock => E::CapsLock,
        Key::ScrollLock => E::Other(0xff14),
        Key::VolumeMute => E::VolumeMute,
        Key::VolumeUp => E::VolumeUp,
        Key::VolumeDown => E::VolumeDown,
        Key::MediaNext => E::MediaNextTrack,
        Key::MediaPrevious => E::MediaPrevTrack,
        Key::MediaStop => E::MediaStop,
        Key::MediaPlayPause => E::MediaPlayPause,
        Key::MediaSelect => E::Other(0x1008ff32),
        Key::Meta => E::Meta,
        Key::Control => E::Control,
        Key::Alt => E::Alt,
        Key::Shift => E::Shift,
    }
}

impl Keyboard for LinuxKeyboard {
    fn erase(&mut self, count: usize) {
        for _ in 0..count {
            self.click(enigo::Key::Backspace);
        }
    }

    fn type_text(&mut self, text: &str, delay_ms: u64) {
        let text = text.replace("\r\n", "\n");
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                self.click(enigo::Key::Return);
            }
            if line.is_empty() {
                continue;
            }
            if delay_ms == 0 {
                if let Some(e) = self.enigo() {
                    let _ = e.text(line);
                }
            } else {
                for c in line.chars() {
                    if let Some(e) = self.enigo() {
                        let _ = e.text(&c.to_string());
                    }
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
            }
        }
    }

    fn paste(&mut self, use_shift_insert: bool) {
        let Some(e) = self.enigo() else { return };
        if use_shift_insert {
            let _ = e.key(enigo::Key::Shift, Direction::Press);
            let _ = e.key(enigo::Key::Insert, Direction::Click);
            let _ = e.key(enigo::Key::Shift, Direction::Release);
        } else {
            let _ = e.key(enigo::Key::Control, Direction::Press);
            let _ = e.key(enigo::Key::Unicode('v'), Direction::Click);
            let _ = e.key(enigo::Key::Control, Direction::Release);
        }
    }

    fn press_key(&mut self, key: Key, repeat: u32, delay_ms: u64) {
        for i in 0..repeat {
            self.click(to_enigo(key));
            if i + 1 < repeat && delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
    }

    fn press_shortcut(&mut self, sc: &Shortcut) {
        let Some(e) = self.enigo() else { return };
        let mods: Vec<enigo::Key> =
            [(sc.ctrl, enigo::Key::Control), (sc.alt, enigo::Key::Alt), (sc.shift, enigo::Key::Shift), (sc.meta, enigo::Key::Meta)]
                .into_iter()
                .filter_map(|(on, k)| on.then_some(k))
                .collect();
        for m in &mods {
            let _ = e.key(*m, Direction::Press);
        }
        let _ = e.key(to_enigo(sc.key), Direction::Click);
        for m in mods.iter().rev() {
            let _ = e.key(*m, Direction::Release);
        }
    }

    fn move_left(&mut self, count: usize) {
        for _ in 0..count {
            self.click(enigo::Key::LeftArrow);
        }
    }
}

pub fn new_keyboard() -> Box<dyn Keyboard> {
    Box::new(LinuxKeyboard { enigo: None })
}

pub fn new_clipboard() -> Box<dyn Clipboard> {
    Box::new(ArboardClipboard)
}

// ---------------------------------------------------------------------------------------------------------------------
// Windows and applications (X11)
// ---------------------------------------------------------------------------------------------------------------------

struct X11 {
    conn: RustConnection,
    root: Window,
    net_active_window: u32,
    net_wm_pid: u32,
}

fn x11() -> Option<&'static Mutex<X11>> {
    static X: OnceLock<Option<Mutex<X11>>> = OnceLock::new();
    X.get_or_init(|| {
        std::env::var_os("DISPLAY")?;
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| conn.intern_atom(false, name).ok()?.reply().ok().map(|r| r.atom);
        let net_active_window = atom(b"_NET_ACTIVE_WINDOW")?;
        let net_wm_pid = atom(b"_NET_WM_PID")?;
        Some(Mutex::new(X11 { conn, root, net_active_window, net_wm_pid }))
    })
    .as_ref()
}

fn active_window() -> Option<Window> {
    let x = x11()?.lock();
    let reply = x.conn.get_property(false, x.root, x.net_active_window, AtomEnum::WINDOW, 0, 1).ok()?.reply().ok()?;
    reply.value32()?.next().filter(|w| *w != 0)
}

fn window_pid(window: Window) -> Option<u32> {
    let x = x11()?.lock();
    let reply = x.conn.get_property(false, window, x.net_wm_pid, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?;
    reply.value32()?.next()
}

pub fn active_app_name() -> Option<String> {
    let pid = window_pid(active_window()?)?;
    std::fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|s| s.trim().to_string())
}

pub fn foreground_window() -> Option<WindowToken> {
    active_window().map(|w| WindowToken(w as u64))
}

pub fn activate_window(token: WindowToken) {
    let Some(x) = x11() else { return };
    let x = x.lock();
    let event = ClientMessageEvent::new(32, token.0 as Window, x.net_active_window, [2u32, 0, 0, 0, 0]);
    let _ = x.conn.send_event(false, x.root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event);
    let _ = x.conn.flush();
}

pub fn is_own_process_foreground() -> bool {
    active_window().and_then(window_pid).map(|p| p == std::process::id()).unwrap_or(false)
}

// ---------------------------------------------------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------------------------------------------------

pub fn post_substitution_grace() -> Duration {
    // XRecord delivers our own synthetic events asynchronously
    Duration::from_millis(250)
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
    let path = autostart_file().ok_or_else(|| anyhow::anyhow!("no configuration folder"))?;
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

pub fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").map(|s| s.eq_ignore_ascii_case("wayland")).unwrap_or(false)
        || std::env::var_os("WAYLAND_DISPLAY").map(|v| !v.is_empty()).unwrap_or(false)
}

pub fn platform_status() -> PlatformStatus {
    let ok = *input_ok().lock();
    let message = if !ok {
        Some(crate::i18n::tr("Keyboard monitoring is not available: an X11 display is required.").to_string())
    } else if is_wayland_session() {
        Some(crate::i18n::tr("Wayland session: ExpanderCraft only works in X11/XWayland applications for now.").to_string())
    } else {
        None
    };
    PlatformStatus { input_ok: ok, message }
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
