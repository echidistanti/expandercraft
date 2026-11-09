//! Keyboard layouts with libxkbcommon (loaded at run time): evdev key codes to text, and text to key strokes.
//!
//! In a Wayland session the compositor applies the user's layout to every keyboard, including our uinput
//! virtual keyboard. ExpanderCraft compiles the same layout (see `layout`) to understand what is typed and to
//! know which keys produce a character.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::ptr;

use xkbcommon_dl::{
    XkbCommon, XkbCommonCompose, xkb_compose_compile_flags, xkb_compose_feed_result, xkb_compose_state, xkb_compose_state_flags,
    xkb_compose_status, xkb_compose_table, xkb_context, xkb_context_flags, xkb_key_direction, xkb_keymap, xkb_keymap_compile_flags,
    xkb_rule_names, xkb_state, xkb_state_component,
};

/// evdev key codes are XKB key codes minus 8.
const EVDEV_OFFSET: u32 = 8;

pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_RIGHTALT: u16 = 100;

/// The "RMLVO" names describing a layout. Empty fields use the libxkbcommon defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutNames {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: String,
}

impl LayoutNames {
    pub fn layout(layout: &str, variant: &str) -> Self {
        Self { layout: layout.into(), variant: variant.into(), ..Default::default() }
    }

    /// Parse the notation used in the preferences: `it`, `fr(oss)`, `de(nodeadkeys)`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        match s.split_once('(') {
            Some((l, v)) => Some(Self::layout(l.trim(), v.trim_end_matches(')').trim())),
            None => Some(Self::layout(s, "")),
        }
    }

    /// The layout in the notation of the preferences.
    pub fn display(&self) -> String {
        if self.variant.is_empty() { self.layout.clone() } else { format!("{}({})", self.layout, self.variant) }
    }
}

/// How to produce a character: a key and the modifiers to hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stroke {
    /// evdev key code.
    pub code: u16,
    pub shift: bool,
    /// AltGr (ISO_Level3_Shift, on the right Alt key).
    pub altgr: bool,
}

fn lib() -> Option<&'static XkbCommon> {
    xkbcommon_dl::xkbcommon_option()
}

/// A compiled keymap.
pub struct Keymap {
    xkb: &'static XkbCommon,
    ctx: *mut xkb_context,
    keymap: *mut xkb_keymap,
}

// The raw pointers are only used through &self/&mut self, behind the backend's mutex.
unsafe impl Send for Keymap {}

impl Keymap {
    pub fn new(names: &LayoutNames) -> Option<Keymap> {
        let xkb = lib()?;
        let c = |s: &str| CString::new(s).ok();
        let (rules, model, layout, variant, options) =
            (c(&names.rules)?, c(&names.model)?, c(&names.layout)?, c(&names.variant)?, c(&names.options)?);
        let p = |s: &CString| if s.as_bytes().is_empty() { ptr::null() } else { s.as_ptr() };
        let rmlvo = xkb_rule_names { rules: p(&rules), model: p(&model), layout: p(&layout), variant: p(&variant), options: p(&options) };
        unsafe {
            // no environment names: the layout must be exactly the one detected or chosen
            let ctx = (xkb.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_ENVIRONMENT_NAMES);
            if ctx.is_null() {
                return None;
            }
            let keymap = (xkb.xkb_keymap_new_from_names)(ctx, &rmlvo, xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS);
            if keymap.is_null() {
                (xkb.xkb_context_unref)(ctx);
                return None;
            }
            Some(Keymap { xkb, ctx, keymap })
        }
    }

    fn new_state(&self) -> *mut xkb_state {
        unsafe { (self.xkb.xkb_state_new)(self.keymap) }
    }

    /// Which keys produce each character, preferring the fewest modifiers. Each candidate is checked with a
    /// real keyboard state, so layouts where AltGr does not exist (us) or does something else are handled.
    pub fn reverse_map(&self) -> HashMap<char, Stroke> {
        let mut map = HashMap::new();
        for (shift, altgr) in [(false, false), (true, false), (false, true), (true, true)] {
            let state = self.new_state();
            if state.is_null() {
                continue;
            }
            unsafe {
                if shift {
                    (self.xkb.xkb_state_update_key)(state, KEY_LEFTSHIFT as u32 + EVDEV_OFFSET, xkb_key_direction::XKB_KEY_DOWN);
                }
                if altgr {
                    (self.xkb.xkb_state_update_key)(state, KEY_RIGHTALT as u32 + EVDEV_OFFSET, xkb_key_direction::XKB_KEY_DOWN);
                }
                for code in 1u16..=248 {
                    if code == KEY_LEFTSHIFT || code == KEY_RIGHTALT {
                        continue;
                    }
                    let u = (self.xkb.xkb_state_key_get_utf32)(state, code as u32 + EVDEV_OFFSET);
                    if let Some(ch) = char::from_u32(u).filter(|c| *c != '\0' && !c.is_control()) {
                        map.entry(ch).or_insert(Stroke { code, shift, altgr });
                    }
                }
                (self.xkb.xkb_state_unref)(state);
            }
        }
        map
    }

