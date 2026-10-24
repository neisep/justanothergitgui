//! The Agents tab: every checkout of this repository, and who is working in it.
//!
//! The sidebar's Worktrees section answers "which checkouts exist" in three
//! narrow columns. This answers "what is being done in each of them", at full
//! width, with room for the agent and the states spelled out.
//!
//! Render-only, like every other centre view: it opens no panel of its own and
//! emits [`UiAction`]s rather than touching git.
//!
//! One known rough edge: the Diff button leaves this view for the Review tab,
//! and Review's own Back button then lands on an empty Review rather than
//! returning here. Routing it back would mean recording where the review came
//! from, which is more state than the papercut is worth today.

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::shared::actions::UiAction;
use crate::shared::worktree_metadata::{WorktreeMetadata, WorktreeMetadataMap, storage_key};
use crate::shared::worktrees::{LinkedWorktree, LinkedWorktreeStatus};
use crate::state::UiState;

use super::HoveredRow;
use super::worktree_chips::{BROKEN_MARK, DIRTY_MARK, render_marker, review_color, test_color};

// The centre panel is not the window: the sidebar and the details panel each
// claim their own slot first, leaving roughly 650px at the default window size.
// Seven columns have to fit inside that, so the fixed ones are sized to their
// content rather than to what looks comfortable in isolation, and the two
// elastic ones are allowed to shrink further than is pretty before anything is
// pushed off the right-hand edge.
const AGENT_COL_WIDTH: f32 = 76.0;
const CHANGES_COL_WIDTH: f32 = 104.0;
const TESTS_COL_WIDTH: f32 = 58.0;
/// Wide enough for the longest pill, "Changes requested".
const REVIEW_COL_WIDTH: f32 = 92.0;
/// Wide enough for both buttons. The second one is the first thing to fall off
/// the right-hand edge, and it does so silently.
const ACTIONS_COL_WIDTH: f32 = 124.0;
/// Narrowest the worktree name column is allowed to get.
const NAME_COL_MIN: f32 = 96.0;
/// Narrowest the branch column is allowed to get.
const BRANCH_COL_MIN: f32 = 76.0;
/// Shown wherever a worktree has recorded nothing for a column.
const NOTHING_RECORDED: &str = "-";

pub struct AgentsPanelState<'a> {
    pub worktrees: &'a [LinkedWorktree],
    pub metadata: &'a WorktreeMetadataMap,
    /// The [`storage_key`] of the picked row.
    pub selected_key: Option<&'a str>,
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, state: AgentsPanelState<'_>) {
    if state.worktrees.is_empty() {
        // Only reachable before the first refresh, or when the listing itself
        // failed — a repository always has at least a main working tree.
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.35);
            ui.weak("No worktrees to show");
            ui.add_space(4.0);
            let weak = ui.visuals().weak_text_color();
            ui.label(
                egui::RichText::new("Open a repository, or add a worktree from the sidebar.")
                    .small()
                    .color(weak),
            );
        });
        return;
    }

    show_banner(ui, &state);
    ui.separator();
    show_table(ui, state);
}

/// One line of orientation above the table: how many checkouts there are, and
/// how many of them anyone has claimed.
fn show_banner(ui: &mut egui::Ui, state: &AgentsPanelState<'_>) {
    let total = state.worktrees.len();
    let with_agent = state
        .worktrees
        .iter()
        .filter(|worktree| {
            metadata_for(state.metadata, worktree)
                .is_some_and(|metadata| !metadata.agent.trim().is_empty())
        })
        .count();

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{total} worktree{}",
                if total == 1 { "" } else { "s" }
            ))
            .strong(),
        );
        let weak = ui.visuals().weak_text_color();
        let detail = if with_agent == 0 {
            "no agent recorded yet".to_string()
        } else {
            format!("{with_agent} with an agent recorded")
        };
        ui.label(egui::RichText::new(detail).small().color(weak));
    });
}

