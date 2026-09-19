//! The worktree metadata form: what a worktree is for, who is on it, and how
//! far review and tests have got.
//!
//! Render-only. Nothing here runs anything — the states are what the user says
//! they saw, and the base commit is read-only because the app recorded it when
//! it created the worktree.

use eframe::egui;

use crate::shared::worktree_metadata::{ReviewState, TestState};
use crate::state::WorktreeMetadataDialogState;

const FIELD_LABEL_WIDTH: f32 = 92.0;
const FIELD_WIDTH: f32 = 300.0;

pub struct WorktreeMetadataDialogOutput {
    pub keep_open: bool,
    pub save_requested: bool,
    pub clear_requested: bool,
}

pub fn show(
    ctx: &egui::Context,
    worktree_name: &str,
    state: &mut WorktreeMetadataDialogState,
    save_error: Option<&str>,
    // `started_label` arrives already formatted: reading a clock is the app
    // layer's job, not a render function's.
    started_label: Option<&str>,
) -> WorktreeMetadataDialogOutput {
    let mut keep_open = true;
    let mut close_requested = false;
    let mut save_requested = false;
    let mut clear_requested = false;

    egui::Window::new(format!("Worktree metadata — {worktree_name}"))
        .id(egui::Id::new("worktree_metadata_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            let task_response = labelled_row(ui, "Task", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.task)
                        .hint_text("What this worktree is for")
                        .desired_width(FIELD_WIDTH),
                )
            });
            if state.focus_task_requested {
                task_response.request_focus();
                state.focus_task_requested = false;
            }

            labelled_row(ui, "Agent", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.agent)
                        .hint_text("claude, codex, a person's name…")
                        .desired_width(FIELD_WIDTH),
                )
            });

            // Prose, so it gets height and its label sits at the top of the
            // field rather than floating halfway down it.
            labelled_row_top(ui, "Notes", |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut state.notes)
                        .hint_text("Anything worth remembering about this worktree")
                        .desired_rows(4)
                        .desired_width(FIELD_WIDTH),
                )
            });

            labelled_row(ui, "Review", |ui| {
                egui::ComboBox::from_id_salt("worktree_metadata_review")
                    .selected_text(state.review.label())
                    .width(FIELD_WIDTH)
                    .show_ui(ui, |ui| {
                        for option in ReviewState::ALL {
                            ui.selectable_value(&mut state.review, option, option.label());
                        }
                    })
                    .response
            });

            labelled_row(ui, "Tests", |ui| {
                egui::ComboBox::from_id_salt("worktree_metadata_test")
                    .selected_text(state.test.label())
                    .width(FIELD_WIDTH)
                    .show_ui(ui, |ui| {
                        for option in TestState::ALL {
                            ui.selectable_value(&mut state.test, option, option.label());
                        }
                    })
                    .response
            });

            // Read-only: the app records this when it creates a worktree, and a
            // commit typed by hand would be worse than nothing.
            labelled_row(ui, "Base commit", |ui| {
                let commit = state.base_commit.trim();
                if commit.is_empty() {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(
                                "— not recorded (worktree created outside the app)",
                            )
                            .small()
                            .color(ui.visuals().weak_text_color()),
                        )
                        .truncate(),
                    )
                } else {
                    ui.add(egui::Label::new(egui::RichText::new(commit).monospace()).truncate())
                }
            });

            // Read-only for the same reason as the base commit: the app knows
            // when it made the worktree, and a date typed by hand would be
            // worse than nothing.
            labelled_row(ui, "Started", |ui| {
                let weak = ui.visuals().weak_text_color();
                let text = match started_label {
                    Some(label) => egui::RichText::new(label).small(),
                    None => egui::RichText::new("\u{2014} not recorded")
                        .small()
                        .color(weak),
                };
                ui.add(egui::Label::new(text).truncate())
            });

            if let Some(error) = save_error {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button("Clear metadata")
                    .on_hover_text("Forget everything recorded about this worktree")
                    .clicked()
                {
                    clear_requested = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Save").clicked() {
                        save_requested = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        keep_open = false;
    }

    WorktreeMetadataDialogOutput {
        keep_open,
        save_requested,
        clear_requested,
    }
}

/// A form row whose field is taller than one line, so the label sits at its top.
fn labelled_row_top<R>(
    ui: &mut egui::Ui,
    label: &str,
    field: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut result = None;
    ui.horizontal_top(|ui| {
        row_label(ui, label);
        result = Some(field(ui));
    });
    result.expect("the row body always runs")
}

/// The label column of a form row: a fixed width, left-aligned inside it.
///
/// Deliberately not a bare `add_sized`, which *centres* its content — that
/// leaves every label starting at a different x depending on its length, so
/// "Task" and "Base commit" do not line up and the column reads as ragged.
fn row_label(ui: &mut egui::Ui, label: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(FIELD_LABEL_WIDTH, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(egui::Label::new(label).truncate());
        },
    );
}

/// One form row: a fixed-width label, then the field.
fn labelled_row<R>(ui: &mut egui::Ui, label: &str, field: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut result = None;
    ui.horizontal(|ui| {
        row_label(ui, label);
        result = Some(field(ui));
    });
    ui.add_space(4.0);
    result.expect("field closure runs inside the row")
}
