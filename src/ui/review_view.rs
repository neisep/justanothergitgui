//! Read-only view of what a worktree has done since its base commit.
//!
//! The header is all that differs from the commit view: a review is identified
//! by a worktree and a base rather than by a commit, and it has to say which
//! base it is measuring from, because for most worktrees that base was derived
//! rather than recorded.

use eframe::egui;

use crate::shared::actions::UiAction;
use crate::state::{SelectedReview, UiState};

use super::change_set_view::{self, ChangeListView};

const FILE_LIST_WIDTH: f32 = 240.0;
const UNCOMMITTED: egui::Color32 = egui::Color32::from_rgb(230, 180, 90);

pub struct ReviewViewState<'a> {
    pub review: &'a mut SelectedReview,
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, state: ReviewViewState<'_>) {
    let ReviewViewState { review, ui_state } = state;

    render_header(ui, review, ui_state);

    egui::Panel::left("review_files")
        .resizable(true)
        .default_size(FILE_LIST_WIDTH)
        .min_size(160.0)
        .show_inside(ui, |ui| {
            change_set_view::render_file_list(
                ui,
                &review.changes,
                ChangeListView {
                    id_salt: "review_files_table",
                    empty_text: "Nothing has changed in this worktree since its base.",
                    error_text: "Could not read this worktree's changes.",
                    on_select: UiAction::select_review_file,
                },
                ui_state,
            );
        });

    change_set_view::render_diff(
        ui,
        &mut review.changes,
        "Click any file on the left to see what this worktree changed.",
    );
}

fn render_header(ui: &mut egui::Ui, review: &SelectedReview, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        if ui
            .button("\u{2190} Back")
            .on_hover_text("Close this review")
            .clicked()
        {
            ui_state.actions.push(UiAction::close_review());
        }
        ui.add(egui::Label::new(egui::RichText::new(&review.worktree_name).strong()).truncate());
        if let Some(branch) = &review.branch {
            ui.weak(" \u{2022} ");
            ui.add(egui::Label::new(egui::RichText::new(branch).weak()).truncate());
        }
    });

    ui.horizontal(|ui| {
        ui.monospace(
            egui::RichText::new(&review.base.short_oid).color(egui::Color32::from_gray(170)),
        );
        // Which base this is matters: most worktrees have no recorded one, and a
        // derived fork point answers a subtly different question.
        ui.weak(format!("base: {}", review.base.description()));

        if review.uncommitted > 0 {
            ui.weak(" \u{2022} ");
            ui.colored_label(
                UNCOMMITTED,
                format!(
                    "{} uncommitted file{}",
                    review.uncommitted,
                    if review.uncommitted == 1 { "" } else { "s" }
                ),
            );
        }
    });
    ui.separator();
}