fn show_table(ui: &mut egui::Ui, state: AgentsPanelState<'_>) {
    let row_height = ui.spacing().interact_size.y.max(26.0);
    // Copied out so the row bodies can read them while `ui_state` is borrowed
    // mutably for the actions they queue.
    let worktrees = state.worktrees;
    let metadata = state.metadata;
    let selected_key = state.selected_key;

    ui.push_id("agent_worktree_rows", |ui| {
        super::prepare_clickable_rows(ui);
        let mut hover = HoveredRow::load(ui, "hover");

        TableBuilder::new(ui)
            .id_salt("agent_worktree_table")
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(NAME_COL_MIN).clip(true))
            .column(Column::exact(AGENT_COL_WIDTH))
            .column(Column::remainder().at_least(BRANCH_COL_MIN).clip(true))
            .column(Column::exact(CHANGES_COL_WIDTH))
            .column(Column::exact(TESTS_COL_WIDTH))
            .column(Column::exact(REVIEW_COL_WIDTH))
            .column(Column::exact(ACTIONS_COL_WIDTH))
            .header(row_height, |mut header| {
                for title in [
                    "Worktree", "Agent", "Branch", "Changes", "Tests", "Review", "Actions",
                ] {
                    header.col(|ui| {
                        let weak = ui.visuals().weak_text_color();
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(title).small().strong().color(weak),
                            )
                            .truncate(),
                        );
                    });
                }
            })
            .body(|body| {
                body.rows(row_height, worktrees.len(), |mut row| {
                    let index = row.index();
                    let worktree = &worktrees[index];
                    let key = storage_key(worktree);
                    let entry = metadata.get(&key);

                    row.set_selected(selected_key == Some(key.as_str()));
                    row.set_hovered(hover.is_hovered(index));

                    row.col(|ui| render_name(ui, worktree, row_height));
                    row.col(|ui| render_agent(ui, entry));
                    row.col(|ui| render_branch(ui, worktree));
                    row.col(|ui| render_changes(ui, worktree));
                    row.col(|ui| render_test(ui, entry));
                    row.col(|ui| render_review(ui, entry));
                    row.col(|ui| render_actions(ui, worktree, state.ui_state));

                    let row_response = row.response();
                    hover.observe(index, &row_response);
                    row_response
                        .clone()
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if row_response.clicked() {
                        state
                            .ui_state
                            .actions
                            .push(UiAction::select_worktree(worktree.clone()));
                    }
                });
            });

        hover.store(ui);
    });
}

fn metadata_for<'a>(
    metadata: &'a WorktreeMetadataMap,
    worktree: &LinkedWorktree,
) -> Option<&'a WorktreeMetadata> {
    metadata.get(&storage_key(worktree))
}

fn render_name(ui: &mut egui::Ui, worktree: &LinkedWorktree, row_height: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        render_marker(ui, worktree, row_height);

        let mut name = egui::RichText::new(&worktree.name);
        if worktree.is_current {
            name = name.strong();
        }
        ui.add(egui::Label::new(name).truncate())
            .on_hover_text(worktree.path.display().to_string());
    });
}

fn render_agent(ui: &mut egui::Ui, metadata: Option<&WorktreeMetadata>) {
    match metadata.map(|metadata| metadata.agent.trim()) {
        Some(agent) if !agent.is_empty() => {
            ui.add(egui::Label::new(agent).truncate())
                .on_hover_text(agent);
        }
        _ => render_nothing(ui),
    }
}

fn render_branch(ui: &mut egui::Ui, worktree: &LinkedWorktree) {
    let text = egui::RichText::new(worktree.branch_label());
    let text = if worktree.branch.is_some() {
        text
    } else {
        text.color(ui.visuals().weak_text_color())
    };
    ui.add(egui::Label::new(text).truncate());
}

