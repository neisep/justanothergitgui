//! The visual vocabulary shared by every view that lists worktrees.
//!
//! The sidebar's Worktrees section and the Agents table describe the same
//! things — which checkout this tab has open, how dirty each one is, how far
//! review and tests have got — and they have to agree about what each state
//! looks like. The colours and the painted marker live here so neither panel
//! owns them.
//!
//! Each panel still chooses its own *words*: the sidebar has three narrow
//! columns and abbreviates, the Agents table has room for the full label.

use eframe::egui;

use crate::shared::worktree_metadata::{ReviewState, TestState};
use crate::shared::worktrees::{LinkedWorktree, LinkedWorktreeStatus};

/// Width of the painted state marker in front of each name.
pub const MARKER_DIAMETER: f32 = 10.0;

pub const CURRENT_MARK: egui::Color32 = egui::Color32::from_rgb(120, 190, 255);
pub const CLEAN_MARK: egui::Color32 = egui::Color32::from_rgb(120, 190, 130);
pub const DIRTY_MARK: egui::Color32 = egui::Color32::from_rgb(230, 180, 90);
pub const BROKEN_MARK: egui::Color32 = egui::Color32::from_rgb(220, 120, 120);
pub const REVIEW_PENDING: egui::Color32 = egui::Color32::from_rgb(96, 84, 156);
pub const REVIEW_CHANGES: egui::Color32 = egui::Color32::from_rgb(160, 92, 32);
pub const REVIEW_APPROVED: egui::Color32 = egui::Color32::from_rgb(48, 112, 80);
pub const TEST_PASSING: egui::Color32 = egui::Color32::from_rgb(48, 112, 80);
pub const TEST_FAILING: egui::Color32 = egui::Color32::from_rgb(152, 64, 64);

/// Paint the state marker rather than writing a bullet character: the app ships
/// no font with `●`, so a text bullet renders as a missing-glyph box.
///
/// The main worktree gets a ring instead of a solid dot, because a text badge
/// reading "main" is indistinguishable from a branch that happens to be called
/// `main` — which is the common case.
///
/// `height` is the cell height to centre within. Taken as a parameter rather
/// than read from `ui.available_height()` so a caller in a table row and a
/// caller in a free-standing line can each centre correctly.
pub fn render_marker(ui: &mut egui::Ui, worktree: &LinkedWorktree, height: f32) {
    let diameter = MARKER_DIAMETER;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(diameter, height), egui::Sense::hover());
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

/// Which checkout this tab has open outranks how dirty it is: the status is one
/// column away, but nothing else says "you are standing here".
pub fn mark_color(worktree: &LinkedWorktree) -> egui::Color32 {
    match &worktree.status {
        _ if worktree.is_current => CURRENT_MARK,
        LinkedWorktreeStatus::Clean => CLEAN_MARK,
        LinkedWorktreeStatus::Dirty { .. } => DIRTY_MARK,
        LinkedWorktreeStatus::Missing | LinkedWorktreeStatus::Unavailable(_) => BROKEN_MARK,
    }
}

pub fn review_color(review: ReviewState) -> egui::Color32 {
    match review {
        ReviewState::Unreviewed | ReviewState::NeedsReview => REVIEW_PENDING,
        ReviewState::ChangesRequested => REVIEW_CHANGES,
        ReviewState::Approved => REVIEW_APPROVED,
    }
}

pub fn test_color(test: TestState) -> egui::Color32 {
    match test {
        TestState::Unknown | TestState::Passing => TEST_PASSING,
        TestState::Failing => TEST_FAILING,
    }
}
