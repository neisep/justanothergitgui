use eframe::egui;

use crate::state::AppState;
use crate::ui;

pub struct CreateTagDialogOutput {
    pub keep_open: bool,
    pub submit_tag: Option<String>,
}

pub fn show(
    ctx: &egui::Context,
    state: &mut AppState,
    can_create_tag: bool,
    github_auth_available: bool,
    create_tag_busy: bool,
    busy_label: Option<&str>,
) -> CreateTagDialogOutput {
    let mut keep_open = state.show_create_tag_dialog;
    let mut close_requested = false;
    let mut submit_tag = None;

    egui::Window::new("Create Tag")
        .id(egui::Id::new("create_tag_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(format!("Current branch: {}", state.branch));
            ui.add_space(6.0);
            ui.label("Tag name");
            let response = ui.add_enabled(
                !create_tag_busy,
                egui::TextEdit::singleline(&mut state.new_tag_name)
                    .desired_width(260.0)
                    .hint_text("v1.0.0.0"),
            );
            if state.focus_new_tag_name_requested {
                response.request_focus();
                state.focus_new_tag_name_requested = false;
            }
            let can_submit = can_create_tag && !state.new_tag_name.trim().is_empty();

            if response.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter))
                && can_submit
            {
                submit_tag = Some(state.new_tag_name.trim().to_string());
            }

            ui.add_space(8.0);
            if can_create_tag {
                if state.has_origin_remote {
                    ui.weak("The tag will be pushed to origin after it is created.");
                } else {
                    ui.weak("No origin remote is configured, so the tag will be local only.");
                }
            } else if state.has_github_https_origin && !github_auth_available {
                ui.weak("Sign in to GitHub before creating tags for this repository.");
            } else {
                ui.weak("Tags can only be created from the main or master branch.");
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if let Some(label) = busy_label {
                    ui::show_inline_busy(ui, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(can_submit && !create_tag_busy, egui::Button::new("Create"))
                        .clicked()
                    {
                        submit_tag = Some(state.new_tag_name.trim().to_string());
                    }

                    if ui
                        .add_enabled(!create_tag_busy, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        keep_open = false;
    }

    CreateTagDialogOutput {
        keep_open,
        submit_tag,
    }
}
