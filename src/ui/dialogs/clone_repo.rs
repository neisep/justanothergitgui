use std::path::PathBuf;

use eframe::egui;

use crate::app::CloneRepoDialogState;
use crate::{git_ops, ui};

pub struct CloneRepoDialogOutput {
    pub keep_open: bool,
    pub choose_folder_clicked: bool,
    pub clone_clicked: bool,
}

pub fn show(
    ctx: &egui::Context,
    dialog: &mut CloneRepoDialogState,
    worker_busy: bool,
    worker_dispatch_busy: bool,
    busy_label: Option<&str>,
    signed_in: bool,
    signed_in_login: &str,
) -> CloneRepoDialogOutput {
    let mut keep_open = dialog.show;
    let mut choose_folder_clicked = false;
    let mut clone_clicked = false;
    let mut cancel_clicked = false;

    egui::Window::new("Clone Repository")
        .id(egui::Id::new("clone_repo_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.add_enabled_ui(!worker_busy, |ui| {
                ui.label("Clone URL");
                let url_response = ui.add(
                    egui::TextEdit::singleline(&mut dialog.url)
                        .desired_width(420.0)
                        .hint_text("https://github.com/owner/repo.git"),
                );
                if dialog.focus_url_requested {
                    url_response.request_focus();
                    dialog.focus_url_requested = false;
                }

                ui.add_space(8.0);
                ui.label("Destination folder");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog.parent_folder).desired_width(320.0),
                    );
                    if ui.button("Choose...").clicked() {
                        choose_folder_clicked = true;
                    }
                });

                let repo_name = git_ops::repo_name_from_clone_url(&dialog.url);
                let parent_display = dialog.parent_folder.trim();
                if let Some(name) = &repo_name
                    && !parent_display.is_empty()
                {
                    let preview = PathBuf::from(parent_display).join(name);
                    ui.weak(format!("Will clone into: {}", preview.display()));
                } else {
                    ui.weak("Pick a parent folder and paste a URL to see the target path.");
                }
            });

            if signed_in {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.heading("Your GitHub repositories");
                ui.weak(format!("Signed in as @{}", signed_in_login));

                if dialog.github_repos_loading {
                    ui::show_inline_busy(ui, "Loading repositories...");
                } else if let Some(error) = dialog.github_repos_error.clone() {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                } else if dialog.github_repos.is_empty() {
                    ui.weak("No repositories returned.");
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Filter");
                        ui.add(
                            egui::TextEdit::singleline(&mut dialog.filter_text)
                                .desired_width(320.0)
                                .hint_text("Type to filter by name..."),
                        );
                    });

                    let filter = dialog.filter_text.to_ascii_lowercase();
                    let filter = filter.trim();
                    egui::ScrollArea::vertical()
                        .max_height(220.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut chosen_url: Option<String> = None;
                            for repo in &dialog.github_repos {
                                if !filter.is_empty()
                                    && !repo.full_name.to_ascii_lowercase().contains(filter)
                                {
                                    continue;
                                }
                                let selected = dialog.url == repo.clone_url;
                                let mut label = repo.full_name.clone();
                                if repo.private {
                                    label.push_str("  [private]");
                                }
                                let response = ui.selectable_label(selected, label);
                                if response.clicked() {
                                    chosen_url = Some(repo.clone_url.clone());
                                }
                                if let Some(desc) = repo
                                    .description
                                    .as_ref()
                                    .map(|value| value.trim())
                                    .filter(|value| !value.is_empty())
                                {
                                    ui.weak(format!("    {}", desc));
                                }
                            }
                            if let Some(url) = chosen_url {
                                dialog.url = url;
                            }
                        });
                }
            } else {
                ui.add_space(8.0);
                ui.weak(
                    "Tip: sign in to GitHub from the welcome screen to browse your repositories here.",
                );
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let clone_enabled = !worker_dispatch_busy
                    && !dialog.url.trim().is_empty()
                    && !dialog.parent_folder.trim().is_empty();
                if ui
                    .add_enabled(clone_enabled, egui::Button::new("Clone"))
                    .clicked()
                {
                    clone_clicked = true;
                }
                if ui
                    .add_enabled(!worker_busy, egui::Button::new("Cancel"))
                    .clicked()
                {
                    cancel_clicked = true;
                }
                if let Some(label) = busy_label {
                    ui::show_inline_busy(ui, label);
                }
            });

            if !dialog.status.is_empty() {
                ui.add_space(6.0);
                ui.weak(&dialog.status);
            }
        });

    if cancel_clicked {
        keep_open = false;
    }

    CloneRepoDialogOutput {
        keep_open,
        choose_folder_clicked,
        clone_clicked,
    }
}
