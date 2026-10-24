//! The New Worktree form and the removal confirmation.
//!
//! Render-only, like every dialog here: both return an output struct and the
//! controller in `app/dialogs.rs` turns it into state changes and actions.

use eframe::egui;

use crate::shared::worktrees::LinkedWorktree;
use crate::state::WorktreeDialogState;
use crate::ui;

const DANGER_TEXT: egui::Color32 = egui::Color32::from_rgb(220, 120, 120);
const DANGER_FILL: egui::Color32 = egui::Color32::from_rgb(160, 60, 60);
const DANGER_LABEL: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
const FIELD_LABEL_WIDTH: f32 = 96.0;

pub struct NewWorktreeDialogOutput {
    pub keep_open: bool,
    pub create_requested: bool,
    /// The user asked to pick the destination folder rather than type it.
    pub browse_requested: bool,
}

/// Everything the form needs from the repository, gathered by the controller so
/// this module never touches git.
pub struct NewWorktreeDialogView<'a> {
    /// Local branches offered as the base.
    pub branches: &'a [String],
    /// Why the current input cannot be used, if it cannot.
    pub validation_error: Option<String>,
    /// Whether the typed branch already exists and is free to check out.
    pub reuses_existing_branch: bool,
    pub busy: bool,
    pub busy_label: Option<&'a str>,
}

pub fn show_new_dialog(
    ctx: &egui::Context,
    state: &mut WorktreeDialogState,
    view: NewWorktreeDialogView<'_>,
) -> NewWorktreeDialogOutput {
    let mut keep_open = state.show_new_worktree_dialog;
    let mut close_requested = false;
    let mut create_requested = false;
    let mut browse_requested = false;

    egui::Window::new("New worktree")
        .id(egui::Id::new("new_worktree_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label("A worktree is another checkout of this repository, in its own folder.");
            ui.add_space(10.0);

            ui.add_enabled_ui(!view.busy, |ui| {
                let name_response = labelled_row(ui, "Name", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut state.name)
                            .hint_text("feature-auth")
                            .desired_width(f32::INFINITY),
                    )
                });
                if state.focus_name_requested {
                    name_response.request_focus();
                    state.focus_name_requested = false;
                }
                // Until the user edits the path themselves, it keeps tracking the
                // name, so the common case needs one field filled in, not three.
                if name_response.changed() {
                    if state.branch_follows_name {
                        state.branch = state.name.trim().to_string();
                    }
                    if state.path_follows_name {
                        state.path = proposed_path(&state.path_parent, &state.name);
                    }
                }

                let branch_response = labelled_row(ui, "Branch", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut state.branch)
                            .hint_text("feature/auth")
                            .desired_width(f32::INFINITY),
                    )
                });
                if branch_response.changed() {
                    state.branch_follows_name = false;
                }

                labelled_row(ui, "Base branch", |ui| {
                    let selected = state.base_branch.clone().unwrap_or_else(|| "HEAD".into());
                    egui::ComboBox::from_id_salt("worktree_base_branch")
                        .selected_text(selected)
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut state.base_branch,
                                None,
                                "HEAD (current commit)",
                            );
                            for branch in view.branches {
                                ui.selectable_value(
                                    &mut state.base_branch,
                                    Some(branch.clone()),
                                    branch,
                                );
                            }
                        })
                        .response
                });

                labelled_row(ui, "Folder", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Browse…").clicked() {
                            browse_requested = true;
                        }
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut state.path)
                                .desired_width(ui.available_width()),
                        );
                        if response.changed() {
                            // Once the path is edited by hand it stops following
                            // the name; nothing should overwrite a typed path.
                            state.path_follows_name = false;
                        }
                        response
                    })
                    .inner
                });
            });

            ui.add_space(8.0);

            if let Some(problem) = &view.validation_error {
                ui.colored_label(DANGER_TEXT, problem);
            } else if view.reuses_existing_branch {
                ui.weak(format!(
                    "ⓘ Branch '{}' exists — it will be checked out instead of created.",
                    state.branch.trim()
                ));
            } else {
                // Hold the row so the buttons do not jump as messages come and go.
                ui.weak(" ");
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if let Some(label) = view.busy_label {
                    ui::show_inline_busy(ui, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let ready = !view.busy
                        && view.validation_error.is_none()
                        && !state.name.trim().is_empty()
                        && !state.branch.trim().is_empty()
                        && !state.path.trim().is_empty();
                    if ui
                        .add_enabled(ready, egui::Button::new("Create worktree"))
                        .clicked()
                    {
                        create_requested = true;
                    }
                    if ui
                        .add_enabled(!view.busy, egui::Button::new("Cancel"))
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

    NewWorktreeDialogOutput {
        keep_open,
        create_requested,
        browse_requested,
    }
}

/// Lay one form row out as a fixed-width label plus a field filling the rest, so
/// the four fields line up.
fn labelled_row<R>(ui: &mut egui::Ui, label: &str, field: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut result = None;
    ui.horizontal(|ui| {
        ui.add_sized(
            [FIELD_LABEL_WIDTH, ui.spacing().interact_size.y],
            |ui: &mut egui::Ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.label(label)
                })
                .inner
            },
        );
        result = Some(field(ui));
    });
    ui.add_space(4.0);
    result.expect("field closure runs inside the row")
}

