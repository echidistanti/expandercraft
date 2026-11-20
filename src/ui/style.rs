//! Look and feel.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle, Theme, Visuals};

use crate::prefs::Theme as PrefTheme;

pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x0F, 0x76, 0x6E);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x2D, 0xD4, 0xBF);
pub const WARNING: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
pub const DANGER: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26);

pub fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode { ACCENT_DARK } else { ACCENT_LIGHT }
}

pub fn setup(ctx: &egui::Context) {
    // bundled fonts only, so that symbols and emojis look the same everywhere; Hack covers arrows and shapes
    let mut fonts = egui::FontDefinitions::default();
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional)
        && !family.iter().any(|f| f == "Hack")
    {
        family.push("Hack".to_owned());
    }
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style.text_styles.insert(TextStyle::Heading, FontId::proportional(19.0));
        style.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(11.5));
    });
    ctx.set_visuals_of(Theme::Light, light_visuals());
    ctx.set_visuals_of(Theme::Dark, dark_visuals());
}

fn tune(mut v: Visuals, accent: Color32) -> Visuals {
    v.selection.bg_fill = accent.linear_multiply(if v.dark_mode { 0.45 } else { 0.25 });
    v.selection.stroke = Stroke::new(1.0, accent);
    v.hyperlink_color = accent;
    let r = CornerRadius::same(6);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v
}

fn light_visuals() -> Visuals {
    let mut v = tune(Visuals::light(), ACCENT_LIGHT);
    v.panel_fill = Color32::from_rgb(0xF7, 0xF8, 0xF9);
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(0xF1, 0xF3, 0xF4);
    v
}

fn dark_visuals() -> Visuals {
    let mut v = tune(Visuals::dark(), ACCENT_DARK);
    v.panel_fill = Color32::from_rgb(0x1B, 0x1E, 0x22);
    v.extreme_bg_color = Color32::from_rgb(0x13, 0x15, 0x18);
    v.faint_bg_color = Color32::from_rgb(0x22, 0x26, 0x2B);
    v
}

pub fn apply_theme(ctx: &egui::Context, theme: PrefTheme) {
    ctx.set_theme(match theme {
        PrefTheme::System => egui::ThemePreference::System,
        PrefTheme::Light => egui::ThemePreference::Light,
        PrefTheme::Dark => egui::ThemePreference::Dark,
    });
    send_title_bar_theme(ctx);
}

/// Give the title bar of the system of the window being drawn the theme of the app.
pub fn send_title_bar_theme(ctx: &egui::Context) {
    let theme = match ctx.options(|o| o.theme_preference) {
        egui::ThemePreference::Dark => egui::SystemTheme::Dark,
        egui::ThemePreference::Light => egui::SystemTheme::Light,
        egui::ThemePreference::System => egui::SystemTheme::SystemDefault,
    };
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(theme));
}

/// A label with a muted color.
pub fn muted(ui: &egui::Ui, text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).color(ui.visuals().weak_text_color())
}

/// One line preview of a snippet.
pub fn preview(text: &str, max_chars: usize) -> String {
    let flat: String = text.chars().map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c }).collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > max_chars {
        let mut s: String = flat.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        flat
    }
}
