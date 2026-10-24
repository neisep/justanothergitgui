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
use super::worktree_chips::{render_marker, review_color, test_color};

/// Height of the section header plus its separator. Same construct, and so the
/// same measured height, as a file-list section header.
const SECTION_CHROME: f32 = 52.0;
/// The section never takes more than this share of the sidebar on its own.
const MAX_PANEL_FRACTION: f32 = 0.45;
/// Rows the section sizes itself for before the user has to scroll or drag.
const PREFERRED_VISIBLE_ROWS: usize = 4;
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
/// A row is a name and its state chips, nothing else. Branch and dirty counts
/// used to have columns of their own, which left four truncated fields fighting
/// over 300px and read as a cramped table rather than a list of places to go.
/// Neither is lost: the marker already carries the status as colour, the Agents
/// table shows both in full, and the tooltip spells them out.
///
/// Declared before the file sections so it claims the topmost strip — above the
/// file filter — and leaves the Unstaged/Staged split below it untouched.
pub fn show(ui: &mut egui::Ui, mut state: WorktreePanelState<'_>) -> WorktreePanelResponse {
    let mut response = WorktreePanelResponse::default();
    // Two lines per row: the worktree's name over the branch it has checked out.
    // Derived from the text styles rather than hardcoded, so it still fits when
    // the user changes the font size.
    let row_height =
        (ui.spacing().interact_size.y + ui.text_style_height(&egui::TextStyle::Small)).max(40.0);
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
        // Git's own word, deliberately. A friendlier synonym would not appear in
        // `git worktree list` or in anything the user reads about the feature,
        // and the gap between the two is what makes a worktree easy to mistake
        // for a branch in the first place.
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
                    row.col(|ui| render_name(ui, worktree, row_height));
                    row.col(|ui| render_state_chips(ui, metadata));

                    let row_response = row.response();
                    hover.observe(index, &row_response);

                    // One click, not two: the row looked like a switcher and
                    // behaved like a preview, so the file lists below it went on
                    // showing another checkout's files. Switching is what it
                    // always appeared to offer.
                    if row_response.clicked() && can_open(worktree) {
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
/// The marker, the worktree's name, and under it the branch it has checked out.
///
/// The two are different things and the row has to say so: a worktree is a
/// directory of files, a branch is a name pointing at a commit, and a worktree's
/// directory name is free to differ from the branch inside it entirely. Showing
/// only the name invited reading the list as a list of branches.
fn render_name(ui: &mut egui::Ui, worktree: &LinkedWorktree, row_height: f32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        render_marker(ui, worktree, row_height);

        ui.vertical(|ui| {
            // The two lines belong together; the table already spaces the rows.
            ui.spacing_mut().item_spacing.y = 0.0;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
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

            let weak = ui.visuals().weak_text_color();
            ui.add(
                egui::Label::new(
                    egui::RichText::new(worktree.branch_label())
                        .small()
                        .color(weak),
                )
                .truncate(),
            );
        });
    });
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

fn test_chip_text(test: TestState) -> &'static str {
    match test {
        TestState::Unknown => "",
        TestState::Passing => "pass",
        TestState::Failing => "fail",
    }
}

/// Whether this worktree can be switched to.
///
/// Shared by the row click and the context menu so the two can never disagree.
/// The `Missing` guard is load-bearing rather than cosmetic: opening a
/// repository goes through `Repository::discover`, which walks *upward*, so a
/// checkout whose directory has been deleted would silently resolve to an
/// ancestor repository and open the wrong thing instead of failing.
fn can_open(worktree: &LinkedWorktree) -> bool {
    !worktree.is_current && !matches!(worktree.status, LinkedWorktreeStatus::Missing)
}

/// The task of the checkout this tab has open, under the table.
///
/// The rows are three narrow columns in a 300px sidebar, with no room for prose;
/// this is where the task gets to be readable.
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

    if ui
        .add_enabled(can_open(worktree), egui::Button::new("Open in new tab"))
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
    use crate::ui::worktree_chips::{BROKEN_MARK, CURRENT_MARK, DIRTY_MARK, mark_color};

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

    /// A click switches worktree, so the rule that decides whether it may has
    /// to hold for the row and the menu item alike.
    #[test]
    fn a_worktree_can_be_switched_to_unless_it_is_current_or_gone() {
        assert!(can_open(&worktree(
            "elsewhere",
            false,
            LinkedWorktreeStatus::Clean
        )));

        assert!(
            !can_open(&worktree("here", true, LinkedWorktreeStatus::Clean)),
            "the checkout this tab already has open is not somewhere to go"
        );
        assert!(
            !can_open(&worktree("gone", false, LinkedWorktreeStatus::Missing)),
            "discover() walks upward, so opening a deleted worktree would \
             silently land on an ancestor repository"
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