/// The destination a given name proposes inside the chosen parent folder.
///
/// Takes the parent explicitly rather than recovering it from the current path:
/// with an empty name the joined path ends in a separator, and `Path::parent`
/// then strips the parent folder itself, silently moving the destination a level
/// up every time the name field was cleared.
fn proposed_path(parent: &str, name: &str) -> String {
    let parent = parent.trim();
    let name = name.trim();

    if parent.is_empty() {
        return name.to_string();
    }
    if name.is_empty() {
        return parent.to_string();
    }

    std::path::Path::new(parent)
        .join(name)
        .display()
        .to_string()
}

pub struct RemoveWorktreeDialogOutput {
    pub keep_open: bool,
    pub confirm_requested: bool,
}

pub fn show_remove_dialog(
    ctx: &egui::Context,
    worktree: &LinkedWorktree,
    blocker: Option<&str>,
    busy: bool,
    busy_label: Option<&str>,
) -> RemoveWorktreeDialogOutput {
    let mut keep_open = true;
    let mut close_requested = false;
    let mut confirm_requested = false;

    egui::Window::new("Remove worktree")
        .id(egui::Id::new("remove_worktree_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.label(format!(
                "This will delete the worktree '{}' and its folder.",
                worktree.name
            ));
            ui.weak(worktree.path.display().to_string());
            ui.add_space(6.0);
            ui.colored_label(DANGER_TEXT, "This cannot be undone.");
            ui.add_space(10.0);

            ui.label(format!(
                "Branch '{}' is kept — only the checkout is removed.",
                worktree.branch_label()
            ));

            let damage = worktree.status.damage_lines();
            if !damage.is_empty() {
                ui.add_space(10.0);
                ui.colored_label(DANGER_TEXT, "This worktree has uncommitted work:");
                ui.indent("worktree_damage", |ui| {
                    for line in &damage {
                        ui.label(format!("• {line}"));
                    }
                });
            }

            if let Some(blocker) = blocker {
                ui.add_space(10.0);
                ui.colored_label(DANGER_TEXT, blocker);
                ui.weak("Open the worktree in a tab to commit or discard the changes.");
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if let Some(label) = busy_label {
                    ui::show_inline_busy(ui, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let removable = blocker.is_none() && !busy;
                    if ui
                        .add_enabled(
                            removable,
                            egui::Button::new(
                                egui::RichText::new("Remove worktree").color(DANGER_LABEL),
                            )
                            .fill(DANGER_FILL),
                        )
                        .clicked()
                    {
                        confirm_requested = true;
                    }
                    if ui.add_enabled(!busy, egui::Button::new("Cancel")).clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        keep_open = false;
    }

    RemoveWorktreeDialogOutput {
        keep_open,
        confirm_requested,
    }
}

#[cfg(test)]
mod tests {
    use super::proposed_path;

    #[test]
    fn the_name_lands_inside_the_proposed_folder() {
        assert_eq!(
            proposed_path("/home/u/app-worktrees", "feature-auth"),
            "/home/u/app-worktrees/feature-auth"
        );
        // A folder the user picked by hand is used the same way.
        assert_eq!(
            proposed_path("/elsewhere", "feature-auth"),
            "/elsewhere/feature-auth"
        );
    }

    #[test]
    fn clearing_and_retyping_the_name_never_climbs_out_of_the_folder() {
        let parent = "/home/u/app-worktrees";
        assert_eq!(proposed_path(parent, ""), parent);
        // The regression this guards: deriving the parent from the joined path
        // turned "/home/u/app-worktrees/" into "/home/u" and lost the folder.
        assert_eq!(
            proposed_path(parent, "second"),
            "/home/u/app-worktrees/second"
        );
    }

    #[test]
    fn a_name_without_a_folder_is_kept_as_is() {
        assert_eq!(proposed_path("", "new"), "new");
    }
}
