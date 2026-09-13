//! Render the actual widgets and dispatch pointer events without a display server.
use eframe::egui::{self, Event, Pos2, Rect, Shape};

use crate::commit_rules::CommitMessageRuleSet;
use crate::shared::worktrees::{LinkedWorktree, LinkedWorktreeStatus};
use crate::shared::{
    actions::{FileActionKind, PendingFileAction, UiAction},
    git::{FileChangeKind, FileEntry},
};
use crate::state::AppState;

struct PaintedText {
    text: String,
    rect: Rect,
    clip: Rect,
}

struct Harness {
    ctx: egui::Context,
    width: f32,
    time: f64,
}

impl Harness {
    fn new(width: f32) -> Self {
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        Self {
            ctx,
            width,
            time: 0.0,
        }
    }

    fn frame(
        &mut self,
        events: Vec<Event>,
        draw: &mut impl FnMut(&mut egui::Ui),
    ) -> Vec<PaintedText> {
        self.time += 0.05;
        let output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(self.width, 820.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| draw(ui));
            },
        );
        fn collect(shape: &Shape, clip: Rect, text: &mut Vec<PaintedText>) {
            match shape {
                Shape::Text(shape) => text.push(PaintedText {
                    text: shape.galley.text().to_owned(),
                    rect: Rect::from_min_size(shape.pos, shape.galley.size()),
                    clip,
                }),
                Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, clip, text);
                    }
                }
                _ => {}
            }
        }
        let mut text = Vec::new();
        for shape in output.shapes {
            collect(&shape.shape, shape.clip_rect, &mut text);
        }
        text
    }

    fn settled(&mut self, draw: &mut impl FnMut(&mut egui::Ui)) -> Vec<PaintedText> {
        self.frame(vec![], draw);
        self.frame(vec![], draw)
    }

    fn click(&mut self, pos: Pos2, draw: &mut impl FnMut(&mut egui::Ui)) -> Vec<PaintedText> {
        self.press(pos, egui::PointerButton::Primary, draw)
    }

    fn right_click(&mut self, pos: Pos2, draw: &mut impl FnMut(&mut egui::Ui)) -> Vec<PaintedText> {
        self.press(pos, egui::PointerButton::Secondary, draw)
    }

    fn press(
        &mut self,
        pos: Pos2,
        button: egui::PointerButton,
        draw: &mut impl FnMut(&mut egui::Ui),
    ) -> Vec<PaintedText> {
        self.frame(vec![Event::PointerMoved(pos)], draw);
        self.frame(
            vec![Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Default::default(),
            }],
            draw,
        );
        self.frame(
            vec![Event::PointerButton {
                pos,
                button,
                pressed: false,
                modifiers: Default::default(),
            }],
            draw,
        );
        self.settled(draw)
    }
}

fn has_label(painted: &[PaintedText], expected: &str) -> bool {
    painted.iter().any(|item| item.text == expected)
}

fn label<'a>(painted: &'a [PaintedText], expected: &str) -> &'a PaintedText {
    painted
        .iter()
        .find(|item| item.text == expected)
        .unwrap_or_else(|| {
            panic!(
                "Missing {expected:?}; painted: {:?}",
                painted.iter().map(|item| &item.text).collect::<Vec<_>>()
            )
        })
}

fn file(path: &str, conflicted: bool) -> FileEntry {
    FileEntry {
        path: path.into(),
        display_status: if conflicted { "C" } else { "M" }.into(),
        kind: if conflicted {
            FileChangeKind::Conflicted
        } else {
            FileChangeKind::Modified
        },
    }
}

fn file_with_kind(path: &str, status: &str, kind: FileChangeKind) -> FileEntry {
    FileEntry {
        path: path.into(),
        display_status: status.into(),
        kind,
    }
}

fn draw_commit(ui: &mut egui::Ui, state: &mut AppState) {
    super::commit_panel::show(ui, state, CommitMessageRuleSet::Off, &[]);
}

