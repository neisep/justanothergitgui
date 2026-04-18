mod app;
mod commit_rules;
mod core;
mod git_ops;
mod infra;
mod logging;
mod settings;
mod shared;
mod state;
mod ui;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Just Another Git GUI",
        options,
        Box::new(|cc| Ok(Box::new(app::GitGuiApp::new(cc)))),
    )
}