    /// The character of a key without modifiers (for shortcuts: the Q key of an AZERTY keyboard is "A").
    pub fn base_char(&self, code: u16) -> Option<char> {
        let state = self.new_state();
        if state.is_null() {
            return None;
        }
        let u = unsafe {
            let u = (self.xkb.xkb_state_key_get_utf32)(state, code as u32 + EVDEV_OFFSET);
            (self.xkb.xkb_state_unref)(state);
            u
        };
        char::from_u32(u).filter(|c| *c != '\0' && !c.is_control())
    }
}

impl Drop for Keymap {
    fn drop(&mut self) {
        unsafe {
            (self.xkb.xkb_keymap_unref)(self.keymap);
            (self.xkb.xkb_context_unref)(self.ctx);
        }
    }
}

/// What a key press produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// Nothing printable (modifier, function key...).
    None,
    Text(String),
    /// A dead key or the beginning of a compose sequence: the character comes with a later key.
    Composing,
    /// A compose sequence was abandoned.
    Cancelled,
}

struct Compose {
    lib: &'static XkbCommonCompose,
    table: *mut xkb_compose_table,
    state: *mut xkb_compose_state,
}

impl Compose {
    fn new(ctx: *mut xkb_context) -> Option<Compose> {
        let lib = xkbcommon_dl::xkbcommon_compose_option()?;
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| "en_US.UTF-8".into());
        let locale = CString::new(locale).ok()?;
        unsafe {
            let mut table =
                (lib.xkb_compose_table_new_from_locale)(ctx, locale.as_ptr(), xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS);
            if table.is_null() {
                let fallback = c"en_US.UTF-8";
                table = (lib.xkb_compose_table_new_from_locale)(
                    ctx,
                    fallback.as_ptr(),
                    xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
                );
            }
            if table.is_null() {
                return None;
            }
            let state = (lib.xkb_compose_state_new)(table, xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS);
            if state.is_null() {
                (lib.xkb_compose_table_unref)(table);
                return None;
            }
            Some(Compose { lib, table, state })
        }
    }
}

impl Drop for Compose {
    fn drop(&mut self) {
        unsafe {
            (self.lib.xkb_compose_state_unref)(self.state);
            (self.lib.xkb_compose_table_unref)(self.table);
        }
    }
}

/// The state of the keyboard as seen by ExpanderCraft (modifiers, dead keys), fed with every key event.
pub struct Translator {
    keymap: Keymap,
    state: *mut xkb_state,
    compose: Option<Compose>,
}

unsafe impl Send for Translator {}

impl Translator {
    pub fn new(keymap: Keymap) -> Option<Translator> {
        let state = keymap.new_state();
        if state.is_null() {
            return None;
        }
        let compose = Compose::new(keymap.ctx);
        Some(Translator { keymap, state, compose })
    }

    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// A key was released.
    pub fn release(&mut self, code: u16) {
        unsafe {
            (self.keymap.xkb.xkb_state_update_key)(self.state, code as u32 + EVDEV_OFFSET, xkb_key_direction::XKB_KEY_UP);
        }
    }

    /// A key was pressed (`repeat`: auto-repeat of a key already down).
    pub fn press(&mut self, code: u16, repeat: bool) -> Output {
        let xkb = self.keymap.xkb;
        let kc = code as u32 + EVDEV_OFFSET;
        let (sym, utf32) = unsafe { ((xkb.xkb_state_key_get_one_sym)(self.state, kc), (xkb.xkb_state_key_get_utf32)(self.state, kc)) };
        if !repeat {
            unsafe {
                (xkb.xkb_state_update_key)(self.state, kc, xkb_key_direction::XKB_KEY_DOWN);
            }
        }
        if let Some(c) = &self.compose {
            unsafe {
                if (c.lib.xkb_compose_state_feed)(c.state, sym) == xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED {
                    match (c.lib.xkb_compose_state_get_status)(c.state) {
                        xkb_compose_status::XKB_COMPOSE_COMPOSING => return Output::Composing,
                        xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                            let mut buf = [0 as c_char; 64];
                            let n = (c.lib.xkb_compose_state_get_utf8)(c.state, buf.as_mut_ptr(), buf.len());
                            (c.lib.xkb_compose_state_reset)(c.state);
                            if n > 0 {
                                let text = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                                return Output::Text(text);
                            }
                            return Output::None;
                        }
                        xkb_compose_status::XKB_COMPOSE_CANCELLED => {
                            (c.lib.xkb_compose_state_reset)(c.state);
                            return Output::Cancelled;
                        }
                        xkb_compose_status::XKB_COMPOSE_NOTHING => {}
                    }
                }
            }
        }
        match char::from_u32(utf32) {
            Some(ch) if ch != '\0' => Output::Text(ch.to_string()),
            _ => Output::None,
        }
    }

    /// Is Caps Lock active in this state?
    pub fn caps_lock(&self) -> bool {
        unsafe {
            (self.keymap.xkb.xkb_state_mod_name_is_active)(
                self.state,
                xkbcommon_dl::XKB_MOD_NAME_CAPS.as_ptr().cast(),
                xkb_state_component::XKB_STATE_MODS_LOCKED,
            ) > 0
        }
    }

    /// Abandon a dead key or compose sequence in progress.
    pub fn reset_compose(&mut self) {
        if let Some(c) = &self.compose {
            unsafe { (c.lib.xkb_compose_state_reset)(c.state) };
        }
    }
}