fn draw_files(ui: &mut egui::Ui, state: &mut AppState) {
    super::file_panel::show(
        ui,
        super::file_panel::FilePanelState {
            worktree: &state.worktree,
            worktrees: &state.repo.linked_worktrees,
            inspector: &mut state.inspector,
            ui_state: &mut state.ui,
        },
    );
}

fn linked_worktree(name: &str, is_main: bool, status: LinkedWorktreeStatus) -> LinkedWorktree {
    LinkedWorktree {
        name: name.into(),
        path: std::path::PathBuf::from("/tmp/worktrees").join(name),
        branch: Some(format!("feature/{name}")),
        head_short_oid: "abc1234".into(),
        is_main,
        is_current: is_main,
        is_locked: false,
        lock_reason: None,
        status,
    }
}

fn state_with_worktrees() -> AppState {
    let mut state = AppState::default();
    state
        .repo
        .linked_worktrees
        .push(linked_worktree("myapp", true, LinkedWorktreeStatus::Clean));
    state.repo.linked_worktrees.push(linked_worktree(
        "feature-auth",
        false,
        LinkedWorktreeStatus::Dirty {
            modified: 4,
            staged: 0,
            untracked: 0,
        },
    ));
    state
}

#[test]
fn the_sidebar_lists_every_worktree_with_its_status() {
    let mut state = state_with_worktrees();
    let mut harness = Harness::new(1280.0);

    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));

    label(&painted, "Worktrees (2)");
    label(&painted, "myapp");
    label(&painted, "feature-auth");
    label(&painted, "Clean");
    label(&painted, "4 modified files");
    // The branch belongs in the list, not only in the tooltip.
    label(&painted, "feature/myapp");
    label(&painted, "feature/feature-auth");
    // The file sections keep their own headers: the strip must not push either
    // of them out of the panel.
    assert!(has_label(&painted, "Unstaged (0)"));
    assert!(has_label(&painted, "Staged (0)"));

    // The section sits at the top of the sidebar, above the filter row and both
    // file lists.
    let worktrees_header = label(&painted, "Worktrees (2)").rect.top();
    assert!(
        worktrees_header < label(&painted, "Filter files...").rect.top(),
        "the Worktrees header must sit above the file filter"
    );
    assert!(
        worktrees_header < label(&painted, "Unstaged (0)").rect.top(),
        "the Worktrees header must sit above the Unstaged list"
    );
}

#[test]
fn the_new_worktree_button_only_opens_the_dialog() {
    let mut state = state_with_worktrees();
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let button = label(&painted, "New…");

    harness.click(button.rect.center(), &mut |ui| draw_files(ui, &mut state));

    assert!(
        matches!(
            state.ui.actions.as_slice(),
            [UiAction::OpenNewWorktreeDialog]
        ),
        "expected exactly one dialog-opening action: {:?}",
        state.ui.actions
    );
}

#[test]
fn right_click_on_a_worktree_only_opens_the_removal_confirmation() {
    let mut state = state_with_worktrees();
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let row = label(&painted, "feature-auth");

    let painted = harness.right_click(row.rect.center(), &mut |ui| draw_files(ui, &mut state));
    let item = label(&painted, "Remove worktree…");

    harness.click(item.rect.center(), &mut |ui| draw_files(ui, &mut state));

    assert!(
        matches!(
            state.ui.actions.as_slice(),
            [UiAction::OpenRemoveWorktreeDialog(worktree)] if worktree.name == "feature-auth"
        ),
        "the menu item must only ask for confirmation: {:?}",
        state.ui.actions
    );
}

#[test]
fn the_main_worktree_is_never_offered_for_removal() {
    let mut state = state_with_worktrees();
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let row = label(&painted, "myapp");

    let painted = harness.right_click(row.rect.center(), &mut |ui| draw_files(ui, &mut state));

    assert!(
        !has_label(&painted, "Remove worktree…"),
        "the main worktree has no removal item at all"
    );
    assert!(state.ui.actions.is_empty());
}

