//! What the selected worktree is working on, beside the Agents list.
//!
//! Takes the Commit panel's slot while the Agents tab is showing, because the
//! two answer different questions and only one of them is ever the question:
//! staging files is about *this* checkout, and this panel is about another one.
//!
//! Read-only. Everything it shows is edited elsewhere — the task and notes in
//! the metadata dialog, the files in the Review tab — so it borrows shared
//! references and emits [`UiAction`]s for the two buttons it has.
//!
//! The metadata and the branch are looked up **live** rather than taken from
//! the selection: a refresh re-reads both, and a copy captured when the row was
//! clicked would contradict the table one column to the left, and would make a
//! metadata edit look as though it had not saved.

use eframe::egui;

use crate::shared::actions::UiAction;
use crate::shared::worktree_metadata::WorktreeMetadata;
use crate::shared::worktrees::LinkedWorktree;
use crate::state::{RepoState, SelectedWorktree, UiState};

use super::worktree_chips::{BROKEN_MARK, review_color, test_color};

const PANEL_DEFAULT_WIDTH: f32 = 300.0;
const PANEL_MIN_WIDTH: f32 = 220.0;
/// Width of the caption column, so every value starts at the same x.
const CAPTION_WIDTH: f32 = 86.0;
const BUTTON_HEIGHT: f32 = 24.0;

pub struct TaskDetailsState<'a> {
    pub repo: &'a RepoState,
    pub selected: Option<&'a SelectedWorktree>,
    /// When the worktree was started, already formatted: reading a clock is the
    /// app layer's job.
    pub started_label: Option<&'a str>,
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, state: TaskDetailsState<'_>) {
    // A separate panel id from the commit panel's, so each remembers the width
    // the user dragged it to rather than inheriting the other's.
    egui::Panel::right("task_details_panel")
        .default_size(PANEL_DEFAULT_WIDTH)
        .min_size(PANEL_MIN_WIDTH)
        .show_inside(ui, |ui| {
            ui.heading("Task Details");
            ui.separator();

            let Some(selected) = state.selected else {
                show_empty(ui);
                return;
            };

            let worktree = state.repo.worktree_by_key(&selected.storage_key);
            let metadata = state.repo.worktree_metadata.get(&selected.storage_key);

            show_header(ui, selected, worktree);
            ui.add_space(6.0);
            show_base_and_totals(ui, selected);
            if let Some(worktree) = worktree {
                field(ui, "Uncommitted", &worktree.status.summary());
            }
            if let Some(started) = state.started_label {
                field(ui, "Started", started);
            }
            ui.add_space(6.0);
            show_states(ui, metadata);
            ui.add_space(6.0);
            show_prose(ui, metadata);

            show_actions(ui, worktree, state.ui_state);
        });
}

fn show_empty(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.35);
        ui.weak("No worktree selected");
        ui.add_space(4.0);
        let weak = ui.visuals().weak_text_color();
        ui.label(
            egui::RichText::new("Pick one in the Agents list to see what it is working on.")
                .small()
                .color(weak),
        );
    });
}

fn show_header(ui: &mut egui::Ui, selected: &SelectedWorktree, worktree: Option<&LinkedWorktree>) {
    ui.add(egui::Label::new(egui::RichText::new(&selected.worktree_name).strong()).truncate());
    if let Some(worktree) = worktree {
        let weak = ui.visuals().weak_text_color();
        ui.add(
            egui::Label::new(
                egui::RichText::new(worktree.branch_label())
                    .small()
                    .color(weak),
            )
            .truncate(),
        );
    }
}

