//! Read-only view of one commit: metadata, the files it touched, and a
//! side-by-side diff of the selected file.
//!
//! Strictly an inspector — it renders state and emits selection actions, and has
//! no way to edit, stage, or write anything.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::shared::actions::UiAction;
use crate::shared::diff::{DiffLineKind, parse_diff_rows, to_side_by_side};
use crate::state::{SelectedCommit, UiState};

use super::HoveredRow;
use super::diff_view::{self, SideBySideView};

const FILE_LIST_WIDTH: f32 = 240.0;
const STATUS_COL_WIDTH: f32 = 88.0;

pub struct CommitViewState<'a> {
    pub commit: &'a mut SelectedCommit,
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, state: CommitViewState<'_>) {
    let CommitViewState { commit, ui_state } = state;

    render_header(ui, commit, ui_state);

    egui::Panel::left("commit_files")
        .resizable(true)
        .default_size(FILE_LIST_WIDTH)
        .min_size(160.0)
        .show_inside(ui, |ui| {
            render_file_list(ui, commit, ui_state);
        });

    render_diff(ui, commit);
}

fn render_header(ui: &mut egui::Ui, commit: &SelectedCommit, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        if ui
            .button("\u{2190} Back")
            .on_hover_text("Back to the commit list")
            .clicked()
        {
            ui_state.actions.push(UiAction::close_commit());
        }
        ui.monospace(egui::RichText::new(&commit.short_oid).color(egui::Color32::from_gray(170)));
        ui.weak(&commit.author);
        ui.weak(" \u{2022} ");
        ui.weak(&commit.time);
    });
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(&commit.summary).strong()).truncate());
    });
    ui.separator();
}

fn render_file_list(ui: &mut egui::Ui, commit: &SelectedCommit, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        ui.strong(format!("Files ({})", commit.files.len()));
    });
    ui.separator();

    if commit.files.is_empty() {
        ui.weak("This commit changed no files.");
        return;
    }

    let row_height = ui.spacing().interact_size.y.max(22.0);

    ui.push_id("commit_file_rows", |ui| {
        super::prepare_clickable_rows(ui);
        let mut hover = HoveredRow::load(ui, "hover");

        TableBuilder::new(ui)
            .id_salt("commit_files_table")
            .striped(true)
            .sense(egui::Sense::click())
            .column(Column::remainder().clip(true))
            .column(Column::exact(STATUS_COL_WIDTH))
            .body(|body| {
                body.rows(row_height, commit.files.len(), |mut row| {
                    let index = row.index();
                    let file = &commit.files[index];
                    let is_selected = commit
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
                        ui_state
                            .actions
                            .push(UiAction::select_commit_file(file.path.clone()));
                    }
                });
            });

        hover.store(ui);
    });
}

fn render_diff(ui: &mut egui::Ui, commit: &mut SelectedCommit) {
    let Some(path) = commit.selected_path.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.35);
            ui.weak("Pick a file to inspect");
            ui.add_space(4.0);
            let weak = ui.visuals().weak_text_color();
            ui.label(
                egui::RichText::new("Click any file on the left to see what this commit changed.")
                    .small()
                    .color(weak),
            );
        });
        return;
    };

    let mut rows = parse_diff_rows(&commit.diff_content);
    let added_lines = rows
        .iter()
        .filter(|row| row.kind == DiffLineKind::Added)
        .count();
    let removed_lines = rows
        .iter()
        .filter(|row| row.kind == DiffLineKind::Removed)
        .count();

    // The patch preamble (`diff --git`, `index`, `---`, `+++`) is about the file
    // as a whole, and the path is already in the header above — drop it so both
    // panes start at the first hunk.
    rows.retain(|row| row.kind != DiffLineKind::FileHeader);

    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(&path).strong()).truncate());
        ui.separator();
        ui.weak(format!("+{} / -{}", added_lines, removed_lines));
    });
    ui.separator();

    if commit.diff_content.is_empty() {
        ui.weak("No textual diff available (the file may be binary or empty)");
        return;
    }

    let entries = to_side_by_side(&rows);
    commit.scroll = diff_view::show_side_by_side(
        ui,
        SideBySideView {
            entries: &entries,
            scroll: commit.scroll,
        },
    );
}
