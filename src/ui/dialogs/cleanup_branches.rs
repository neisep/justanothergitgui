use eframe::egui;

use crate::state::AppState;

pub struct CleanupBranchesDialogOutput {
    pub keep_open: bool,
    pub delete_requested: bool,
}

pub fn show(ctx: &egui::Context, state: &mut AppState) -> CleanupBranchesDialogOutput {
    let mut keep_open = state.show_cleanup_branches_dialog;
    let mut close_requested = false;
    let mut delete_requested = false;

    egui::Window::new("Clean up branches")
        .id(egui::Id::new("cleanup_branches_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            if state.stale_branches.is_empty() {
                ui.label("No stale branches to clean up.");
                ui.add_space(4.0);
                ui.weak(
                    "A branch is listed here when its upstream has been deleted on the remote.\nPull first to refresh remote tracking.",
                );
            } else {
                ui.label("These branches no longer exist on the remote:");
                ui.add_space(6.0);

                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for branch in state.stale_branches.iter_mut() {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut branch.selected, &branch.name);
                                if branch.merged_into_head {
                                    ui.weak("merged");
                                } else {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(220, 180, 100),
                                        "unmerged — commits may be lost",
                                    );
                                }
                            });
                        }
                    });

                ui.add_space(8.0);
                let any_selected = state.stale_branches.iter().any(|branch| branch.selected);
                let any_unmerged_selected = state
                    .stale_branches
                    .iter()
                    .any(|branch| branch.selected && !branch.merged_into_head);

                if any_unmerged_selected {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 120, 120),
                        "Warning: deleting an unmerged branch loses its local commits.",
                    );
                    ui.add_space(4.0);
                }

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(any_selected, egui::Button::new("Delete Selected"))
                            .clicked()
                        {
                            delete_requested = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_requested = true;
                        }
                    });
                });
            }
        });

    if close_requested {
        keep_open = false;
    }

    CleanupBranchesDialogOutput {
        keep_open,
        delete_requested,
    }
}
