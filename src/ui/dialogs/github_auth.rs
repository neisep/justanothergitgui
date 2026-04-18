use eframe::egui;

use crate::shared::github::GithubAuthPrompt;

pub struct GithubAuthDialogOutput {
    pub open_github_again_clicked: bool,
}

pub fn show(ctx: &egui::Context, prompt: &GithubAuthPrompt) -> GithubAuthDialogOutput {
    let mut open_github_again_clicked = false;

    egui::Window::new("GitHub Sign In")
        .id(egui::Id::new("github_auth_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label("Enter this code on GitHub to finish signing in:");
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(&prompt.user_code)
                    .monospace()
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label("Verification page");
            ui.hyperlink_to(&prompt.verification_uri, &prompt.verification_uri);
            ui.add_space(8.0);
            if ui.button("Open GitHub Again").clicked() {
                open_github_again_clicked = true;
            }
            ui.add_space(4.0);
            ui.weak("This window closes automatically after sign-in completes.");
        });

    GithubAuthDialogOutput {
        open_github_again_clicked,
    }
}
