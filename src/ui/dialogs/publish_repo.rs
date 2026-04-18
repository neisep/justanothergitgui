use std::path::Path;

use eframe::egui;

use crate::app::PublishRepoDialogState;
use crate::commit_rules::{self, CommitMessageRuleSet};
use crate::shared::github::GithubRepoVisibility;
use crate::ui;

pub struct PublishRepoDialogOutput {
    pub keep_open: bool,
    pub choose_folder_clicked: bool,
    pub sign_in_clicked: bool,
    pub create_clicked: bool,
}

pub fn show(
    ctx: &egui::Context,
    dialog: &mut PublishRepoDialogState,
    worker_busy: bool,
    worker_dispatch_busy: bool,
    busy_label: Option<&str>,
    ruleset: CommitMessageRuleSet,
    custom_scopes: &[String],
) -> PublishRepoDialogOutput {
    let mut keep_open = dialog.show;
    let mut choose_folder_clicked = false;
    let mut sign_in_clicked = false;
    let mut create_clicked = false;
    let mut close_requested = false;

    egui::Window::new("Publish Folder to GitHub")
        .id(egui::Id::new("publish_repo_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.add_enabled_ui(!worker_busy, |ui| {
                ui.label("Folder");
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut dialog.folder_path).desired_width(320.0),
                    );
                    if dialog.focus_folder_requested {
                        response.request_focus();
                        dialog.focus_folder_requested = false;
                    }
                    if ui.button("Choose...").clicked() {
                        choose_folder_clicked = true;
                    }
                });

                ui.add_space(8.0);
                ui.label("Repository name");
                ui.add(
                    egui::TextEdit::singleline(&mut dialog.repo_name)
                        .desired_width(320.0)
                        .hint_text("owner/repository or repository"),
                );

                ui.add_space(8.0);
                ui.label("Initial commit message");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut dialog.commit_message).desired_width(320.0),
                );
                let inferred_scopes = commit_rules::infer_commit_scopes(
                    folder_path_from_text(&dialog.folder_path),
                    std::iter::empty::<&str>(),
                );
                ui::commit_panel::show_prefix_suggestions(
                    ui,
                    &response,
                    &mut dialog.commit_message,
                    ruleset,
                    &inferred_scopes,
                    custom_scopes,
                );
            });

            let commit_message_error =
                commit_rules::validation_error(ruleset, &dialog.commit_message);
            if let Some(error) = &commit_message_error {
                ui.add_space(4.0);
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
            } else if let Some(description) = ruleset.description() {
                ui.add_space(4.0);
                ui.weak(description);
            }

            ui.add_space(8.0);
            ui.add_enabled_ui(!worker_busy, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Visibility");
                    ui.selectable_value(
                        &mut dialog.visibility,
                        GithubRepoVisibility::Private,
                        "Private",
                    );
                    ui.selectable_value(
                        &mut dialog.visibility,
                        GithubRepoVisibility::Public,
                        "Public",
                    );
                });
            });

            ui.add_space(8.0);
            let auth_color = if dialog.github_authenticated {
                egui::Color32::from_rgb(100, 200, 100)
            } else {
                egui::Color32::from_rgb(220, 180, 100)
            };
            ui.colored_label(auth_color, &dialog.github_status);

            if !dialog.operation_status.is_empty() {
                ui.add_space(4.0);
                ui.weak(&dialog.operation_status);
            }

            let can_create = !worker_dispatch_busy
                && dialog.github_authenticated
                && !dialog.folder_path.trim().is_empty()
                && !dialog.repo_name.trim().is_empty()
                && !dialog.commit_message.trim().is_empty()
                && commit_message_error.is_none();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if let Some(label) = busy_label {
                    ui::show_inline_busy(ui, label);
                }
                if ui
                    .add_enabled(
                        !worker_dispatch_busy,
                        egui::Button::new("Sign In with GitHub"),
                    )
                    .clicked()
                {
                    sign_in_clicked = true;
                }

                if ui
                    .add_enabled(can_create, egui::Button::new("Create Repository"))
                    .clicked()
                {
                    create_clicked = true;
                }

                if ui
                    .add_enabled(!worker_busy, egui::Button::new("Cancel"))
                    .clicked()
                {
                    close_requested = true;
                }
            });
        });

    if close_requested {
        keep_open = false;
    }

    PublishRepoDialogOutput {
        keep_open,
        choose_folder_clicked,
        sign_in_clicked,
        create_clicked,
    }
}

fn folder_path_from_text(folder_path: &str) -> Option<&Path> {
    let trimmed = folder_path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Path::new(trimmed))
    }
}
