use eframe::egui;

use crate::state::AppState;
use crate::ui;

pub struct DiscardDialogOutput {
    pub keep_open: bool,
    pub confirm_requested: bool,
}

pub fn show(
    ctx: &egui::Context,
    state: &mut AppState,
    discard_busy: bool,
    busy_label: Option<&str>,
) -> DiscardDialogOutput {
    let mut keep_open = state.dialogs.discard.show_discard_dialog;
    let mut close_requested = false;
    let mut confirm_requested = false;
    let branch = state.repo.branch.clone();
    let preview = state
        .dialogs
        .discard
        .discard_preview
        .clone()
        .unwrap_or_default();

    egui::Window::new("Discard local changes")
        .id(egui::Id::new("discard_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(format!(
                "This will hard-reset '{}' to origin/{}.",
                branch, branch
            ));
            ui.add_space(6.0);
            ui.colored_label(
                egui::Color32::from_rgb(220, 120, 120),
                "This cannot be undone.",
            );
            ui.add_space(10.0);

            ui.label("You will lose:");
            ui.indent("discard_damage", |ui| {
                ui.label(format!("• {} modified/staged file(s)", preview.dirty_files));
                ui.label(format!(
                    "• {} local commit(s) not on origin",
                    preview.local_only_commits
                ));
                if state.dialogs.discard.discard_clean_untracked {
                    ui.label(format!(
                        "• {} untracked file(s)/dir(s)",
                        preview.untracked_files
                    ));
                } else {
                    ui.weak(format!(
                        "  ({} untracked file(s)/dir(s) will be kept)",
                        preview.untracked_files
                    ));
                }
            });

            ui.add_space(10.0);
            ui.add_enabled_ui(!discard_busy, |ui| {
                ui.checkbox(
                    &mut state.dialogs.discard.discard_clean_untracked,
                    "Also delete untracked files",
                );
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if let Some(label) = busy_label {
                    ui::show_inline_busy(ui, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            !discard_busy,
                            egui::Button::new(
                                egui::RichText::new("Discard")
                                    .color(egui::Color32::from_rgb(255, 255, 255)),
                            )
                            .fill(egui::Color32::from_rgb(160, 60, 60)),
                        )
                        .clicked()
                    {
                        confirm_requested = true;
                    }

                    if ui
                        .add_enabled(!discard_busy, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        keep_open = false;
    }

    DiscardDialogOutput {
        keep_open,
        confirm_requested,
    }
}
