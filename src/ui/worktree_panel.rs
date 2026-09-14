//! The Worktrees section of the left sidebar.
//!
//! Lists every checkout sharing the repository's object store and reports what
//! the user did with it. Render-only: git work leaves as a [`UiAction`], and
//! opening a worktree leaves as a [`WorktreePanelResponse`], because adding a
//! repository tab is the app root's job rather than the active tab's.

use std::path::PathBuf;

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::shared::actions::UiAction;
use crate::shared::worktree_metadata::{
    ReviewState, TestState, WorktreeMetadata, WorktreeMetadataMap, storage_key,
};
use crate::shared::worktrees::{LinkedWorktree, LinkedWorktreeStatus};
use crate::state::UiState;

use super::HoveredRow;

/// Height of the section header plus its separator. Same construct, and so the
/// same measured height, as a file-list section header.
const SECTION_CHROME: f32 = 52.0;
/// The section never takes more than this share of the sidebar on its own.
const MAX_PANEL_FRACTION: f32 = 0.45;
/// Rows the section sizes itself for before the user has to scroll or drag.
const PREFERRED_VISIBLE_ROWS: usize = 4;
/// Width of the painted state marker in front of each name.
const MARKER_DIAMETER: f32 = 10.0;

const CURRENT_MARK: egui::Color32 = egui::Color32::from_rgb(120, 190, 255);
const CLEAN_MARK: egui::Color32 = egui::Color32::from_rgb(120, 190, 130);
const DIRTY_MARK: egui::Color32 = egui::Color32::from_rgb(230, 180, 90);
const BROKEN_MARK: egui::Color32 = egui::Color32::from_rgb(220, 120, 120);
const REVIEW_PENDING: egui::Color32 = egui::Color32::from_rgb(96, 84, 156);
const REVIEW_CHANGES: egui::Color32 = egui::Color32::from_rgb(160, 92, 32);
const REVIEW_APPROVED: egui::Color32 = egui::Color32::from_rgb(48, 112, 80);
const TEST_PASSING: egui::Color32 = egui::Color32::from_rgb(48, 112, 80);
const TEST_FAILING: egui::Color32 = egui::Color32::from_rgb(152, 64, 64);
/// Width of the chip column. Two short pills, or nothing at all.
const CHIP_COL_WIDTH: f32 = 74.0;
/// Height of the detail strip under the table.
const DETAIL_STRIP_HEIGHT: f32 = 20.0;

pub struct WorktreePanelState<'a> {
    pub worktrees: &'a [LinkedWorktree],
    /// What the user recorded about them, keyed by [`storage_key`].
    pub metadata: &'a WorktreeMetadataMap,
    pub ui_state: &'a mut UiState,
}

impl WorktreePanelState<'_> {
    fn metadata_for(&self, worktree: &LinkedWorktree) -> Option<&WorktreeMetadata> {
        self.metadata.get(&storage_key(worktree))
    }
}

/// What the section asks the app root to do. Git operations do not travel here —
/// they go through [`UiAction`] like every other panel's intent.
#[derive(Default)]
pub struct WorktreePanelResponse {
    /// A worktree the user asked to open, as a repository tab.
    pub open: Option<PathBuf>,
}

/// Render the section as a resizable strip at the top of the sidebar.
///
/// Declared before the file sections so it claims the topmost strip — above the
/// file filter — and leaves the Unstaged/Staged split below it untouched.
pub fn show(ui: &mut egui::Ui, mut state: WorktreePanelState<'_>) -> WorktreePanelResponse {
    let mut response = WorktreePanelResponse::default();
    let row_height = ui.spacing().interact_size.y.max(28.0);
    // Rows cost their height plus the gap the table puts between them; sizing on
    // the height alone leaves the last row clipped by whatever follows.
    let row_stride = row_height + ui.spacing().item_spacing.y;
    let default_height = preferred_height(state.worktrees.len(), row_stride, ui.available_height());

    egui::Panel::top("worktrees_section")
        .resizable(true)
        .default_size(default_height)
        .min_size(SECTION_CHROME + row_stride)
        .show_inside(ui, |ui| {
            // egui stores this panel's *content* rect each frame and uses it as
            // next frame's size. Content even slightly taller than the panel
            // therefore makes it grow again next frame, and again — a nested
            // panel here grew the section by 2px per frame without ever
            // settling. Claiming exactly the panel's height pins the stored size
            // to the one the user set, as the file lists below do.
            ui.set_min_height(ui.available_height());

            show_header(ui, state.worktrees.len(), state.ui_state);

            if state.worktrees.is_empty() {
                // Only reachable before the first refresh, or when the listing
                // itself failed — a repository always has at least a main tree.
                ui.add_space(6.0);
                ui.weak("No worktrees to show.");
                return;
            }

            // The table is bounded so it can never claim the strip's line; both
            // are ordinary content, never nested panels.
            let table_height =
                (ui.available_height() - DETAIL_STRIP_HEIGHT - ui.spacing().item_spacing.y)
                    .max(row_height);
            response.open = show_table(ui, &mut state, row_height, table_height);

            show_detail_strip(ui, &state);
        });

    response
}

