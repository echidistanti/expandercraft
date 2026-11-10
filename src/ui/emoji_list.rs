//! The list of the emoji shortcodes, searchable by code, name or category. Clicking a row copies its code.

use egui::{Align, Layout, RichText, Sense, Shape};

use super::ExpanderApp;
use super::style::{self, muted};
use crate::emoji::EmojiEntry as Item;
use crate::i18n::tr;

#[derive(Default)]
pub struct EmojiListDialog {
    search: String,
    group: Option<emojis::Group>,
    copied: Option<String>,
    focused: bool,
}

fn group_label(group: emojis::Group) -> &'static str {
    use emojis::Group::*;
    match group {
        SmileysAndEmotion => tr("emoji-group-smileys"),
        PeopleAndBody => tr("emoji-group-people"),
        AnimalsAndNature => tr("emoji-group-animals"),
        FoodAndDrink => tr("emoji-group-food"),
        TravelAndPlaces => tr("emoji-group-travel"),
        Activities => tr("emoji-group-activities"),
        Objects => tr("emoji-group-objects"),
        Symbols => tr("emoji-group-symbols"),
        Flags => tr("emoji-group-flags"),
    }
}

/// The items matching a search (code or English name, case insensitive) and a category.
fn filter<'a>(items: &'a [Item], search: &str, group: Option<emojis::Group>, delimiters: (&str, &str)) -> Vec<&'a Item> {
    let mut needle = search.trim().to_lowercase();
    for d in [delimiters.0, delimiters.1, ":"] {
        if !d.is_empty() {
            needle = needle.trim_start_matches(d).trim_end_matches(d).to_string();
        }
    }
    let needle_spaced = needle.replace('_', " ");
    items
        .iter()
        .filter(|i| group.is_none_or(|g| i.group == g))
        .filter(|i| needle.is_empty() || i.codes.iter().any(|c| c.contains(&needle)) || i.name.contains(&needle_spaced))
        .collect()
}

impl ExpanderApp {
    pub(super) fn emoji_list_ui(&mut self, ctx: &egui::Context, d: &mut EmojiListDialog) -> bool {
        let (left, right, enabled) = {
            let p = self.shared.prefs.read();
            (p.emoji_left_delimiter.clone(), p.emoji_right_delimiter.clone(), p.emoji_shortcodes_enabled)
        };
        let code = |c: &str| format!("{left}{c}{right}");
        let found = filter(crate::emoji::all(), &d.search, d.group, (&left, &right));
        let mut close = false;
        let r = super::dialogs::modal("emoji_list").show(ctx, |ui| {
            ui.set_width(660.0);
            ui.heading(tr("emoji-list-title"));
            ui.add_space(4.0);
            ui.label(muted(ui, tr!("emoji-list-intro", example = code("smile"))));
            if !enabled {
                ui.label(RichText::new(tr("emoji-list-disabled")).color(style::WARNING).small());
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let edit = ui.add(egui::TextEdit::singleline(&mut d.search).hint_text(tr("emoji-list-search")).desired_width(330.0));
                if !d.focused {
                    edit.request_focus();
                    d.focused = true;
                }
                egui::ComboBox::from_id_salt("emoji_group")
                    .selected_text(d.group.map_or(tr("emoji-group-all"), group_label))
                    .width(190.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.group, None, tr("emoji-group-all"));
                        for g in emojis::Group::iter() {
                            ui.selectable_value(&mut d.group, Some(g), group_label(g));
                        }
                    });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(muted(ui, tr!("emoji-list-count", count = found.len())));
                });
            });
            ui.add_space(4.0);

            let row_height = 28.0;
            let accent = style::accent(ui);
            egui::ScrollArea::vertical().max_height(380.0).auto_shrink([false, false]).show_rows(
                ui,
                row_height,
                found.len(),
                |ui, range| {
                    for item in &found[range] {
                        let background = ui.painter().add(Shape::Noop);
                        let row = ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), row_height),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.add_space(6.0);
                                ui.add_sized([30.0, row_height], egui::Label::new(RichText::new(item.emoji).size(20.0)).selectable(false));
                                ui.allocate_ui_with_layout(egui::vec2(210.0, row_height), Layout::left_to_right(Align::Center), |ui| {
                                    ui.set_min_width(210.0);
                                    ui.add(
                                        egui::Label::new(RichText::new(code(item.codes[0])).monospace().color(accent))
                                            .selectable(false)
                                            .truncate(),
                                    );
                                });
                                let others: Vec<String> = item.codes[1..].iter().map(|c| code(c)).collect();
                                let mut name = item.name.to_string();
                                if !others.is_empty() {
                                    name = format!("{name}  ·  {}", others.join("  "));
                                }
                                ui.add(egui::Label::new(muted(ui, name)).selectable(false).truncate());
                            },
                        );
                        // the whole width of the list is clickable, not only the texts
                        let rect = egui::Rect::from_min_size(row.response.rect.min, egui::vec2(ui.max_rect().width(), row_height));
                        let response = ui
                            .interact(rect, egui::Id::unique(("emoji_row", item.emoji)), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(item.codes.iter().map(|c| code(c)).collect::<Vec<_>>().join("  "));
                        if response.hovered() {
                            ui.painter().set(background, Shape::rect_filled(response.rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill));
                        }
                        if response.clicked() {
                            let text = code(item.codes[0]);
                            ui.ctx().copy_text(text.clone());
                            d.copied = Some(text);
                        }
                    }
                },
            );
            if found.is_empty() {
                ui.label(muted(ui, tr("emoji-list-no-match")));
            }

            ui.add_space(8.0);
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 30.0), Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(tr("app-ok")).min_size(egui::vec2(90.0, 0.0))).clicked() {
                    close = true;
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| match &d.copied {
                    Some(c) => ui.label(RichText::new(tr!("emoji-list-copied", code = c.as_str())).color(accent)),
                    None => ui.label(muted(ui, tr("emoji-list-click-hint"))),
                });
            });
        });
        !(close || r.should_close())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_by_code_name_and_group() {
        let all = crate::emoji::all();
        assert!(all.len() > 1500);
        let codes = |v: Vec<&Item>| v.iter().map(|i| i.codes[0]).collect::<Vec<_>>();
        // by code, with or without delimiters
        assert!(codes(filter(all, "smile", None, ("|", "|"))).contains(&"smile"));
        assert!(codes(filter(all, "|smirk_cat|", None, ("|", "|"))).contains(&"smirk_cat"));
        assert!(codes(filter(all, ":santa:", None, ("|", "|"))).contains(&"santa"));
        // by English name, with spaces or underscores
        assert!(codes(filter(all, "grinning face", None, ("|", "|"))).contains(&"grinning"));
        // by category
        let flags = filter(all, "", Some(emojis::Group::Flags), ("|", "|"));
        assert!(!flags.is_empty() && flags.iter().all(|i| i.group == emojis::Group::Flags));
        assert!(filter(all, "no such emoji at all", None, ("|", "|")).is_empty());
    }
}
