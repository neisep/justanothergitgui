use eframe::egui;

use crate::commit_rules::{self, CommitMessageRuleSet};
use crate::shared::actions::UiAction;
use crate::state::AppState;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    ruleset: CommitMessageRuleSet,
    custom_scopes: &[String],
) {
    let merging = state.inspector.conflict_data.is_some();
    let collapse_id = ui.id().with("commit_panel_expanded_during_merge");
    let mut expanded = ui
        .data(|data| data.get_temp::<bool>(collapse_id))
        .unwrap_or(false);
    if merging && !expanded {
        egui::Panel::right("commit_panel_collapsed")
            .exact_size(92.0)
            .show_inside(ui, |ui| {
                if ui.button("Commit…").clicked() {
                    ui.data_mut(|data| data.insert_temp(collapse_id, true));
                }
            });
        return;
    }
    egui::Panel::right("commit_panel")
        .default_size(260.0)
        .min_size(180.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Commit");
                if merging && ui.button("Collapse").clicked() {
                    expanded = false;
                    ui.data_mut(|data| data.insert_temp(collapse_id, expanded));
                }
            });
            ui.separator();

            ui.label("Summary:");
            let inferred_scopes = state.commit.inferred_commit_scopes.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut state.commit.commit_summary)
                    .desired_width(f32::INFINITY)
                    .hint_text("Describe your changes..."),
            );
            if state.commit.focus_commit_summary_requested {
                response.request_focus();
                state.commit.focus_commit_summary_requested = false;
            }
            show_prefix_suggestions(
                ui,
                &response,
                &mut state.commit.commit_summary,
                ruleset,
                &inferred_scopes,
                custom_scopes,
            );

            ui.add_space(8.0);
            ui.label("Body (optional):");
            ui.add(
                egui::TextEdit::multiline(&mut state.commit.commit_body)
                    .desired_width(f32::INFINITY)
                    .hint_text("Explain what changed and why...")
                    .desired_rows(4),
            );

            ui.add_space(8.0);
            let commit_message = commit_rules::build_message(
                &state.commit.commit_summary,
                &state.commit.commit_body,
            );
            let validation_error = commit_rules::validation_error(ruleset, &commit_message);

            if let Some(error) = &validation_error {
                ui.colored_label(egui::Color32::from_rgb(220, 120, 120), error);
                ui.add_space(8.0);
            } else if let Some(description) = ruleset.description() {
                ui.weak(description);
                ui.add_space(8.0);
            }

            let blocked = if state
                .worktree
                .unstaged
                .iter()
                .any(|file| file.is_conflicted)
            {
                Some("Resolve and save all conflicted files first")
            } else if state.worktree.staged.is_empty() {
                Some("Stage files to include in this commit")
            } else if state.commit.commit_summary.trim().is_empty() {
                Some("Enter a commit summary")
            } else if validation_error.is_some() {
                Some("Fix the commit message format first")
            } else {
                None
            };
            let can_commit = blocked.is_none();
            let count = state.worktree.staged.len();
            ui.strong(format!(
                "Committing {count} staged {}",
                if count == 1 { "file" } else { "files" }
            ));
            if let Some(reason) = blocked {
                ui.label(reason);
            }
            ui.add_space(8.0);

            ui.add_enabled_ui(can_commit, |ui| {
                if ui
                    .add(
                        egui::Button::new(format!(
                            "Commit {count} {}",
                            if count == 1 { "file" } else { "files" }
                        ))
                        .fill(if can_commit {
                            egui::Color32::from_rgb(0, 95, 125)
                        } else {
                            ui.visuals().widgets.inactive.bg_fill
                        }),
                    )
                    .on_hover_text(if can_commit {
                        "Create a commit with staged changes\nShortcut: Ctrl/Cmd+Enter"
                    } else if validation_error.is_some() {
                        "Fix the commit message format first"
                    } else if state.worktree.staged.is_empty() {
                        "Stage files first"
                    } else {
                        "Enter a commit summary"
                    })
                    .clicked()
                {
                    state.ui.actions.push(UiAction::commit());
                }
            });
        });
}

pub fn show_prefix_suggestions(
    ui: &mut egui::Ui,
    response: &egui::Response,
    message: &mut String,
    ruleset: CommitMessageRuleSet,
    inferred_scopes: &[String],
    custom_scopes: &[String],
) {
    let suggestions =
        commit_rules::prefix_suggestions(ruleset, message, inferred_scopes, custom_scopes);
    let popup_id = egui::Popup::default_response_id(response);

    if suggestions.is_empty() {
        egui::Popup::close_id(ui.ctx(), popup_id);
        return;
    }

    let should_open = response.has_focus() || egui::Popup::is_id_open(ui.ctx(), popup_id);
    if !should_open {
        egui::Popup::close_id(ui.ctx(), popup_id);
        return;
    }

    egui::Popup::from_response(response)
        .open_memory(Some(egui::SetOpenCommand::Bool(true)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(response.rect.width().max(220.0));
            ui.weak("Suggestions");
            ui.separator();
            for suggestion in suggestions {
                if ui.button(&suggestion).clicked() {
                    commit_rules::apply_prefix(ruleset, message, &suggestion);
                    move_text_cursor_to_subject_end(ui.ctx(), response.id, message);
                    egui::Popup::close_id(ui.ctx(), popup_id);
                    ui.memory_mut(|memory| memory.request_focus(response.id));
                    ui.ctx().request_repaint();
                }
            }
        });
}

fn move_text_cursor_to_subject_end(ctx: &egui::Context, text_edit_id: egui::Id, message: &str) {
    let mut state = egui::TextEdit::load_state(ctx, text_edit_id).unwrap_or_default();
    let cursor_index = message
        .lines()
        .next()
        .map(|line| line.chars().count())
        .unwrap_or_default();

    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(cursor_index),
        )));
    state.store(ctx, text_edit_id);
}