/// Tall enough for the worktrees there are, without crowding out the file lists.
///
/// `row_stride` is one row's full cost - its height plus the gap after it.
fn preferred_height(count: usize, row_stride: f32, available_height: f32) -> f32 {
    let rows = count.clamp(1, PREFERRED_VISIBLE_ROWS) as f32;
    let chrome = SECTION_CHROME + DETAIL_STRIP_HEIGHT;
    let wanted = chrome + rows * row_stride;
    let ceiling = (available_height * MAX_PANEL_FRACTION).max(chrome + row_stride);
    wanted.min(ceiling)
}

fn show_header(ui: &mut egui::Ui, count: usize, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        ui.strong(format!("Worktrees ({count})"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("New…")
                .on_hover_text("Create a worktree: another checkout of this repository")
                .clicked()
            {
                ui_state.actions.push(UiAction::open_new_worktree_dialog());
            }
        });
    });
    ui.separator();
}

/// The worktree rows. Returns the path of a worktree the user asked to open.
fn show_table(
    ui: &mut egui::Ui,
    state: &mut WorktreePanelState<'_>,
    row_height: f32,
    max_height: f32,
) -> Option<PathBuf> {
    let mut open = None;
    let worktrees = state.worktrees;

    ui.push_id("worktree_rows", |ui| {
        // Both are required for clickable rows: labels are selectable by default
        // and would swallow the row's clicks, and the hover highlight needs the
        // repaint `egui_extras` omits. See `ui::mod`.
        super::prepare_clickable_rows(ui);
        let mut hover = HoveredRow::load(ui, "hover");

        TableBuilder::new(ui)
            .id_salt("worktree_table")
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(80.0).clip(true))
            .column(Column::remainder().at_least(60.0).clip(true))
            .column(Column::remainder().at_least(60.0).clip(true))
            .column(Column::exact(CHIP_COL_WIDTH))
            .min_scrolled_height(0.0)
            .max_scroll_height(max_height.max(row_height * 2.0))
            .body(|body| {
                body.rows(row_height, worktrees.len(), |mut row| {
                    let index = row.index();
                    let worktree = &worktrees[index];
                    row.set_selected(worktree.is_current);
                    row.set_hovered(hover.is_hovered(index));

                    let metadata = state.metadata.get(&storage_key(worktree));
                    row.col(|ui| render_name(ui, worktree));
                    row.col(|ui| render_branch(ui, worktree));
                    row.col(|ui| render_status(ui, worktree));
                    row.col(|ui| render_state_chips(ui, metadata));

                    let row_response = row.response();
                    hover.observe(index, &row_response);

                    if row_response.double_clicked() && !worktree.is_current {
                        open = Some(worktree.path.clone());
                    }

                    row_response
                        .clone()
                        .on_hover_text(hover_text(worktree, metadata))
                        .context_menu(|ui| {
                            if show_row_context_menu(ui, worktree, state.ui_state) {
                                open = Some(worktree.path.clone());
                            }
                        });
                });
            });

        hover.store(ui);
    });

    open
}

/// The marker plus the worktree's name.
fn render_name(ui: &mut egui::Ui, worktree: &LinkedWorktree) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        render_marker(ui, worktree);

        let mut name = egui::RichText::new(&worktree.name);
        if worktree.is_current {
            name = name.strong();
        }
        ui.add(egui::Label::new(name).truncate());

        if worktree.is_locked {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("locked")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                )
                .truncate(),
            );
        }
    });
}

/// Paint the state marker rather than writing a bullet character: the app ships
/// no font with `●`, so a text bullet renders as a missing-glyph box.
///
/// The main worktree gets a ring instead of a solid dot, because a text badge
/// reading "main" is indistinguishable from a branch that happens to be called
/// `main` — which is the common case.
fn render_marker(ui: &mut egui::Ui, worktree: &LinkedWorktree) {
    let diameter = MARKER_DIAMETER;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(diameter, ui.available_height()),
        egui::Sense::hover(),
    );
    let center = rect.center();
    let radius = diameter / 2.0;
    let color = mark_color(worktree);

    if worktree.is_main {
        ui.painter()
            .circle_stroke(center, radius - 1.0, egui::Stroke::new(2.0, color));
    } else {
        ui.painter().circle_filled(center, radius - 1.5, color);
    }
}

