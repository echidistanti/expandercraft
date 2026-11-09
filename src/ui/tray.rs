//! System tray icon and menu.

use std::sync::Arc;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::app_state::{AppShared, UiMsg};
use crate::i18n::tr;

const TRAY_ICON_SIZE: u32 = 32;

pub struct Tray {
    icon: TrayIcon,
    enabled_item: CheckMenuItem,
    shown_enabled: bool,
}

fn make_icon(active: bool) -> Option<Icon> {
    Icon::from_rgba(crate::icon::render_rgba(TRAY_ICON_SIZE, active), TRAY_ICON_SIZE, TRAY_ICON_SIZE).ok()
}

fn tooltip(enabled: bool) -> String {
    if enabled { "ExpanderCraft".to_string() } else { format!("ExpanderCraft ({})", tr("paused")) }
}

impl Tray {
    /// Create the tray icon. Must be called on the thread running the platform event loop.
    pub fn create(shared: Arc<AppShared>) -> Option<Tray> {
        let enabled = shared.is_enabled();
        let menu = Menu::new();
        let open = MenuItem::with_id("open", tr("Open ExpanderCraft"), true, None);
        let enabled_item = CheckMenuItem::with_id("enabled", tr("Enabled"), true, enabled, None);
        let new_combo = MenuItem::with_id("new_combo", tr("New combo..."), true, None);
        let prefs = MenuItem::with_id("prefs", tr("Preferences..."), true, None);
        let quit = MenuItem::with_id("quit", tr("Quit"), true, None);
        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();
        if let Err(e) = menu.append_items(&[&open, &sep1, &enabled_item, &new_combo, &prefs, &sep2, &quit]) {
            log::warn!("Could not build the tray menu: {e}");
        }

        let s = shared.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.as_ref() {
            "open" => s.ui.send(UiMsg::ShowMainWindow),
            "enabled" => s.set_enabled(!s.is_enabled()),
            "new_combo" => s.ui.send(UiMsg::NewCombo),
            "prefs" => s.ui.send(UiMsg::ShowPreferences),
            "quit" => s.ui.send(UiMsg::Quit),
            _ => {}
        }));
        let s = shared.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| match event {
            TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } => s.ui.send(UiMsg::ShowMainWindow),
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } if !cfg!(target_os = "macos") => {
                s.ui.send(UiMsg::ShowMainWindow)
            }
            _ => {}
        }));

        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip(enabled))
            .with_menu_on_left_click(cfg!(target_os = "macos"));
        if let Some(icon) = make_icon(enabled) {
            builder = builder.with_icon(icon);
        }
        match builder.build() {
            Ok(icon) => Some(Tray { icon, enabled_item, shown_enabled: enabled }),
            Err(e) => {
                log::warn!("Could not create the tray icon: {e}");
                None
            }
        }
    }

    pub fn update(&mut self, enabled: bool) {
        if self.shown_enabled == enabled && self.enabled_item.is_checked() == enabled {
            return;
        }
        self.shown_enabled = enabled;
        self.enabled_item.set_checked(enabled);
        let _ = self.icon.set_icon(make_icon(enabled));
        let _ = self.icon.set_tooltip(Some(tooltip(enabled)));
    }
}

/// On Linux the tray lives in a GTK thread; it polls the enabled state to update itself.
#[cfg(target_os = "linux")]
pub fn spawn_linux_tray(shared: Arc<AppShared>) {
    crate::platform::spawn_tray_thread(move || {
        let mut tray = Tray::create(shared.clone())?;
        gtk::glib::timeout_add_local(std::time::Duration::from_millis(300), move || {
            tray.update(shared.is_enabled());
            gtk::glib::ControlFlow::Continue
        });
        None
    });
}
