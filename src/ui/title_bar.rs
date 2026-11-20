//! The title bar of the windows. On Windows the bar of the system follows the theme of the system, so the windows
//! are created without it and the app draws its own: icon, title, buttons, moving, double click to maximize, and
//! resizing at the edges. winit keeps the styles of a normal window under the hood, so Aero Snap, Win+arrows and
//! Alt+Space still work.

use egui::{Align2, Color32, CursorIcon, FontId, Rect, ResizeDirection, Sense, Stroke, ViewportCommand, pos2, vec2};

/// Whether the windows are created without the title bar of the system.
pub const CUSTOM: bool = cfg!(windows);

const HEIGHT: f32 = 32.0;
const BUTTON_WIDTH: f32 = 46.0;
/// Width of the zone along the edges that resizes the window; wider in the corners.
const EDGE: f32 = 5.0;
const CORNER: f32 = 12.0;
/// The red of the close button of Windows 11, under the pointer.
const CLOSE_RED: Color32 = Color32::from_rgb(0xC4, 0x2B, 0x1C);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Buttons {
    /// Minimize, maximize and close, and double click to maximize (the main window).
    All,
    /// Close only (the dialogs).
    Close,
}

#[derive(Clone, Copy, Debug, Hash)]
enum Kind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// The size to create a window with, so that its content keeps `size` under the title bar drawn by the app.
pub fn outer_size(size: egui::Vec2) -> egui::Vec2 {
    if CUSTOM { size + vec2(0.0, HEIGHT) } else { size }
}

/// Show the title bar at the top of the window being drawn, before its other panels. The close button asks the
/// window to close, like the one of the system: the caller handles `close_requested` as before.
pub fn show(ui: &mut egui::Ui, title: &str, buttons: Buttons, resizable: bool) {
    let ctx = ui.ctx().clone();
    if !CUSTOM {
        // the bar of the system, given the theme of the app once per new window (closed viewports are forgotten:
        // a reopened one starts at 0 again)
        if ctx.cumulative_pass_nr() == 0 {
            super::style::send_title_bar_theme(&ctx);
        }
        return;
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let active = ctx.input(|i| i.viewport().focused.unwrap_or(true));
    if !maximized {
        border(ui);
        if resizable {
            resize_edges(&ctx);
        }
    }

    let fill = ui.visuals().panel_fill;
    egui::Panel::top(("title_bar", ctx.viewport_id()))
        .exact_size(HEIGHT)
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE.fill(fill))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            let bar = ui.interact(rect, ui.scope_id().with("drag"), Sense::click_and_drag());
            if bar.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if buttons == Buttons::All && bar.double_clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
            }

            let text = if active { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
            let icon_rect = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(16.0, 16.0));
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            ui.painter().image(icon(&ctx).id(), icon_rect, uv, Color32::WHITE);
            ui.painter().text(
                pos2(icon_rect.right() + 10.0, rect.center().y),
                Align2::LEFT_CENTER,
                title,
                FontId::proportional(12.5),
                text,
            );

            let kinds: &[Kind] = match buttons {
                Buttons::All => &[Kind::Close, if maximized { Kind::Restore } else { Kind::Maximize }, Kind::Minimize],
                Buttons::Close => &[Kind::Close],
            };
            let mut right = rect.right();
            for &kind in kinds {
                let button = Rect::from_min_max(pos2(right - BUTTON_WIDTH, rect.top()), pos2(right, rect.bottom()));
                right -= BUTTON_WIDTH;
                if window_button(ui, button, kind, text).clicked() {
                    ctx.send_viewport_cmd(match kind {
                        Kind::Minimize => ViewportCommand::Minimized(true),
                        Kind::Maximize => ViewportCommand::Maximized(true),
                        Kind::Restore => ViewportCommand::Maximized(false),
                        Kind::Close => ViewportCommand::Close,
                    });
                }
            }
        });
}

