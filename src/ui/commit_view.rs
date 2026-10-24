//! Read-only view of one commit: metadata, the files it touched, and a
//! side-by-side diff of the selected file.
//!
//! Strictly an inspector — it renders state and emits selection actions, and has
//! no way to edit, stage, or write anything.

use eframe::egui;

use crate::shared::actions::UiAction;
use crate::state::{SelectedCommit, UiState};

use super::change_set_view::{self, ChangeListView};

const FILE_LIST_WIDTH: f32 = 240.0;

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
            change_set_view::render_file_list(
                ui,
                &commit.changes,
                ChangeListView {
                    id_salt: "commit_files_table",
                    empty_text: "This commit changed no files.",
                    error_text: "Could not read this commit's files.",
                    on_select: UiAction::select_commit_file,
                },
                ui_state,
            );
        });

    change_set_view::render_diff(
        ui,
        &mut commit.changes,
        "Click any file on the left to see what this commit changed.",
    );
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
