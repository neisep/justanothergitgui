use eframe::egui;

use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &AppState) {
    egui::Panel::bottom("bottom_bar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(path) = &state.repo_path {
                ui.label(path.display().to_string());
            } else {
                ui.weak("No repository open");
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(&state.status_msg);
            });
        });
    });
}