#[test]
fn the_removal_confirmation_refuses_a_worktree_holding_uncommitted_work() {
    let dirty = linked_worktree(
        "feature-auth",
        false,
        LinkedWorktreeStatus::Dirty {
            modified: 4,
            staged: 0,
            untracked: 2,
        },
    );
    let blocker = crate::core::worktrees::service::removal_blocker(&dirty);
    assert!(blocker.is_some(), "a dirty worktree must have a blocker");

    let mut confirmed = false;
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| {
        let ctx = ui.ctx().clone();
        let output = super::dialogs::worktree::show_remove_dialog(
            &ctx,
            &dirty,
            blocker.as_deref(),
            false,
            None,
        );
        confirmed |= output.confirm_requested;
    });

    label(&painted, "This cannot be undone.");
    label(&painted, "This worktree has uncommitted work:");
    label(&painted, "• 4 modified files");
    label(&painted, "• 2 untracked files");
    label(
        &painted,
        "Open the worktree in a tab to commit or discard the changes.",
    );
    // Phase 1 has no force path at all: there is no checkbox offering one.
    assert!(!has_label(&painted, "Also discard uncommitted changes"));

    let button = label(&painted, "Remove worktree");
    harness.click(button.rect.center(), &mut |ui| {
        let ctx = ui.ctx().clone();
        let output = super::dialogs::worktree::show_remove_dialog(
            &ctx,
            &dirty,
            blocker.as_deref(),
            false,
            None,
        );
        confirmed |= output.confirm_requested;
    });

    assert!(
        !confirmed,
        "the confirm button must stay disabled while work would be lost"
    );
}

#[test]
fn the_removal_confirmation_allows_a_clean_worktree() {
    let clean = linked_worktree("cache-refactor", false, LinkedWorktreeStatus::Clean);
    assert!(crate::core::worktrees::service::removal_blocker(&clean).is_none());

    let mut confirmed = false;
    let mut draw = |ui: &mut egui::Ui| {
        let ctx = ui.ctx().clone();
        let output = super::dialogs::worktree::show_remove_dialog(&ctx, &clean, None, false, None);
        confirmed |= output.confirm_requested;
    };

    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut draw);
    // Removing a worktree must not read as removing the branch too.
    label(
        &painted,
        "Branch 'feature/cache-refactor' is kept — only the checkout is removed.",
    );
    let button = label(&painted, "Remove worktree");

    harness.click(button.rect.center(), &mut draw);

    assert!(confirmed, "a clean worktree can be confirmed for removal");
}

#[test]
fn the_new_worktree_form_blocks_creation_while_the_input_is_invalid() {
    let mut state = crate::state::WorktreeDialogState {
        show_new_worktree_dialog: true,
        name: "feature-auth".into(),
        branch: "feature/auth".into(),
        path: "/tmp/worktrees/feature-auth".into(),
        ..Default::default()
    };
    let branches = vec!["main".to_string()];
    let mut created = false;

    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| {
        let ctx = ui.ctx().clone();
        let output = super::dialogs::worktree::show_new_dialog(
            &ctx,
            &mut state,
            super::dialogs::worktree::NewWorktreeDialogView {
                branches: &branches,
                validation_error: Some("A worktree named 'feature-auth' already exists.".into()),
                reuses_existing_branch: false,
                busy: false,
                busy_label: None,
            },
        );
        created |= output.create_requested;
    });

    label(&painted, "A worktree named 'feature-auth' already exists.");
    let button = label(&painted, "Create worktree");

    harness.click(button.rect.center(), &mut |ui| {
        let ctx = ui.ctx().clone();
        let output = super::dialogs::worktree::show_new_dialog(
            &ctx,
            &mut state,
            super::dialogs::worktree::NewWorktreeDialogView {
                branches: &branches,
                validation_error: Some("A worktree named 'feature-auth' already exists.".into()),
                reuses_existing_branch: false,
                busy: false,
                busy_label: None,
            },
        );
        created |= output.create_requested;
    });

    assert!(!created, "an invalid request must not reach the worker");
}

