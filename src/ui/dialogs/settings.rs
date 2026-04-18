use eframe::egui;

use crate::app::SettingsDialogState;
use crate::commit_rules::{self, CommitMessageRuleSet};

pub struct SettingsDialogOutput {
    pub keep_open: bool,
    pub selected_ruleset: CommitMessageRuleSet,
    pub custom_scope_error: Option<String>,
}

pub fn show(
    ctx: &egui::Context,
    dialog: &mut SettingsDialogState,
    current_ruleset: CommitMessageRuleSet,
) -> SettingsDialogOutput {
    let mut keep_open = dialog.show;
    let mut selected_ruleset = current_ruleset;
    let mut custom_scope_error = None;
    let mut close_requested = false;

    egui::Window::new("Settings")
        .id(egui::Id::new("settings_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label("Commit message rules");
            egui::ComboBox::from_id_salt("commit_message_ruleset")
                .selected_text(selected_ruleset.display_name())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut selected_ruleset,
                        CommitMessageRuleSet::Off,
                        CommitMessageRuleSet::Off.display_name(),
                    );
                    ui.selectable_value(
                        &mut selected_ruleset,
                        CommitMessageRuleSet::ConventionalCommits,
                        CommitMessageRuleSet::ConventionalCommits.display_name(),
                    );
                });

            ui.add_space(8.0);
            if let Some(description) = selected_ruleset.description() {
                ui.weak(description);
                ui.add_space(6.0);
                ui.weak("Type a prefix like `fix` to get scope suggestions.");
            } else {
                ui.weak("Leave this off to allow any commit message format.");
            }

            ui.add_space(10.0);
            ui.label("Custom commit scopes");
            let response = ui.add(
                egui::TextEdit::singleline(&mut dialog.custom_scopes_input)
                    .desired_width(320.0)
                    .hint_text("ui, settings, worker"),
            );
            if dialog.focus_custom_scopes_requested {
                response.request_focus();
                dialog.focus_custom_scopes_requested = false;
            }

            match commit_rules::parse_custom_scopes(&dialog.custom_scopes_input) {
                Ok(scopes) => {
                    if scopes.is_empty() {
                        ui.weak(
                            "Optional. Add comma-separated scopes to keep them available in autocomplete.",
                        );
                    } else {
                        ui.weak("Custom scopes stay available alongside inferred scopes.");
                    }
                }
                Err(error) => {
                    custom_scope_error = Some(error.clone());
                    ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
                }
            }

            if !dialog.status.is_empty() {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), &dialog.status);
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        keep_open = false;
    }

    SettingsDialogOutput {
        keep_open,
        selected_ruleset,
        custom_scope_error,
    }
}