/// Uncommitted work in the checkout — what the listing already counted.
///
/// Not the totals since the base: those need a diff per worktree, which would
/// run for every row on every refresh. They are in the details panel for the
/// one row the user picked, and the tooltip says so, because two different
/// numbers under one word would otherwise be indistinguishable.
fn render_changes(ui: &mut egui::Ui, worktree: &LinkedWorktree) {
    let summary = worktree.status.summary();
    let text = egui::RichText::new(&summary).small();
    let text = match &worktree.status {
        LinkedWorktreeStatus::Dirty { .. } => text.color(DIRTY_MARK),
        LinkedWorktreeStatus::Missing | LinkedWorktreeStatus::Unavailable(_) => {
            text.color(BROKEN_MARK)
        }
        LinkedWorktreeStatus::Clean => text.color(ui.visuals().weak_text_color()),
    };
    ui.add(egui::Label::new(text).truncate()).on_hover_text(
        "Uncommitted work in this checkout. Select the row for totals since its base.",
    );
}

/// The full label, not the sidebar's abbreviation: there is room here.
fn render_test(ui: &mut egui::Ui, metadata: Option<&WorktreeMetadata>) {
    match metadata.map(|metadata| metadata.test) {
        Some(test) if test.is_noteworthy() => {
            super::render_pill(ui, test.label(), test_color(test));
        }
        _ => render_nothing(ui),
    }
}

fn render_review(ui: &mut egui::Ui, metadata: Option<&WorktreeMetadata>) {
    match metadata.map(|metadata| metadata.review) {
        Some(review) if review.is_noteworthy() => {
            super::render_pill(ui, review.label(), review_color(review));
        }
        _ => render_nothing(ui),
    }
}

/// Every row action only *asks*: none of them performs git work here.
fn render_actions(ui: &mut egui::Ui, worktree: &LinkedWorktree, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if ui
            .small_button("Diff")
            .on_hover_text("See what this worktree has done since its base")
            .clicked()
        {
            ui_state
                .actions
                .push(UiAction::review_worktree(worktree.clone()));
        }
        if ui
            .small_button("Edit\u{2026}")
            .on_hover_text("Edit what is recorded about this worktree")
            .clicked()
        {
            ui_state
                .actions
                .push(UiAction::open_worktree_metadata_dialog(worktree.clone()));
        }
    });
}

/// A word, not a glyph: the bundled fonts have no dash character worth relying
/// on, and a missing one paints a box the painted-text tests would accept.
fn render_nothing(ui: &mut egui::Ui) {
    let weak = ui.visuals().weak_text_color();
    ui.label(egui::RichText::new(NOTHING_RECORDED).small().color(weak));
}

/// Total width the columns demand at their narrowest.
///
/// Pulled out so the arithmetic can be tested: the first version of this table
/// asked for 850px inside a 650px panel, and silently pushed the Review and
/// Actions columns off the right-hand edge.
#[cfg(test)]
fn minimum_table_width() -> f32 {
    NAME_COL_MIN
        + AGENT_COL_WIDTH
        + BRANCH_COL_MIN
        + CHANGES_COL_WIDTH
        + TESTS_COL_WIDTH
        + REVIEW_COL_WIDTH
        + ACTIONS_COL_WIDTH
}

#[cfg(test)]
mod tests {
    use super::minimum_table_width;
    use crate::shared::worktree_metadata::{ReviewState, TestState};

    /// What the centre panel actually gets at the default 1280px window once
    /// the file sidebar and the task details panel have claimed their slots.
    const REALISTIC_CENTRE_WIDTH: f32 = 650.0;

    /// Every column has to fit, or the ones on the right are simply not there —
    /// and nothing about the window looks wrong, so nobody finds out.
    #[test]
    fn all_seven_columns_fit_the_centre_panel() {
        assert!(
            minimum_table_width() <= REALISTIC_CENTRE_WIDTH,
            "the columns ask for {}px inside {REALISTIC_CENTRE_WIDTH}px",
            minimum_table_width()
        );
    }

    #[test]
    fn the_default_states_earn_no_pill() {
        // Mirrors the sidebar: an unreviewed, untested worktree says nothing,
        // and a pill claiming otherwise would be noise on every row.
        assert!(!ReviewState::Unreviewed.is_noteworthy());
        assert!(!TestState::Unknown.is_noteworthy());
        assert!(ReviewState::Approved.is_noteworthy());
        assert!(TestState::Failing.is_noteworthy());
    }
}
