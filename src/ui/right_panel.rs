//! Which panel holds the right-hand slot.
//!
//! The Commit panel is the answer in every view but one: the Agents tab replaces
//! it with the Task Details panel, because staging files is about *this*
//! checkout and that tab is about another one.
//!
//! The branch lives here rather than in `app.rs` so the root keeps one
//! unconditional call in its carefully ordered panel sequence — the right-hand
//! slot must be claimed after the left sidebar and before the central panel —
//! and so the swap itself can be driven by the headless tests.

use eframe::egui;

use crate::commit_rules::CommitMessageRuleSet;
use crate::state::{AppState, CenterView};

use super::task_details_panel::{self, TaskDetailsState};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    ruleset: CommitMessageRuleSet,
    custom_scopes: &[String],
    // Already formatted; the app layer owns the clock.
    started_label: Option<&str>,
) {
    // `CenterView` is `Copy`, and reading it out first is what lets the commit
    // arm take `&mut AppState` whole.
    match state.inspector.center_view {
        CenterView::Agents => {
            let AppState {
                repo,
                inspector,
                // Destructured under another name: `AppState`'s own `ui` field
                // would otherwise shadow the `egui::Ui` parameter.
                ui: ui_state,
                ..
            } = state;

            task_details_panel::show(
                ui,
                TaskDetailsState {
                    repo,
                    selected: inspector.selected_worktree.as_ref(),
                    started_label,
                    ui_state,
                },
            );
        }
        CenterView::Diff | CenterView::History | CenterView::Review => {
            super::commit_panel::show(ui, state, ruleset, custom_scopes);
        }
    }
}
