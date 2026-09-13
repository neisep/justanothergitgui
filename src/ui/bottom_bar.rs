use std::path::Path;

use eframe::egui;

use crate::state::{StatusLevel, StatusMessage};

const ERROR_TEXT: egui::Color32 = egui::Color32::from_rgb(230, 120, 120);
const SUCCESS_TEXT: egui::Color32 = egui::Color32::from_rgb(120, 200, 130);

pub struct BottomBarView<'a> {
    pub repo_path: Option<&'a Path>,
    pub status: &'a StatusMessage,
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
                show_status(ui, view.status);
            });
        });
    });
    open_logs
}

/// Paint the status line at the weight its severity deserves: a failure has to
/// read differently from "Refreshed repository status", which is the one thing
/// a single grey label cannot do.
fn show_status(ui: &mut egui::Ui, status: &StatusMessage) {
    match status.level() {
        StatusLevel::Error => {
            ui.colored_label(ERROR_TEXT, format!("\u{26a0} {}", status.text()));
        }
        StatusLevel::Success => {
            ui.colored_label(SUCCESS_TEXT, status.text());
        }
        StatusLevel::Info => {
            ui.label(status.text());
        }
    }
}