/// The base this worktree is measured from, and how far it has got since.
///
/// A failure is reported in place rather than left blank: a checkout an agent
/// deleted, or one that has not committed anything yet, is exactly the row
/// somebody opened this panel to understand.
fn show_base_and_totals(ui: &mut egui::Ui, selected: &SelectedWorktree) {
    if let Some(base) = &selected.base {
        ui.horizontal(|ui| {
            caption(ui, "Base");
            ui.monospace(egui::RichText::new(&base.short_oid).small());
        });
        let weak = ui.visuals().weak_text_color();
        ui.label(egui::RichText::new(base.description()).small().color(weak));
    }

    if let Some(summary) = &selected.summary {
        if summary.is_empty() {
            let weak = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(summary.label()).small().color(weak));
        } else {
            field(ui, "Changes", &summary.label());
        }
    }

    if let Some(error) = &selected.load_error {
        ui.add_space(2.0);
        ui.add(egui::Label::new(egui::RichText::new(error).small().color(BROKEN_MARK)).wrap());
    }
}

fn show_states(ui: &mut egui::Ui, metadata: Option<&WorktreeMetadata>) {
    let Some(metadata) = metadata else {
        return;
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if metadata.review.is_noteworthy() {
            super::render_pill(ui, metadata.review.label(), review_color(metadata.review));
        }
        if metadata.test.is_noteworthy() {
            super::render_pill(ui, metadata.test.label(), test_color(metadata.test));
        }
    });
}

/// The task and the notes, the two things nothing else has room to show.
fn show_prose(ui: &mut egui::Ui, metadata: Option<&WorktreeMetadata>) {
    let weak = ui.visuals().weak_text_color();

    let task = metadata.map(|metadata| metadata.task.trim()).unwrap_or("");
    ui.label(egui::RichText::new("Task").small().color(weak));
    if task.is_empty() {
        ui.label(
            egui::RichText::new("Nothing recorded yet")
                .small()
                .color(weak),
        );
    } else {
        ui.add(egui::Label::new(task).wrap());
    }

    let notes = metadata.map(|metadata| metadata.notes.trim()).unwrap_or("");
    if !notes.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Notes").small().color(weak));
        // Scrolled, so a long note cannot push the buttons off the panel.
        egui::ScrollArea::vertical()
            .max_height(160.0)
            .show(ui, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(notes).small()).wrap());
            });
    }
}

/// Both buttons only ask; neither reads or writes anything itself.
fn show_actions(ui: &mut egui::Ui, worktree: Option<&LinkedWorktree>, ui_state: &mut UiState) {
    let Some(worktree) = worktree else {
        return;
    };

    // `bottom_up` stacks upwards, so these are added in reverse: the margin ends
    // up lowest, the separator highest.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            // Two equal halves of the row, so the pair reads as one deliberate
            // control strip instead of two buttons of whatever width their
            // labels happened to need.
            let width = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(60.0);
            if ui
                .add_sized([width, BUTTON_HEIGHT], egui::Button::new("View diff"))
                .on_hover_text("See what this worktree has done since its base")
                .clicked()
            {
                ui_state
                    .actions
                    .push(UiAction::review_worktree(worktree.clone()));
            }
            if ui
                .add_sized([width, BUTTON_HEIGHT], egui::Button::new("Edit\u{2026}"))
                .on_hover_text("Edit what is recorded about this worktree")
                .clicked()
            {
                ui_state
                    .actions
                    .push(UiAction::open_worktree_metadata_dialog(worktree.clone()));
            }
        });

        ui.add_space(8.0);
        ui.separator();
    });
}

/// One labelled line: a fixed-width caption, then the value.
fn field(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        caption(ui, label);
        ui.add(egui::Label::new(egui::RichText::new(value).small()).truncate())
            .on_hover_text(value);
    });
}

/// A caption occupying a fixed column, left-aligned inside it.
///
/// Not `add_sized`, which *centres* its content in the box it allocates: that
/// left every caption starting at a different x depending on its length, so the
/// column read as ragged and drifting rightwards.
fn caption(ui: &mut egui::Ui, text: &str) {
    let weak = ui.visuals().weak_text_color();
    ui.allocate_ui_with_layout(
        egui::vec2(CAPTION_WIDTH, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(egui::Label::new(egui::RichText::new(text).small().color(weak)).truncate());
        },
    );
}
