pub mod bottom_bar;
pub mod commit_panel;
pub mod diff_panel;
pub mod file_panel;
pub mod history_panel;

use eframe::egui;

pub fn show_inline_busy(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(12.0));
        ui.label(egui::RichText::new(label).small().weak());
    });
}