#[test]
fn the_new_worktree_form_explains_when_it_will_reuse_a_branch() {
    let mut state = crate::state::WorktreeDialogState {
        show_new_worktree_dialog: true,
        name: "auth".into(),
        branch: "feature/auth".into(),
        path: "/tmp/worktrees/auth".into(),
        ..Default::default()
    };
    let branches = vec!["main".to_string(), "feature/auth".to_string()];
    let mut created = false;

    let mut draw = |ui: &mut egui::Ui| {
        let ctx = ui.ctx().clone();
        let output = super::dialogs::worktree::show_new_dialog(
            &ctx,
            &mut state,
            super::dialogs::worktree::NewWorktreeDialogView {
                branches: &branches,
                validation_error: None,
                reuses_existing_branch: true,
                busy: false,
                busy_label: None,
            },
        );
        created |= output.create_requested;
    };

    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut draw);
    label(
        &painted,
        "ⓘ Branch 'feature/auth' exists — it will be checked out instead of created.",
    );
    let button = label(&painted, "Create worktree");

    harness.click(button.rect.center(), &mut draw);

    assert!(created, "a valid request reaches the worker");
}

#[test]
fn commit_shows_scope_without_duplicating_the_staged_file_list() {
    for width in [1280.0, 640.0] {
        let mut state = AppState::default();
        state.worktree.staged.push(file("README.md", false));
        state.worktree.unstaged.push(file("src/review.rs", false));
        let mut harness = Harness::new(width);
        let painted = harness.settled(&mut |ui| draw_commit(ui, &mut state));
        label(&painted, "Committing 1 staged file");
        label(&painted, "Enter a commit summary");
        label(&painted, "Commit 1 file");
        assert!(
            painted
                .iter()
                .all(|item| item.text != "README.md" && item.text != "src/review.rs")
        );
        assert!(state.ui.actions.is_empty());
    }
}

#[test]
fn disabled_commit_explains_reason_and_does_not_dispatch_commit() {
    for (staged, summary, conflicted, reason) in [
        (false, "", false, "Stage files to include in this commit"),
        (true, "", false, "Enter a commit summary"),
        (
            true,
            "fix conflict",
            true,
            "Resolve and save all conflicted files first",
        ),
    ] {
        let mut state = AppState::default();
        if staged {
            state.worktree.staged.push(file("README.md", false));
        }
        if conflicted {
            state.worktree.unstaged.push(file("src/review.rs", true));
        }
        state.commit.commit_summary = summary.into();
        let mut harness = Harness::new(1280.0);
        let painted = harness.settled(&mut |ui| draw_commit(ui, &mut state));
        label(&painted, reason);
        let button = label(
            &painted,
            if staged {
                "Commit 1 file"
            } else {
                "Commit 0 files"
            },
        );
        harness.click(button.rect.center(), &mut |ui| draw_commit(ui, &mut state));
        assert!(state.ui.actions.is_empty());
    }
}

#[test]
fn conflicted_file_resolve_button_opens_editor_without_staging() {
    for width in [1280.0, 640.0] {
        let mut state = AppState::default();
        state.worktree.unstaged.push(file("src/review.rs", true));
        let mut harness = Harness::new(width);
        let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
        let target = label(&painted, "Resolve…");
        assert!(
            target.clip.expand(1.0).contains_rect(target.rect),
            "resolve action clipped at width {width}"
        );
        harness.click(target.rect.center(), &mut |ui| draw_files(ui, &mut state));
        assert!(
            matches!(state.ui.actions.as_slice(), [UiAction::SelectFile { path, staged: false }] if path == "src/review.rs")
        );
    }
}