fn render_branch(ui: &mut egui::Ui, worktree: &LinkedWorktree) {
    let text = egui::RichText::new(worktree.branch_label()).small();
    let text = if worktree.branch.is_some() {
        text
    } else {
        // A detached head is a fact about the checkout, not a normal branch.
        text.color(ui.visuals().weak_text_color())
    };
    ui.add(egui::Label::new(text).truncate());
}

fn render_status(ui: &mut egui::Ui, worktree: &LinkedWorktree) {
    let text = egui::RichText::new(worktree.status.summary()).small();
    let text = match &worktree.status {
        LinkedWorktreeStatus::Clean => text.color(ui.visuals().weak_text_color()),
        LinkedWorktreeStatus::Dirty { .. } => text.color(DIRTY_MARK),
        LinkedWorktreeStatus::Missing | LinkedWorktreeStatus::Unavailable(_) => {
            text.color(BROKEN_MARK)
        }
    };
    ui.add(egui::Label::new(text).truncate());
}

fn mark_color(worktree: &LinkedWorktree) -> egui::Color32 {
    match &worktree.status {
        _ if worktree.is_current => CURRENT_MARK,
        LinkedWorktreeStatus::Clean => CLEAN_MARK,
        LinkedWorktreeStatus::Dirty { .. } => DIRTY_MARK,
        LinkedWorktreeStatus::Missing | LinkedWorktreeStatus::Unavailable(_) => BROKEN_MARK,
    }
}

/// The two states as pills, shown only when they say something.
fn render_state_chips(ui: &mut egui::Ui, metadata: Option<&WorktreeMetadata>) {
    let Some(metadata) = metadata else {
        return;
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        if metadata.review.is_noteworthy() {
            super::render_pill(
                ui,
                review_chip_text(metadata.review),
                review_color(metadata.review),
            )
            .on_hover_text(format!("Review: {}", metadata.review.label()));
        }
        if metadata.test.is_noteworthy() {
            super::render_pill(ui, test_chip_text(metadata.test), test_color(metadata.test))
                .on_hover_text(format!("Tests: {}", metadata.test.label()));
        }
    });
}

/// Chip wording is abbreviated; the full label is a hover away.
fn review_chip_text(review: ReviewState) -> &'static str {
    match review {
        ReviewState::Unreviewed => "",
        ReviewState::NeedsReview => "review",
        ReviewState::ChangesRequested => "changes",
        ReviewState::Approved => "ok",
    }
}

fn review_color(review: ReviewState) -> egui::Color32 {
    match review {
        ReviewState::Unreviewed | ReviewState::NeedsReview => REVIEW_PENDING,
        ReviewState::ChangesRequested => REVIEW_CHANGES,
        ReviewState::Approved => REVIEW_APPROVED,
    }
}

fn test_chip_text(test: TestState) -> &'static str {
    match test {
        TestState::Unknown => "",
        TestState::Passing => "pass",
        TestState::Failing => "fail",
    }
}

fn test_color(test: TestState) -> egui::Color32 {
    match test {
        TestState::Unknown | TestState::Passing => TEST_PASSING,
        TestState::Failing => TEST_FAILING,
    }
}

/// The task of the checkout this tab has open, under the table.
///
/// The rows are three narrow columns in a 300px sidebar, with no room for prose;
/// this is where the current worktree's task gets to be readable.
fn show_detail_strip(ui: &mut egui::Ui, state: &WorktreePanelState<'_>) {
    let current = state.worktrees.iter().find(|worktree| worktree.is_current);

    ui.horizontal(|ui| {
        let Some(current) = current else {
            return;
        };

        match state.metadata_for(current) {
            Some(metadata) if !metadata.task.trim().is_empty() => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("task")
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    )
                    .truncate(),
                );
                ui.add(egui::Label::new(egui::RichText::new(&metadata.task).small()).truncate())
                    .on_hover_text(&metadata.task);
            }
            _ => {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("No task recorded for this worktree")
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    )
                    .truncate(),
                );
            }
        }
    });
}

