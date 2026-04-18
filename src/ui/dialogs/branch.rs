use eframe::egui;

use crate::state::AppState;

pub struct CreateBranchDialogOutput {
    pub keep_open: bool,
    pub submit_branch: Option<String>,
}

pub struct ConfirmCreateBranchDialogOutput {
    pub keep_open: bool,
    pub confirm_requested: bool,
}

pub fn show_create_dialog(
    ctx: &egui::Context,
    state: &mut AppState,
    validation_error: Option<&str>,
) -> CreateBranchDialogOutput {
    let mut keep_open = state.dialogs.branch.show_create_branch_dialog;
    let mut close_requested = false;
    let mut submit_branch = None;

    egui::Window::new("Create Branch")
        .id(egui::Id::new("create_branch_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label("Branch name");
            let response = ui.add(
                egui::TextEdit::singleline(&mut state.dialogs.branch.new_branch_name)
                    .desired_width(260.0)
                    .hint_text("feature/my-branch"),
            );
            if state.dialogs.branch.focus_new_branch_name_requested {
                response.request_focus();
                state.dialogs.branch.focus_new_branch_name_requested = false;
            }

            if let Some(error) = validation_error {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
            }

            let can_create = !state.dialogs.branch.new_branch_name.trim().is_empty()
                && validation_error.is_none();

            if response.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter))
                && can_create
            {
                submit_branch = Some(state.dialogs.branch.new_branch_name.trim().to_string());
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create"))
                        .clicked()
                    {
                        submit_branch =
                            Some(state.dialogs.branch.new_branch_name.trim().to_string());
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

    CreateBranchDialogOutput {
        keep_open,
        submit_branch,
    }
}

pub fn show_confirm_dialog(
    ctx: &egui::Context,
    state: &mut AppState,
) -> ConfirmCreateBranchDialogOutput {
    let mut keep_open = state.dialogs.branch.show_create_branch_confirm;
    let mut close_requested = false;
    let mut confirm_requested = false;
    let current_branch = state.repo.branch.clone();
    let preview = state
        .dialogs
        .branch
        .create_branch_preview
        .clone()
        .unwrap_or_default();

    egui::Window::new("Create branch with uncommitted changes?")
        .id(egui::Id::new("create_branch_confirm_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(format!("New branch: {}", preview.branch_name)).strong());
            ui.add_space(8.0);
            ui.label(format!(
                "Your uncommitted changes will come with you to the new branch. \
                 Nothing is lost on '{}' — the changes simply ride along.",
                current_branch
            ));
            ui.add_space(10.0);

            ui.label("Changes traveling with you:");
            ui.indent("create_branch_preview", |ui| {
                if preview.dirty_files > 0 {
                    ui.label(format!("• {} modified file(s)", preview.dirty_files));
                }
                if preview.staged_files > 0 {
                    ui.label(format!("• {} staged file(s)", preview.staged_files));
                }
                if preview.untracked_files > 0 {
                    ui.label(format!("• {} untracked file(s)", preview.untracked_files));
                }
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Create branch").clicked() {
                        confirm_requested = true;
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

    ConfirmCreateBranchDialogOutput {
        keep_open,
        confirm_requested,
    }
}
