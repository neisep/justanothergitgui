use eframe::egui;

use crate::shared::actions::{FileActionKind, PendingFileAction};

pub struct FileActionDialogOutput {
    pub keep_open: bool,
    pub confirm_requested: bool,
}

/// Title, explanation and button label for one kind of destructive file action.
struct FileActionCopy {
    title: &'static str,
    explanation: String,
    button: &'static str,
}

fn copy_for(pending: &PendingFileAction) -> FileActionCopy {
    let path = &pending.path;
    match pending.kind {
        FileActionKind::DiscardWorktree => FileActionCopy {
            title: "Discard changes",
            explanation: format!(
                "Restore '{path}' to its staged content. Unsaved edits will be lost."
            ),
            button: "Discard",
        },
        FileActionKind::DiscardStaged => FileActionCopy {
            title: "Discard changes",
            explanation: format!(
                "Restore '{path}' to the committed version. Staged and unsaved edits will be lost."
            ),
            button: "Discard",
        },
        // A staged `new` file reaches this arm too, and git does hold a copy of
        // it: the blob in the index, which the delete drops along with the file
        // on disk. Saying otherwise on the one screen asking for informed
        // consent would be a lie.
        FileActionKind::DeleteUntracked if pending.staged => FileActionCopy {
            title: "Delete file",
            explanation: format!(
                "Delete '{path}' from disk and drop it from the index. The staged version goes with it."
            ),
            button: "Delete",
        },
        FileActionKind::DeleteUntracked => FileActionCopy {
            title: "Delete file",
            explanation: format!("Delete '{path}' from disk. Git has no copy of this file."),
            button: "Delete",
        },
    }
}

pub fn show(ctx: &egui::Context, pending: &PendingFileAction) -> FileActionDialogOutput {
    let mut keep_open = true;
    let mut close_requested = false;
    let mut confirm_requested = false;
    let copy = copy_for(pending);

    egui::Window::new(copy.title)
        .id(egui::Id::new("file_action_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut keep_open)
        .show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(copy.explanation);
            ui.add_space(6.0);
            ui.colored_label(
                egui::Color32::from_rgb(220, 120, 120),
                "This cannot be undone.",
            );
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(copy.button)
                                    .color(egui::Color32::from_rgb(255, 255, 255)),
                            )
                            .fill(egui::Color32::from_rgb(160, 60, 60)),
                        )
                        .clicked()
                    {
                        confirm_requested = true;
                    }

                    if ui.button("Cancel").clicked() {
                        close_requested = true;
                    }
                });
            });
        });

    if close_requested {
        keep_open = false;
    }

    FileActionDialogOutput {
        keep_open,
        confirm_requested,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(staged: bool) -> PendingFileAction {
        PendingFileAction {
            path: "new.txt".to_string(),
            staged,
            kind: FileActionKind::DeleteUntracked,
        }
    }

    #[test]
    fn delete_copy_claims_no_git_copy_only_for_an_unstaged_file() {
        let explanation = copy_for(&pending(false)).explanation;

        assert!(explanation.contains("Git has no copy"));
    }

    #[test]
    fn delete_copy_names_the_index_for_a_staged_new_file() {
        let explanation = copy_for(&pending(true)).explanation;

        assert!(!explanation.contains("Git has no copy"));
        assert!(explanation.contains("index"));
    }
}