#[test]
fn right_click_on_a_modified_row_offers_discard_and_only_opens_a_confirmation() {
    let mut state = AppState::default();
    state.worktree.unstaged.push(file_with_kind(
        "src/review.rs",
        "modified",
        FileChangeKind::Modified,
    ));
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let row = label(&painted, "review.rs");

    let painted = harness.right_click(row.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(
        matches!(state.ui.actions.as_slice(), [UiAction::SelectFile { path, staged: false }] if path == "src/review.rs"),
        "right-click selects the row so the diff shows what is about to go"
    );
    state.ui.actions.clear();
    assert!(!has_label(&painted, "Delete file…"));
    let item = label(&painted, "Discard changes…");

    let painted = harness.click(item.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert_eq!(
        state.ui.actions.len(),
        1,
        "menu item queues exactly one action: {:?}",
        state.ui.actions
    );
    assert!(matches!(
        &state.ui.actions[0],
        UiAction::OpenFileActionDialog(PendingFileAction { path, staged: false, kind: FileActionKind::DiscardWorktree })
            if path == "src/review.rs"
    ));
    assert!(
        !has_label(&painted, "Discard changes…"),
        "menu closes after choosing an item"
    );
}

#[test]
fn right_click_on_an_untracked_row_offers_delete_instead_of_discard() {
    let mut state = AppState::default();
    state.worktree.unstaged.push(file_with_kind(
        "notes.txt",
        "untracked",
        FileChangeKind::Added,
    ));
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let row = label(&painted, "notes.txt");

    let painted = harness.right_click(row.rect.center(), &mut |ui| draw_files(ui, &mut state));
    state.ui.actions.clear();
    assert!(!has_label(&painted, "Discard changes…"));
    let item = label(&painted, "Delete file…");

    harness.click(item.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(matches!(
        state.ui.actions.as_slice(),
        [UiAction::OpenFileActionDialog(PendingFileAction { path, staged: false, kind: FileActionKind::DeleteUntracked })]
            if path == "notes.txt"
    ));
}

#[test]
fn right_click_on_a_conflicted_row_offers_only_resolve() {
    let mut state = AppState::default();
    state.worktree.unstaged.push(file("src/review.rs", true));
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let row = label(&painted, "review.rs");

    let painted = harness.right_click(row.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(!has_label(&painted, "Discard changes…"));
    assert!(!has_label(&painted, "Delete file…"));
    // The quick-action button and the menu item share the label.
    assert!(
        painted
            .iter()
            .filter(|item| item.text == "Resolve…")
            .count()
            >= 2,
        "menu shows Resolve…"
    );

    // Right-clicking must not queue the selection that "Resolve…" itself
    // queues; the action reaches the queue exactly once.
    let item = painted
        .iter()
        .filter(|item| item.text == "Resolve…")
        .max_by(|a, b| a.rect.top().total_cmp(&b.rect.top()))
        .expect("menu item");
    harness.click(item.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(
        matches!(state.ui.actions.as_slice(), [UiAction::SelectFile { path, staged: false }] if path == "src/review.rs"),
        "expected one SelectFile, got {:?}",
        state.ui.actions
    );
}

#[test]
fn filtered_bulk_action_discloses_total_and_hidden_files_before_click() {
    let mut state = AppState::default();
    state.worktree.unstaged = vec![file("src/review.rs", false), file("README.md", false)];
    state.inspector.file_filter = "review".into();
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    label(&painted, "Unstaged (1 of 2)");
    label(&painted, "Stage All includes files hidden by the filter.");
    let target = label(&painted, "Stage All 2 files");
    harness.click(target.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(matches!(state.ui.actions.as_slice(), [UiAction::StageAll]));
}

#[test]
fn filtered_staging_only_dispatches_matching_non_conflicted_paths() {
    for staged in [false, true] {
        let mut state = AppState::default();
        let files = vec![
            file("src/review.rs", false),
            file("src/conflict.rs", true),
            file("README.md", false),
        ];
        if staged {
            state.worktree.staged = files;
        } else {
            state.worktree.unstaged = files;
        }
        state.inspector.file_filter = "src/".into();
        let mut harness = Harness::new(640.0);
        let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
        let target = label(
            &painted,
            if staged {
                "Unstage matching (1)"
            } else {
                "Stage matching (1)"
            },
        );
        assert!(target.clip.expand(0.5).contains_rect(target.rect));
        harness.click(target.rect.center(), &mut |ui| draw_files(ui, &mut state));
        match state.ui.actions.as_slice() {
            [UiAction::StageFiles(paths)] if !staged => assert_eq!(paths, &["src/review.rs"]),
            [UiAction::UnstageFiles(paths)] if staged => assert_eq!(paths, &["src/review.rs"]),
            _ => panic!("Expected one action affecting only the matching resolved file"),
        }
    }
}

#[test]
fn no_filter_matches_disables_matching_action() {
    let mut state = AppState::default();
    state.worktree.unstaged.push(file("README.md", false));
    state.inspector.file_filter = "missing".into();
    let mut harness = Harness::new(640.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    let target = label(&painted, "Stage matching (0)");
    harness.click(target.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(state.ui.actions.is_empty());
}

#[test]
fn patch_metadata_is_hidden_until_user_expands_details() {
    let rows = crate::shared::diff::parse_diff_rows(
        "diff --git a/review.rs b/review.rs\nindex 1234567..abcdef0 100644\n--- a/review.rs\n+++ b/review.rs\n@@ -1 +1 @@ fn review()\n-old_value\n+new_value\n",
    );
    let mut harness = Harness::new(1280.0);
    let mut draw = |ui: &mut egui::Ui| super::diff_view::show_diff_table(ui, &rows, false);
    let painted = harness.settled(&mut draw);
    assert!(!painted.iter().any(|item| item.text.contains("diff --git")));
    assert!(painted.iter().any(|item| item.text.contains("new_value")));
    let target = label(&painted, "Patch details");
    let painted = harness.click(target.rect.center(), &mut draw);
    label(&painted, "diff --git a/review.rs b/review.rs");
    label(&painted, "index 1234567..abcdef0 100644");
}

#[test]
fn two_unstaged_rows_and_staged_file_fit_with_production_button_padding() {
    let mut state = AppState::default();
    state.worktree.unstaged = vec![file("src/review.rs", false), file("tests/second.rs", false)];
    state.worktree.staged = vec![file("README.md", false)];
    let mut harness = Harness::new(1280.0);
    harness
        .ctx
        .global_style_mut(|style| style.spacing.button_padding = egui::vec2(7.0, 4.0));
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    for name in ["review.rs", "second.rs", "README.md"] {
        let item = label(&painted, name);
        assert!(
            item.clip.expand(0.5).contains_rect(item.rect),
            "{name} is clipped: {:?} vs {:?}",
            item.rect,
            item.clip
        );
    }
    let stage_buttons: Vec<_> = painted.iter().filter(|item| item.text == "Stage").collect();
    assert_eq!(stage_buttons.len(), 2);
    for button in stage_buttons {
        assert!(
            button.clip.expand(0.5).contains_rect(button.rect),
            "Stage button clipped"
        );
    }
    let button = label(&painted, "Unstage");
    assert!(button.clip.expand(0.5).contains_rect(button.rect));
}

#[test]
fn merge_commit_panel_expands_collapses_and_restores_form_when_leaving_merge() {
    use crate::shared::conflicts::{ConflictChoice, ConflictData, ConflictPart};
    let mut state = AppState::default();
    state.commit.commit_summary = "saved draft summary".into();
    state.inspector.set_conflict(Some(ConflictData::new(
        "src/review.rs".into(),
        vec![ConflictPart::Conflict {
            ours: "old".into(),
            theirs: "new".into(),
            resolution: ConflictChoice::Unresolved,
        }],
        Default::default(),
    )));
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_commit(ui, &mut state));
    assert!(!painted.iter().any(|item| item.text == "Summary:"));
    let target = label(&painted, "Commit…");
    let painted = harness.click(target.rect.center(), &mut |ui| draw_commit(ui, &mut state));
    label(&painted, "Summary:");
    label(&painted, "saved draft summary");
    let target = label(&painted, "Collapse");
    let painted = harness.click(target.rect.center(), &mut |ui| draw_commit(ui, &mut state));
    label(&painted, "Commit…");
    assert!(!painted.iter().any(|item| item.text == "Summary:"));
    state.inspector.set_conflict(None);
    let painted = harness.settled(&mut |ui| draw_commit(ui, &mut state));
    label(&painted, "Summary:");
    label(&painted, "saved draft summary");
    assert!(
        !painted
            .iter()
            .any(|item| item.text == "Commit…" || item.text == "Collapse")
    );
    assert!(state.ui.actions.is_empty());
}