impl Drop for Translator {
    fn drop(&mut self) {
        // the compose state must go before the context owned by the keymap
        self.compose = None;
        unsafe { (self.keymap.xkb.xkb_state_unref)(self.state) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // evdev codes
    const KEY_A: u16 = 30;
    const KEY_Q: u16 = 16;
    const KEY_LEFTBRACE: u16 = 26;
    const KEY_SEMICOLON: u16 = 39;
    const KEY_E: u16 = 18;
    const KEY_2: u16 = 3;

    fn translator(layout: &str, variant: &str) -> Option<Translator> {
        Translator::new(Keymap::new(&LayoutNames::layout(layout, variant))?)
    }

    fn text(t: &mut Translator, code: u16) -> Output {
        let o = t.press(code, false);
        t.release(code);
        o
    }

    #[test]
    fn layout_notation() {
        assert_eq!(LayoutNames::parse("fr(oss)").unwrap(), LayoutNames::layout("fr", "oss"));
        assert_eq!(LayoutNames::parse(" it ").unwrap().display(), "it");
        assert!(LayoutNames::parse("").is_none());
    }

    #[test]
    fn italian_layout() {
        let Some(mut t) = translator("it", "") else { return eprintln!("libxkbcommon not available") };
        assert_eq!(text(&mut t, KEY_A), Output::Text("a".into()));
        assert_eq!(text(&mut t, KEY_LEFTBRACE), Output::Text("è".into()));
        assert_eq!(text(&mut t, KEY_SEMICOLON), Output::Text("ò".into()));
        // Shift+è = é, AltGr+ò = @
        t.press(KEY_LEFTSHIFT, false);
        assert_eq!(text(&mut t, KEY_LEFTBRACE), Output::Text("é".into()));
        t.release(KEY_LEFTSHIFT);
        t.press(KEY_RIGHTALT, false);
        assert_eq!(text(&mut t, KEY_SEMICOLON), Output::Text("@".into()));
        t.release(KEY_RIGHTALT);
        assert_eq!(text(&mut t, KEY_A), Output::Text("a".into()));

        let map = t.keymap().reverse_map();
        assert_eq!(map[&'a'], Stroke { code: KEY_A, shift: false, altgr: false });
        assert_eq!(map[&'A'], Stroke { code: KEY_A, shift: true, altgr: false });
        // AltGr+Q and AltGr+ò both type @ in the xkb Italian layout
        assert!(matches!(map[&'@'], Stroke { code: KEY_Q | KEY_SEMICOLON, shift: false, altgr: true }));
        assert_eq!(map[&'"'], Stroke { code: KEY_2, shift: true, altgr: false });
        assert_eq!(map[&'è'].code, KEY_LEFTBRACE);
        assert!(!map.contains_key(&'😀'));
    }

    #[test]
    fn azerty_shortcut_letters() {
        let Some(t) = translator("fr", "") else { return };
        // the key in the position of Q on a QWERTY keyboard types "a" in French
        assert_eq!(t.keymap().base_char(KEY_Q), Some('a'));
        let map = t.keymap().reverse_map();
        assert_eq!(map[&'a'].code, KEY_Q);
    }

    #[test]
    fn us_layout_has_no_altgr_level() {
        let Some(t) = translator("us", "") else { return };
        let map = t.keymap().reverse_map();
        assert_eq!(map[&'@'], Stroke { code: KEY_2, shift: true, altgr: false });
        assert!(map.values().all(|s| !s.altgr), "us has no AltGr level");
    }

    #[test]
    fn dead_keys_compose() {
        let Some(mut t) = translator("fr", "") else { return };
        if t.compose.is_none() {
            return eprintln!("no compose table");
        }
        // French: the key right of P is the dead circumflex
        assert_eq!(text(&mut t, KEY_LEFTBRACE), Output::Composing);
        assert_eq!(text(&mut t, KEY_E), Output::Text("ê".into()));
    }
}
