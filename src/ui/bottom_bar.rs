use std::path::Path;

use eframe::egui;

pub struct BottomBarView<'a> {
    pub repo_path: Option<&'a Path>,
    pub status_msg: &'a str,
}

pub fn show(ui: &mut egui::Ui, view: BottomBarView<'_>, has_logs: bool) -> bool {
    let mut open_logs = false;
    egui::Panel::bottom("bottom_bar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(path) = view.repo_path {
                ui.label(path.display().to_string());
            } else {
                ui.weak("No repository open");
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if has_logs && ui.small_button("Logs").clicked() {
                    open_logs = true;
                }
                ui.label(view.status_msg);
            });
        });
    });
    open_logs
}
