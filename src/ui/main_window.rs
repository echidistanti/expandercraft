//! The main window: menu bar, group list, combo table and status bar.

use egui::{Align, Layout, RichText, Sense};
use egui_extras::{Column, TableBuilder};
use uuid::Uuid;

use super::dialogs::{ComboEditor, Confirm, ConfirmAction, Dialog, GroupEditor, ImportDialog, RestoreBackupDialog};
use super::style::{self, muted, preview};
use super::{ExpanderApp, PrefsWindow};
use crate::i18n::tr;
use crate::library::{CaseSensitivity, Library, MatchingMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupFilter {
    All,
    Group(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Enabled,
    Name,
    Keyword,
    Snippet,
    Group,
    LastUsed,
}

/// Actions collected while drawing, applied afterwards (so that no lock is held while drawing).
pub enum Action {
    NewCombo,
    EditCombo(Uuid),
    Duplicate,
    DeleteSelected,
    SetEnabled(Vec<Uuid>, bool),
    SetMatching(MatchingMode),
    SetCase(CaseSensitivity),
    MoveToGroup(Uuid),
    CopySnippet(Uuid),
    NewGroup,
    EditGroup(Uuid),
    DeleteGroup(Uuid),
    ToggleGroup(Uuid),
    MoveGroup(Uuid, isize),
    Import,
    Export(bool),
    Backup,
    Restore,
    RestoreAuto,
    CheatSheet,
    OpenDataFolder,
    OpenLog,
    Preferences,
    About,
    Help,
    Hide,
    Quit,
    ToggleEnabled,
    SelectAll,
    Click(Uuid, egui::Modifiers),
}

struct Row {
    uuid: Uuid,
    enabled: bool,
    usable: bool,
    name: String,
    keyword: String,
    snippet: String,
    group: String,
    last_used: Option<chrono::NaiveDateTime>,
}

impl ExpanderApp {
    pub(super) fn main_window_ui(&mut self, ui: &mut egui::Ui) {
        let mut actions: Vec<Action> = Vec::new();
        self.shortcuts(ui, &mut actions);

        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menu_bar(ui, &mut actions));
        });
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui, &mut actions));
        egui::Panel::left("groups").resizable(true).default_size(210.0).min_size(160.0).show(ui, |ui| {
            self.groups_panel(ui, &mut actions);
        });
        egui::CentralPanel::default().show(ui, |ui| self.combos_panel(ui, &mut actions));

        for a in actions {
            self.apply(ui.ctx(), a);
        }
    }

    fn shortcuts(&mut self, ui: &egui::Ui, actions: &mut Vec<Action>) {
        if self.dialog.is_some() || self.recording.is_some() {
            return;
        }
        let text_focused = ui.ctx().memory(|m| m.focused().is_some());
        ui.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::N) {
                actions.push(Action::NewCombo);
            }
            if i.modifiers.command && i.key_pressed(egui::Key::F) {
                self.focus_search = true;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::D) && !self.selection.is_empty() {
                actions.push(Action::Duplicate);
            }
            if !text_focused {
                if i.key_pressed(egui::Key::Delete) && !self.selection.is_empty() {
                    actions.push(Action::DeleteSelected);
                }
                if i.key_pressed(egui::Key::Enter) && self.selection.len() == 1 {
                    actions.push(Action::EditCombo(self.selection[0]));
                }
                if i.modifiers.command && i.key_pressed(egui::Key::A) {
                    actions.push(Action::SelectAll);
                }
            }
        });
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let has_sel = !self.selection.is_empty();
        ui.menu_button(tr("File"), |ui| {
            if ui.button(tr("Import combos...")).clicked() {
                actions.push(Action::Import);
            }
            if ui.button(tr("Export all combos...")).clicked() {
                actions.push(Action::Export(false));
            }
            if ui.add_enabled(has_sel, egui::Button::new(tr("Export selected combos..."))).clicked() {
                actions.push(Action::Export(true));
            }
            ui.separator();
            if ui.button(tr("Preferences...")).clicked() {
                actions.push(Action::Preferences);
            }
            ui.separator();
            if ui.button(tr("Hide window")).clicked() {
                actions.push(Action::Hide);
            }
            if ui.button(tr("Quit")).clicked() {
                actions.push(Action::Quit);
            }
        });
        ui.menu_button(tr("Combo"), |ui| self.combo_menu(ui, actions, true));
        ui.menu_button(tr("Group"), |ui| {
            if ui.button(tr("New group...")).clicked() {
                actions.push(Action::NewGroup);
            }
            if let super::GroupFilter::Group(g) = self.group_filter {
                if ui.button(tr("Edit group...")).clicked() {
                    actions.push(Action::EditGroup(g));
                }
                if ui.button(tr("Enable / disable group")).clicked() {
                    actions.push(Action::ToggleGroup(g));
                }
                if ui.button(tr("Delete group...")).clicked() {
                    actions.push(Action::DeleteGroup(g));
                }
            }
        });
        ui.menu_button(tr("Tools"), |ui| {
            if ui.button(tr("Back up combo list...")).clicked() {
                actions.push(Action::Backup);
            }
            if ui.button(tr("Restore combo list...")).clicked() {
                actions.push(Action::Restore);
            }
            if ui.button(tr("Restore an automatic backup...")).clicked() {
                actions.push(Action::RestoreAuto);
            }
            ui.separator();
            if ui.button(tr("Generate cheat sheet...")).clicked() {
                actions.push(Action::CheatSheet);
            }
            ui.separator();
            if ui.button(tr("Open data folder")).clicked() {
                actions.push(Action::OpenDataFolder);
            }
            if ui.button(tr("Open log file")).clicked() {
                actions.push(Action::OpenLog);
            }
        });
        ui.menu_button(tr("Help"), |ui| {
            if ui.button(tr("Variables reference")).clicked() {
                actions.push(Action::Help);
            }
            if ui.button(tr("About ExpanderCraft")).clicked() {
                actions.push(Action::About);
            }
        });
    }

    /// Combo actions, shared by the menu bar and the context menu of the table.
    fn combo_menu(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>, include_new: bool) {
        let n = self.selection.len();
        if include_new && ui.button(tr("New combo...")).clicked() {
            actions.push(Action::NewCombo);
            ui.close();
        }
        if ui.add_enabled(n == 1, egui::Button::new(tr("Edit..."))).clicked() {
            actions.push(Action::EditCombo(self.selection[0]));
            ui.close();
        }
        if ui.add_enabled(n > 0, egui::Button::new(tr("Duplicate"))).clicked() {
            actions.push(Action::Duplicate);
            ui.close();
        }
        if ui.add_enabled(n == 1, egui::Button::new(tr("Copy snippet to clipboard"))).clicked() {
            actions.push(Action::CopySnippet(self.selection[0]));
            ui.close();
        }
        ui.separator();
        if ui.add_enabled(n > 0, egui::Button::new(tr("Enable"))).clicked() {
            actions.push(Action::SetEnabled(self.selection.clone(), true));
            ui.close();
        }
        if ui.add_enabled(n > 0, egui::Button::new(tr("Disable"))).clicked() {
            actions.push(Action::SetEnabled(self.selection.clone(), false));
            ui.close();
        }
        ui.add_enabled_ui(n > 0, |ui| {
            ui.menu_button(tr("Matching mode"), |ui| {
                for (m, label) in matching_labels(&self.shared.prefs.read()) {
                    if ui.button(label).clicked() {
                        actions.push(Action::SetMatching(m));
                        ui.close();
                    }
                }
            });
            ui.menu_button(tr("Case sensitivity"), |ui| {
                for (c, label) in case_labels(&self.shared.prefs.read()) {
                    if ui.button(label).clicked() {
                        actions.push(Action::SetCase(c));
                        ui.close();
                    }
                }
            });
            ui.menu_button(tr("Move to group"), |ui| {
                let groups: Vec<(Uuid, String)> = self.shared.library.read().groups.iter().map(|g| (g.uuid, g.name.clone())).collect();
                for (g, name) in groups {
                    if ui.button(name).clicked() {
                        actions.push(Action::MoveToGroup(g));
                        ui.close();
                    }
                }
            });
        });
        ui.separator();
        if ui.add_enabled(n > 0, egui::Button::new(RichText::new(tr("Delete...")).color(style::DANGER))).clicked() {
            actions.push(Action::DeleteSelected);
            ui.close();
        }
    }

    fn groups_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Groups")).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("+").on_hover_text(tr("New group...")).clicked() {
                    actions.push(Action::NewGroup);
                }
            });
        });
        ui.separator();
        let (groups, counts, total): (Vec<(Uuid, String, bool)>, Vec<usize>, usize) = {
            let lib = self.shared.library.read();
            let groups: Vec<_> = lib.groups.iter().map(|g| (g.uuid, g.name.clone(), g.enabled)).collect();
            let counts = groups.iter().map(|(u, _, _)| lib.combos.iter().filter(|c| c.group == Some(*u)).count()).collect();
            (groups, counts, lib.combos.len())
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                let all_selected = self.group_filter == GroupFilter::All;
                if ui.selectable_label(all_selected, format!("{}  ({total})", tr("All combos"))).clicked() {
                    self.group_filter = GroupFilter::All;
                }
                ui.add_space(2.0);
                let len = groups.len();
                for (i, ((uuid, name, enabled), count)) in groups.into_iter().zip(counts).enumerate() {
                    let selected = self.group_filter == GroupFilter::Group(uuid);
                    let mut text = RichText::new(format!("{name}  ({count})"));
                    if !enabled {
                        text = text.color(ui.visuals().weak_text_color()).italics();
                    }
                    let r = ui.selectable_label(selected, text);
                    if r.clicked() {
                        self.group_filter = GroupFilter::Group(uuid);
                        self.selection.clear();
                    }
                    if r.double_clicked() {
                        actions.push(Action::EditGroup(uuid));
                    }
                    r.context_menu(|ui| {
                        if ui.button(tr("Edit group...")).clicked() {
                            actions.push(Action::EditGroup(uuid));
                            ui.close();
                        }
                        if ui.button(if enabled { tr("Disable group") } else { tr("Enable group") }).clicked() {
                            actions.push(Action::ToggleGroup(uuid));
                            ui.close();
                        }
                        if ui.add_enabled(i > 0, egui::Button::new(tr("Move up"))).clicked() {
                            actions.push(Action::MoveGroup(uuid, -1));
                            ui.close();
                        }
                        if ui.add_enabled(i + 1 < len, egui::Button::new(tr("Move down"))).clicked() {
                            actions.push(Action::MoveGroup(uuid, 1));
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(RichText::new(tr("Delete group...")).color(style::DANGER)).clicked() {
                            actions.push(Action::DeleteGroup(uuid));
                            ui.close();
                        }
                    });
                }
            });
        });
    }

    fn rows(&self) -> Vec<Row> {
        let lib = self.shared.library.read();
        let words: Vec<String> = self.search.to_lowercase().split_whitespace().map(str::to_string).collect();
        let mut rows: Vec<Row> = lib
            .combos
            .iter()
            .filter(|c| match self.group_filter {
                GroupFilter::All => true,
                GroupFilter::Group(g) => c.group == Some(g),
            })
            .filter(|c| {
                if words.is_empty() {
                    return true;
                }
                let hay = format!("{}\n{}\n{}\n{}", c.name, c.keyword, c.snippet, c.description).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .map(|c| Row {
                uuid: c.uuid,
                enabled: c.enabled,
                usable: lib.is_usable(c),
                name: c.display_name(),
                keyword: c.keyword.clone(),
                snippet: preview(&c.snippet, 160),
                group: lib.group_name(&c.group),
                last_used: c.last_use,
            })
            .collect();
        let (col, asc) = self.sort;
        rows.sort_by(|a, b| {
            let o = match col {
                SortColumn::Enabled => a.enabled.cmp(&b.enabled),
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Keyword => a.keyword.to_lowercase().cmp(&b.keyword.to_lowercase()),
                SortColumn::Snippet => a.snippet.to_lowercase().cmp(&b.snippet.to_lowercase()),
                SortColumn::Group => a.group.to_lowercase().cmp(&b.group.to_lowercase()),
                SortColumn::LastUsed => a.last_used.cmp(&b.last_used),
            };
            let o = o.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            if asc { o } else { o.reverse() }
        });
        rows
    }

    fn combos_panel(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let search = egui::TextEdit::singleline(&mut self.search).hint_text(tr("Search combos...")).desired_width(280.0);
            let r = ui.add(search);
            if self.focus_search {
                r.request_focus();
                self.focus_search = false;
            }
            if !self.search.is_empty() && ui.small_button("✖").on_hover_text(tr("Clear")).clicked() {
                self.search.clear();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let accent = style::accent(ui);
                let new_btn = egui::Button::new(RichText::new(format!("+ {}", tr("New combo"))).color(egui::Color32::WHITE)).fill(accent);
                if ui.add(new_btn).on_hover_text("Ctrl+N").clicked() {
                    actions.push(Action::NewCombo);
                }
            });
        });
        ui.add_space(4.0);

        let rows = self.rows();
        if rows.is_empty() {
            self.empty_state(ui, actions);
            return;
        }
        let selected = self.selected_set();
        let show_group = self.group_filter == GroupFilter::All;
        let mono = egui::TextStyle::Monospace.resolve(ui.style());
        let accent = style::accent(ui);
        let weak = ui.visuals().weak_text_color();

        let mut table = TableBuilder::new(ui)
            .id_salt("combo_table")
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(28.0))
            .column(Column::initial(180.0).at_least(80.0).clip(true))
            .column(Column::initial(110.0).at_least(60.0).clip(true));
        table = table.column(Column::remainder().at_least(120.0).clip(true));
        if show_group {
            table = table.column(Column::initial(110.0).at_least(60.0).clip(true));
        }
        table = table.column(Column::initial(115.0).at_least(80.0));

        let mut header_click: Option<SortColumn> = None;
        let sort = self.sort;
        let header_label = |ui: &mut egui::Ui, text: &str, col: SortColumn, header_click: &mut Option<SortColumn>| {
            let arrow = if sort.0 == col { if sort.1 { " ⏶" } else { " ⏷" } } else { "" };
            if ui.add(egui::Label::new(RichText::new(format!("{text}{arrow}")).strong()).sense(Sense::click())).clicked() {
                *header_click = Some(col);
            }
        };
        table
            .header(26.0, |mut header| {
                header.col(|ui| header_label(ui, "✔", SortColumn::Enabled, &mut header_click));
                header.col(|ui| header_label(ui, tr("Name"), SortColumn::Name, &mut header_click));
                header.col(|ui| header_label(ui, tr("Keyword"), SortColumn::Keyword, &mut header_click));
                header.col(|ui| header_label(ui, tr("Snippet"), SortColumn::Snippet, &mut header_click));
                if show_group {
                    header.col(|ui| header_label(ui, tr("Group"), SortColumn::Group, &mut header_click));
                }
                header.col(|ui| header_label(ui, tr("Last used"), SortColumn::LastUsed, &mut header_click));
            })
            .body(|body| {
                body.rows(26.0, rows.len(), |mut row| {
                    let r = &rows[row.index()];
                    row.set_selected(selected.contains(&r.uuid));
                    let dim = |t: RichText| if r.usable { t } else { t.color(weak).italics() };
                    row.col(|ui| {
                        let mut enabled = r.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            actions.push(Action::SetEnabled(vec![r.uuid], enabled));
                        }
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(dim(RichText::new(&r.name))).truncate().selectable(false));
                    });
                    row.col(|ui| {
                        let t = if r.keyword.is_empty() {
                            RichText::new("—").color(weak)
                        } else {
                            RichText::new(&r.keyword).font(mono.clone()).color(accent)
                        };
                        ui.add(egui::Label::new(dim(t)).truncate().selectable(false));
                    });
                    row.col(|ui| {
                        ui.add(egui::Label::new(dim(RichText::new(&r.snippet))).truncate().selectable(false));
                    });
                    if show_group {
                        row.col(|ui| {
                            ui.add(egui::Label::new(dim(RichText::new(&r.group))).truncate().selectable(false));
                        });
                    }
                    row.col(|ui| {
                        let t = r.last_used.map(|d| d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|| "—".into());
                        ui.label(RichText::new(t).color(weak).small());
                    });
                    let resp = row.response();
                    if resp.clicked() {
                        actions.push(Action::Click(r.uuid, resp.ctx.input(|i| i.modifiers)));
                    }
                    if resp.double_clicked() {
                        actions.push(Action::EditCombo(r.uuid));
                    }
                    if resp.secondary_clicked() && !selected.contains(&r.uuid) {
                        actions.push(Action::Click(r.uuid, egui::Modifiers::NONE));
                    }
                    resp.context_menu(|ui| self.combo_menu(ui, actions, false));
                });
            });
        if let Some(col) = header_click {
            self.sort = if self.sort.0 == col { (col, !self.sort.1) } else { (col, true) };
        }
        // keep the order of the visible rows for range selection
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::unique("visible_rows"), rows.iter().map(|r| r.uuid).collect::<Vec<_>>()));
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.25);
            if self.search.is_empty() {
                ui.label(RichText::new(tr("No combo yet")).heading());
                ui.add_space(6.0);
                ui.label(muted(ui, tr("A combo associates a short keyword to a snippet. Type the keyword in any application and it is replaced by the snippet.")));
                ui.add_space(12.0);
                if ui.button(format!("+ {}", tr("Create your first combo"))).clicked() {
                    actions.push(Action::NewCombo);
                }
                if ui.button(tr("Import combos...")).clicked() {
                    actions.push(Action::Import);
                }
            } else {
                ui.label(muted(ui, tr("No combo matches the search.")));
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let enabled = self.shared.is_enabled();
            let (dot, text) = if enabled {
                (RichText::new("●").color(style::accent(ui)), tr("Active"))
            } else {
                (RichText::new("●").color(style::WARNING), tr("Paused"))
            };
            ui.label(dot);
            if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text(tr("Click to enable or disable ExpanderCraft")).clicked()
            {
                actions.push(Action::ToggleEnabled);
            }
            ui.separator();
            let (n, groups) = {
                let lib = self.shared.library.read();
                (lib.combos.len(), lib.groups.len())
            };
            ui.label(muted(ui, format!("{n} {} · {groups} {}", tr("combos"), tr("groups"))));
            if let Some(m) = &self.platform_status.message {
                ui.separator();
                ui.label(RichText::new(format!("⚠ {m}")).color(style::WARNING).small());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.shared.paths.portable {
                    ui.label(muted(ui, tr("Portable edition")));
                }
            });
        });
    }

    // ---- actions ----------------------------------------------------------------------------------------------------

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Click(uuid, mods) => self.click_row(ctx, uuid, mods),
            Action::SelectAll => {
                self.selection = ctx.data(|d| d.get_temp::<Vec<Uuid>>(egui::Id::unique("visible_rows"))).unwrap_or_default();
            }
            Action::NewCombo => {
                let group = self.default_group_for_new_combo();
                self.dialog = Some(Dialog::Combo(ComboEditor::new_combo(group)));
            }
            Action::EditCombo(uuid) => {
                if let Some(c) = self.shared.library.read().combo(&uuid).cloned() {
                    self.dialog = Some(Dialog::Combo(ComboEditor::edit(c)));
                }
            }
            Action::Duplicate => {
                let mut new_ids = Vec::new();
                {
                    let mut lib = self.shared.library.write();
                    let dups: Vec<_> = self.selection.iter().filter_map(|u| lib.combo(u)).map(|c| c.duplicate()).collect();
                    for d in dups {
                        new_ids.push(d.uuid);
                        lib.combos.push(d);
                    }
                }
                self.selection = new_ids;
                self.commit_library();
                if self.selection.len() == 1 {
                    let uuid = self.selection[0];
                    self.apply(ctx, Action::EditCombo(uuid));
                }
            }
            Action::DeleteSelected => {
                let n = self.selection.len();
                let text = if n == 1 {
                    tr("Delete the selected combo?").to_string()
                } else {
                    tr("Delete the {} selected combos?").replace("{}", &n.to_string())
                };
                self.dialog = Some(Dialog::Confirm(Confirm::new(text, ConfirmAction::DeleteCombos(self.selection.clone()))));
            }
            Action::SetEnabled(ids, enabled) => {
                {
                    let mut lib = self.shared.library.write();
                    for c in lib.combos.iter_mut().filter(|c| ids.contains(&c.uuid)) {
                        c.enabled = enabled;
                    }
                }
                self.commit_library();
            }
            Action::SetMatching(m) => {
                self.modify_selected(|c| {
                    c.matching_mode = m;
                    c.touch();
                });
            }
            Action::SetCase(cs) => {
                self.modify_selected(|c| {
                    c.case_sensitivity = cs;
                    c.touch();
                });
            }
            Action::MoveToGroup(g) => {
                self.modify_selected(|c| {
                    c.group = Some(g);
                    c.touch();
                });
            }
            Action::CopySnippet(uuid) => {
                if let Some(c) = self.shared.library.read().combo(&uuid) {
                    ctx.copy_text(c.snippet.clone());
                }
            }
            Action::NewGroup => self.dialog = Some(Dialog::Group(GroupEditor::new_group())),
            Action::EditGroup(uuid) => {
                if let Some(g) = self.shared.library.read().group(&uuid).cloned() {
                    self.dialog = Some(Dialog::Group(GroupEditor::edit(g)));
                }
            }
            Action::DeleteGroup(uuid) => {
                let (groups, count, name) = {
                    let lib = self.shared.library.read();
                    (
                        lib.groups.len(),
                        lib.combos.iter().filter(|c| c.group == Some(uuid)).count(),
                        lib.group(&uuid).map(|g| g.name.clone()).unwrap_or_default(),
                    )
                };
                if groups <= 1 {
                    self.info(tr("The last group cannot be deleted."));
                } else {
                    let text = if count == 0 {
                        tr("Delete the group '{name}'?").replace("{name}", &name)
                    } else {
                        tr("Delete the group '{name}' and the {n} combos it contains?")
                            .replace("{name}", &name)
                            .replace("{n}", &count.to_string())
                    };
                    self.dialog = Some(Dialog::Confirm(Confirm::new(text, ConfirmAction::DeleteGroup(uuid))));
                }
            }
            Action::ToggleGroup(uuid) => {
                if let Some(g) = self.shared.library.write().group_mut(&uuid) {
                    g.enabled = !g.enabled;
                }
                self.commit_library();
            }
            Action::MoveGroup(uuid, delta) => {
                {
                    let mut lib = self.shared.library.write();
                    if let Some(i) = lib.groups.iter().position(|g| g.uuid == uuid) {
                        let j = i as isize + delta;
                        if j >= 0 && (j as usize) < lib.groups.len() {
                            lib.groups.swap(i, j as usize);
                        }
                    }
                }
                self.commit_library();
            }
            Action::Import => self.import_file(),
            Action::Export(selected_only) => self.export(selected_only),
            Action::Backup => self.manual_backup(),
            Action::Restore => self.restore_from_file(),
            Action::RestoreAuto => self.dialog = Some(Dialog::RestoreBackup(RestoreBackupDialog::new(&self.shared.backup_dir()))),
            Action::CheatSheet => self.cheat_sheet(),
            Action::OpenDataFolder => crate::platform::open_path(&self.shared.paths.data_dir),
            Action::OpenLog => {
                if let Some(p) = crate::logger::log_path() {
                    crate::platform::open_path(&p);
                }
            }
            Action::Preferences => self.prefs = Some(PrefsWindow::default()),
            Action::About => self.dialog = Some(Dialog::About),
            Action::Help => self.dialog = Some(Dialog::Help),
            Action::Hide => self.hide_main(ctx),
            Action::Quit => self.quit(ctx),
            Action::ToggleEnabled => self.shared.set_enabled(!self.shared.is_enabled()),
        }
    }

    fn click_row(&mut self, ctx: &egui::Context, uuid: Uuid, mods: egui::Modifiers) {
        if mods.shift
            && let Some(anchor) = self.anchor
        {
            let visible = ctx.data(|d| d.get_temp::<Vec<Uuid>>(egui::Id::unique("visible_rows"))).unwrap_or_default();
            if let (Some(a), Some(b)) = (visible.iter().position(|u| *u == anchor), visible.iter().position(|u| *u == uuid)) {
                let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                self.selection = visible[lo..=hi].to_vec();
                return;
            }
        }
        if mods.command {
            if let Some(i) = self.selection.iter().position(|u| *u == uuid) {
                self.selection.remove(i);
            } else {
                self.selection.push(uuid);
            }
        } else {
            self.selection = vec![uuid];
        }
        self.anchor = Some(uuid);
    }

    fn modify_selected(&mut self, f: impl Fn(&mut crate::library::Combo)) {
        {
            let mut lib = self.shared.library.write();
            for c in lib.combos.iter_mut().filter(|c| self.selection.contains(&c.uuid)) {
                f(c);
            }
        }
        self.commit_library();
    }

    // ---- files ------------------------------------------------------------------------------------------------------

    fn file_dialog(&self) -> rfd::FileDialog {
        let d = rfd::FileDialog::new();
        match self.last_dir() {
            Some(dir) => d.set_directory(dir),
            None => d,
        }
    }

    fn import_file(&mut self) {
        let Some(path) = self.file_dialog().add_filter(tr("Combo files"), &["json", "csv"]).add_filter(tr("All files"), &["*"]).pick_file()
        else {
            return;
        };
        self.remember_dir(&path);
        let target = self.default_group_for_new_combo();
        match ImportDialog::open(&self.shared, &path, target) {
            Ok(d) => self.dialog = Some(Dialog::Import(d)),
            Err(e) => self.error(format!("{}\n{e:#}", tr("The file could not be imported."))),
        }
    }

    fn export(&mut self, selected_only: bool) {
        let Some(path) = self
            .file_dialog()
            .set_file_name("combos.json")
            .add_filter("JSON", &["json"])
            .add_filter(tr("CSV (TextExpander compatible)"), &["csv"])
            .save_file()
        else {
            return;
        };
        self.remember_dir(&path);
        let lib = self.shared.library.read().clone();
        let combos: Vec<_> = lib.combos.iter().filter(|c| !selected_only || self.selection.contains(&c.uuid)).cloned().collect();
        let is_csv = path.extension().map(|e| e.eq_ignore_ascii_case("csv")).unwrap_or(false);
        let result =
            if is_csv { Library::export_csv(combos.iter(), &path) } else { Library { combos, groups: Vec::new() }.save(&path, false) };
        match result {
            Ok(()) => self.info(tr("The combos were exported.")),
            Err(e) => self.error(format!("{}\n{e:#}", tr("The combos could not be exported."))),
        }
    }

    fn manual_backup(&mut self) {
        let name = format!("ExpanderCraft backup {}.json", chrono::Local::now().format("%Y-%m-%d"));
        let Some(path) = self.file_dialog().set_file_name(name).add_filter("JSON", &["json"]).save_file() else { return };
        self.remember_dir(&path);
        let r = self.shared.library.read().save(&path, true);
        match r {
            Ok(()) => self.info(tr("The backup was saved.")),
            Err(e) => self.error(format!("{e:#}")),
        }
    }

    fn restore_from_file(&mut self) {
        let Some(path) = self.file_dialog().add_filter("JSON", &["json"]).pick_file() else { return };
        self.remember_dir(&path);
        self.dialog = Some(Dialog::Confirm(Confirm::new(
            tr("All the existing combos and groups will be replaced by the content of the backup file. Continue?").to_string(),
            ConfirmAction::RestoreFrom(path),
        )));
    }

    fn cheat_sheet(&mut self) {
        let Some(path) = self.file_dialog().set_file_name("ExpanderCraft cheat sheet.csv").add_filter("CSV", &["csv"]).save_file() else {
            return;
        };
        self.remember_dir(&path);
        let r = self.shared.library.read().export_cheat_sheet(&path, [tr("Group"), tr("Name"), tr("Keyword"), tr("Snippet")]);
        match r {
            Ok(()) => self.info(tr("The cheat sheet was generated.")),
            Err(e) => self.error(format!("{e:#}")),
        }
    }
}

pub fn matching_labels(prefs: &crate::prefs::Preferences) -> Vec<(MatchingMode, String)> {
    let default = match MatchingMode::Default.resolve(prefs) {
        MatchingMode::Loose => tr("Loose"),
        _ => tr("Strict"),
    };
    vec![
        (MatchingMode::Default, format!("{} ({default})", tr("Default"))),
        (MatchingMode::Strict, tr("Strict").to_string()),
        (MatchingMode::Loose, tr("Loose").to_string()),
    ]
}

pub fn case_labels(prefs: &crate::prefs::Preferences) -> Vec<(CaseSensitivity, String)> {
    let default = match CaseSensitivity::Default.resolve(prefs) {
        CaseSensitivity::CaseInsensitive => tr("Case insensitive"),
        _ => tr("Case sensitive"),
    };
    vec![
        (CaseSensitivity::Default, format!("{} ({default})", tr("Default"))),
        (CaseSensitivity::CaseSensitive, tr("Case sensitive").to_string()),
        (CaseSensitivity::CaseInsensitive, tr("Case insensitive").to_string()),
    ]
}
