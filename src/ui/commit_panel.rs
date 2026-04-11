use eframe::egui;

use crate::state::{AppState, UiAction};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    egui::Panel::right("commit_panel")
        .default_size(260.0)
        .min_size(180.0)
        .show_inside(ui, |ui| {
            ui.strong("Commit");
            ui.separator();

            ui.label("Message:");
            egui::ScrollArea::vertical()
                .id_salt("commit_msg_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.commit_msg)
                            .desired_width(f32::INFINITY)
                            .hint_text("Describe your changes...")
                            .desired_rows(6),
                    );
                });

            ui.add_space(8.0);

            let can_commit = !state.staged.is_empty() && !state.commit_msg.trim().is_empty();

            ui.add_enabled_ui(can_commit, |ui| {
                if ui
                    .button("Commit")
                    .on_hover_text(if can_commit {
                        "Create a commit with staged changes"
                    } else if state.staged.is_empty() {
                        "Stage files first"
                    } else {
                        "Enter a commit message"
                    })
                    .clicked()
                {
                    state.actions.push(UiAction::Commit);
                }
            });

            ui.add_space(16.0);
            ui.separator();
            ui.weak(format!("{} file(s) staged", state.staged.len()));
        });
}