/// Everything the narrow row could not show.
fn hover_text(worktree: &LinkedWorktree, metadata: Option<&WorktreeMetadata>) -> String {
    let mut lines = vec![
        format!("branch: {}", worktree.branch_label()),
        worktree.path.display().to_string(),
        worktree.status.summary(),
    ];

    if let Some(metadata) = metadata {
        if !metadata.task.trim().is_empty() {
            lines.push(format!("task: {}", metadata.task));
        }
        if !metadata.agent.trim().is_empty() {
            lines.push(format!("agent: {}", metadata.agent));
        }
        if let Some(base) = metadata.short_base_commit() {
            lines.push(format!("base: {base}"));
        }
        if metadata.review.is_noteworthy() {
            lines.push(format!("review: {}", metadata.review.label()));
        }
        if metadata.test.is_noteworthy() {
            lines.push(format!("tests: {}", metadata.test.label()));
        }
    }

    if worktree.is_main {
        lines.push("Main worktree".into());
    }
    if worktree.is_current {
        lines.push("Open in this tab".into());
    }
    if let Some(reason) = &worktree.lock_reason {
        lines.push(format!("Locked: {reason}"));
    }
    if let LinkedWorktreeStatus::Unavailable(detail) = &worktree.status {
        lines.push(detail.clone());
    }

    lines.join("\n")
}

/// The right-click menu: open it, then the one destructive item, which never
/// does more than ask for confirmation. Returns whether "open" was chosen.
fn show_row_context_menu(
    ui: &mut egui::Ui,
    worktree: &LinkedWorktree,
    ui_state: &mut UiState,
) -> bool {
    ui.set_min_width(180.0);
    let mut open = false;

    let openable =
        !worktree.is_current && !matches!(worktree.status, LinkedWorktreeStatus::Missing);
    if ui
        .add_enabled(openable, egui::Button::new("Open in new tab"))
        .clicked()
    {
        open = true;
        ui.close();
    }

    if ui
        .button("Review changes…")
        .on_hover_text("See what this worktree has done since its base")
        .clicked()
    {
        ui_state
            .actions
            .push(UiAction::review_worktree(worktree.clone()));
        ui.close();
    }

    if ui.button("Edit metadata…").clicked() {
        ui_state
            .actions
            .push(UiAction::open_worktree_metadata_dialog(worktree.clone()));
        ui.close();
    }

    if !worktree.is_main {
        ui.separator();
        // Enabled even for a dirty worktree: the confirmation is where the user
        // finds out what is in the way, and it refuses there rather than here.
        if ui
            .add_enabled(!worktree.is_current, egui::Button::new("Remove worktree…"))
            .clicked()
        {
            ui_state
                .actions
                .push(UiAction::open_remove_worktree_dialog(worktree.clone()));
            ui.close();
        }
    }

    open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worktree(name: &str, is_current: bool, status: LinkedWorktreeStatus) -> LinkedWorktree {
        LinkedWorktree {
            name: name.into(),
            path: PathBuf::from("/tmp").join(name),
            branch: Some(format!("feature/{name}")),
            head_short_oid: "abc1234".into(),
            is_main: false,
            is_current,
            is_locked: false,
            lock_reason: None,
            status,
        }
    }

    #[test]
    fn the_section_grows_with_the_list_but_never_takes_the_sidebar() {
        let row = 31.0;
        let one = preferred_height(1, row, 600.0);
        let three = preferred_height(3, row, 600.0);
        assert!(three > one, "more worktrees should ask for more room");
        // Every row must fit under the header, or the last one is clipped by
        // whatever the sidebar puts below the section.
        assert!(
            three >= SECTION_CHROME + 3.0 * row,
            "three rows must fit below the header: {three}"
        );
        assert!(
            preferred_height(40, row, 600.0) <= 600.0 * MAX_PANEL_FRACTION,
            "a long list must still leave the file lists most of the panel"
        );
        // A cramped sidebar still gets a header and one row rather than nothing.
        assert!(preferred_height(40, row, 40.0) >= SECTION_CHROME + row);
    }

    #[test]
    fn the_current_worktree_is_marked_before_its_status() {
        let current = worktree("here", true, LinkedWorktreeStatus::Clean);
        assert_eq!(mark_color(&current), CURRENT_MARK);

        let dirty = worktree(
            "there",
            false,
            LinkedWorktreeStatus::Dirty {
                modified: 1,
                staged: 0,
                untracked: 0,
            },
        );
        assert_eq!(mark_color(&dirty), DIRTY_MARK);
        assert_eq!(
            mark_color(&worktree("gone", false, LinkedWorktreeStatus::Missing)),
            BROKEN_MARK
        );
    }

    #[test]
    fn the_tooltip_carries_what_the_row_cannot_show() {
        let mut locked = worktree("wt", false, LinkedWorktreeStatus::Clean);
        locked.is_locked = true;
        locked.lock_reason = Some("release build".into());

        let text = hover_text(&locked, None);

        assert!(text.contains("branch: feature/wt"));
        assert!(text.contains("/tmp/wt"));
        assert!(text.contains("Locked: release build"));
    }
}
