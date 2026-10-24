//! The two halves every read-only inspector shares: a list of changed files and
//! the patch for the one that is open.
//!
//! Both the commit view and the worktree review are "here are the files that
//! differ, here is the patch for the one you picked"; only the header and the
//! wording differ. Keeping one renderer keeps them from drifting apart, and
//! keeps the parts that are easy to get wrong in one place — a table row needs
//! both [`super::prepare_clickable_rows`] and [`super::HoveredRow`] before it is
//! clickable at all.
//!
//! Render-only: it paints state and emits selection actions, and has no way to
//! edit, stage or write anything.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::shared::actions::UiAction;
use crate::state::{ChangeSet, UiState};

use super::HoveredRow;
use super::diff_view::{self, SideBySideView};

const STATUS_COL_WIDTH: f32 = 88.0;
const LOAD_ERROR: egui::Color32 = egui::Color32::from_rgb(240, 140, 120);

/// The wording and behaviour that differ between the inspectors.
pub struct ChangeListView<'a> {
    /// Distinguishes the two tables' egui ids and hover state.
    pub id_salt: &'a str,
    /// Shown when the set is genuinely empty.
    pub empty_text: &'a str,
    /// Shown above the failure detail when the list could not be read.
    pub error_text: &'a str,
    /// What a click on a row should queue.
    pub on_select: fn(String) -> UiAction,
}

pub fn render_file_list(
    ui: &mut egui::Ui,
    changes: &ChangeSet,
    view: ChangeListView<'_>,
    ui_state: &mut UiState,
) {
    ui.horizontal(|ui| {
        ui.strong(format!("Files ({})", changes.files.len()));
    });
    ui.separator();

    // A read failure and a genuinely empty set both leave `files` empty, so the
    // error has to win — otherwise a git error reads as "nothing changed here".
    if let Some(error) = &changes.load_error {
        ui.colored_label(LOAD_ERROR, view.error_text);
        ui.label(
            egui::RichText::new(error)
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        return;
    }

    if changes.files.is_empty() {
        ui.weak(view.empty_text);
        return;
    }

    let row_height = ui.spacing().interact_size.y.max(22.0);

    ui.push_id(view.id_salt, |ui| {
        super::prepare_clickable_rows(ui);
        let mut hover = HoveredRow::load(ui, "hover");

        TableBuilder::new(ui)
            .id_salt(view.id_salt)
            .striped(true)
            .sense(egui::Sense::click())
            .column(Column::remainder().clip(true))
            .column(Column::exact(STATUS_COL_WIDTH))
            .body(|body| {
                body.rows(row_height, changes.files.len(), |mut row| {
                    let index = row.index();
                    let file = &changes.files[index];
                    let is_selected = changes
                        .selected_path
                        .as_ref()
                        .is_some_and(|selected| selected == &file.path);
                    row.set_selected(is_selected);
                    row.set_hovered(hover.is_hovered(index));

                    row.col(|ui| {
                        let label = if is_selected {
                            egui::RichText::new(&file.path).strong()
                        } else {
                            egui::RichText::new(&file.path)
                        };
                        ui.add(egui::Label::new(label).truncate());
                    });

                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            diff_view::render_status_badge(ui, &file.display_status, false);
                        });
                    });

                    let response = row.response();
                    hover.observe(index, &response);
                    response
                        .clone()
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        ui_state.actions.push((view.on_select)(file.path.clone()));
                    }
                });
            });

        hover.store(ui);
    });
}

/// The patch pane: the selected file's name, its line tally, and the two
/// side-by-side panes.
pub fn render_diff(ui: &mut egui::Ui, changes: &mut ChangeSet, empty_hint: &str) {
    let Some(path) = changes.selected_path.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.35);
            ui.weak("Pick a file to inspect");
            ui.add_space(4.0);
            let weak = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(empty_hint).small().color(weak));
        });
        return;
    };

    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(&path).strong()).truncate());
        ui.separator();
        ui.weak(format!(
            "+{} / -{}",
            changes.added_lines, changes.removed_lines
        ));
    });
    ui.separator();

    if changes.diff_content.is_empty() {
        ui.weak("No textual diff available (the file may be binary or empty)");
        return;
    }

    // Already parsed and paired when the file was selected — this only paints.
    changes.scroll = diff_view::show_side_by_side(
        ui,
        SideBySideView {
            entries: &changes.diff_entries,
            scroll: changes.scroll,
        },
    );
}
