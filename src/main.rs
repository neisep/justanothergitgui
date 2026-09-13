#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod commit_rules;
mod core;
mod git_ops;
mod infra;
mod logging;
mod session;
mod settings;
mod shared;
mod state;
#[cfg(test)]
mod testutil;
mod ui;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([640.0, 480.0]),
        centered: cfg!(target_os = "windows"),
        ..Default::default()
    };

    eframe::run_native(
        "Just Another Git GUI",
        options,
        Box::new(|cc| {
            let app = app::GitGuiApp::new(cc);
            cc.egui_ctx.global_style_mut(|style| {
                style.visuals.widgets.noninteractive.fg_stroke.color =
                    eframe::egui::Color32::from_gray(210);
                style.visuals.weak_text_alpha = 0.78;
                style.visuals.widgets.inactive.fg_stroke.color =
                    eframe::egui::Color32::from_gray(225);
                style.spacing.button_padding = eframe::egui::vec2(7.0, 4.0);
            });
            Ok(Box::new(app))
        }),
    )
}
