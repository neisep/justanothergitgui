use eframe::egui;

pub struct LogViewerDialogOutput {
    pub keep_open: bool,
    pub clear_clicked: bool,
}

pub fn show(
    ctx: &egui::Context,
    open: bool,
    log_path: &str,
    contents: &mut String,
) -> LogViewerDialogOutput {
    let mut keep_open = open;
    let mut clear_clicked = false;

    egui::Window::new("Application Logs")
        .id(egui::Id::new("app_logs_dialog"))
        .default_size(egui::vec2(720.0, 420.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Log file: {}", log_path));
                if ui.button("Clear").clicked() {
                    clear_clicked = true;
                }
            });
            ui.add_space(8.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(contents)
                        .desired_width(f32::INFINITY)
                        .desired_rows(24)
                        .interactive(false)
                        .font(egui::TextStyle::Monospace),
                );
            });
        });

    LogViewerDialogOutput {
        keep_open,
        clear_clicked,
    }
}
