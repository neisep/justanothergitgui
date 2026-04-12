use eframe::egui;

use crate::commit_rules::{self, CommitMessageRuleSet};
use crate::state::{AppState, UiAction};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    ruleset: CommitMessageRuleSet,
    custom_scopes: &[String],
) {
    egui::Panel::right("commit_panel")
        .default_size(260.0)
        .min_size(180.0)
        .show_inside(ui, |ui| {
            ui.strong("Commit");
            ui.separator();

            ui.label("Message:");
            egui::ScrollArea::vertical()
                .id_salt("commit_msg_scroll")
                .max_height(150.0)
                .show(ui, |ui| {
                    let inferred_scopes = state.inferred_commit_scopes.clone();
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut state.commit_msg)
                            .desired_width(f32::INFINITY)
                            .hint_text("Describe your changes...")
                            .desired_rows(6),
                    );
                    response.context_menu(|ui| {
                        if ruleset == CommitMessageRuleSet::Off {
                            ui.weak(
                                "Enable a commit message ruleset in Settings to insert a prefix.",
                            );
                            return;
                        }

                        ui.label("Insert prefix");
                        ui.separator();
                        for prefix in ruleset.prefixes() {
                            if ui.button(*prefix).clicked() {
                                commit_rules::apply_prefix(ruleset, &mut state.commit_msg, prefix);
                                ui.close();
                            }
                        }
                    });
                    show_prefix_suggestions(
                        ui,
                        &mut state.commit_msg,
                        ruleset,
                        &inferred_scopes,
                        custom_scopes,
                        response.has_focus(),
                    );
                });

            ui.add_space(8.0);
            let validation_error = commit_rules::validation_error(ruleset, &state.commit_msg);

            if let Some(error) = &validation_error {
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
                ui.add_space(8.0);
            } else if let Some(description) = ruleset.description() {
                ui.weak(description);
                ui.add_space(8.0);
            }

            let can_commit = !state.staged.is_empty()
                && !state.commit_msg.trim().is_empty()
                && validation_error.is_none();

            ui.add_enabled_ui(can_commit, |ui| {
                if ui
                    .button("Commit")
                    .on_hover_text(if can_commit {
                        "Create a commit with staged changes"
                    } else if validation_error.is_some() {
                        "Fix the commit message format first"
                    } else if state.staged.is_empty() {
                        "Stage files first"
                    } else {
                        "Enter a commit message"
                    })
                    .clicked()
                {
                    state.actions.push(UiAction::Commit);
                }
            });

            ui.add_space(16.0);
            ui.separator();
            ui.weak(format!("{} file(s) staged", state.staged.len()));
        });
}

pub fn show_prefix_suggestions(
    ui: &mut egui::Ui,
    message: &mut String,
    ruleset: CommitMessageRuleSet,
    inferred_scopes: &[String],
    custom_scopes: &[String],
    input_has_focus: bool,
) {
    if !input_has_focus {
        return;
    }

    let suggestions =
        commit_rules::prefix_suggestions(ruleset, message, inferred_scopes, custom_scopes);
    if suggestions.is_empty() {
        return;
    }

    ui.add_space(4.0);
    egui::Frame::popup(ui.style()).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.weak("Suggestions");
        ui.separator();
        for suggestion in suggestions {
            if ui.button(&suggestion).clicked() {
                commit_rules::apply_prefix(ruleset, message, &suggestion);
            }
        }
    });
}