fn window_button(ui: &mut egui::Ui, rect: Rect, kind: Kind, text: Color32) -> egui::Response {
    let response = ui.interact(rect, ui.scope_id().with(kind), Sense::click());
    let pressed = response.is_pointer_button_down_on();
    let (bg, fg) = match kind {
        Kind::Close if pressed => (CLOSE_RED.gamma_multiply(0.85), Color32::WHITE),
        Kind::Close if response.hovered() => (CLOSE_RED, Color32::WHITE),
        _ if pressed => (text.gamma_multiply(0.16), text),
        _ if response.hovered() => (text.gamma_multiply(0.09), text),
        _ => (Color32::TRANSPARENT, text),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, bg);
    // the glyphs of Windows 11: 10 points wide, drawn with thin lines
    let stroke = Stroke::new(1.0, fg);
    let c = rect.center();
    let p = |x: f32, y: f32| pos2(c.x + x, c.y + y);
    match kind {
        Kind::Minimize => {
            painter.line_segment([p(-5.0, 0.0), p(5.0, 0.0)], stroke);
        }
        Kind::Maximize => {
            painter.rect_stroke(Rect::from_min_max(p(-5.0, -5.0), p(5.0, 5.0)), 1.0, stroke, egui::StrokeKind::Middle);
        }
        Kind::Restore => {
            painter.rect_stroke(Rect::from_min_max(p(-5.0, -3.0), p(3.0, 5.0)), 1.0, stroke, egui::StrokeKind::Middle);
            painter.add(egui::Shape::line(vec![p(-3.0, -3.0), p(-3.0, -5.0), p(5.0, -5.0), p(5.0, 3.0), p(3.0, 3.0)], stroke));
        }
        Kind::Close => {
            painter.line_segment([p(-5.0, -5.0), p(5.0, 5.0)], stroke);
            painter.line_segment([p(-5.0, 5.0), p(5.0, -5.0)], stroke);
        }
    }
    response
}

/// A thin line around the window, which has no frame of the system any more.
fn border(ui: &egui::Ui) {
    let ctx = ui.ctx();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("window_border")));
    let color = ui.visuals().widgets.noninteractive.bg_stroke.color;
    painter.rect_stroke(ctx.viewport_rect(), 0.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
}

/// Resize the window from its edges, as the frame of the system does: the cursor shows the direction, and a press
/// hands the resizing to the system.
fn resize_edges(ctx: &egui::Context) {
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let Some(direction) = edge_at(ctx.viewport_rect(), pos) else { return };
    ctx.set_cursor_icon(match direction {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

/// The edge or corner of the window `rect` that resizes it from `pos`, if any.
fn edge_at(rect: Rect, pos: egui::Pos2) -> Option<ResizeDirection> {
    let (west, east, north, south) = (pos.x - rect.left(), rect.right() - pos.x, pos.y - rect.top(), rect.bottom() - pos.y);
    if west.min(east).min(north).min(south) >= EDGE {
        return None;
    }
    Some(match (north < CORNER, south < CORNER, west < CORNER, east < CORNER) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (true, _, _, true) => ResizeDirection::NorthEast,
        (_, true, true, _) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        _ if north < EDGE => ResizeDirection::North,
        _ if south < EDGE => ResizeDirection::South,
        _ if west < EDGE => ResizeDirection::West,
        _ => ResizeDirection::East,
    })
}

/// The application icon as a texture, made once and shared by all the windows.
fn icon(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::unique("title_bar_icon");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([32, 32], &crate::icon::render_rgba(32, true));
    let texture = ctx.load_texture("title_bar_icon", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_and_corners() {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let at = |x: f32, y: f32| edge_at(window, pos2(x, y));
        assert_eq!(at(400.0, 300.0), None);
        assert_eq!(at(400.0, 6.0), None);
        assert_eq!(at(400.0, 2.0), Some(ResizeDirection::North));
        assert_eq!(at(400.0, 598.0), Some(ResizeDirection::South));
        assert_eq!(at(1.0, 300.0), Some(ResizeDirection::West));
        assert_eq!(at(799.0, 300.0), Some(ResizeDirection::East));
        // the corners are wider along the edges
        assert_eq!(at(1.0, 1.0), Some(ResizeDirection::NorthWest));
        assert_eq!(at(10.0, 2.0), Some(ResizeDirection::NorthWest));
        assert_eq!(at(798.0, 10.0), Some(ResizeDirection::NorthEast));
        assert_eq!(at(3.0, 590.0), Some(ResizeDirection::SouthWest));
        assert_eq!(at(790.0, 599.0), Some(ResizeDirection::SouthEast));
        // the corner of the close button resizes, like the frame of the system
        assert_eq!(at(796.0, 3.0), Some(ResizeDirection::NorthEast));
    }
}
