//! Render the actual widgets and dispatch pointer events without a display server.
use eframe::egui::{self, Event, Pos2, Rect, Shape};

use crate::commit_rules::CommitMessageRuleSet;
use crate::shared::{actions::UiAction, git::FileEntry};
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
        self.frame(vec![Event::PointerMoved(pos)], draw);
        self.frame(
            vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            draw,
        );
        self.frame(
            vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            draw,
        );
        self.settled(draw)
    }
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
        is_conflicted: conflicted,
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
            inspector: &mut state.inspector,
            ui_state: &mut state.ui,
        },
    );
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
fn filtered_bulk_action_discloses_total_and_hidden_files_before_click() {
    let mut state = AppState::default();
    state.worktree.unstaged = vec![file("src/review.rs", false), file("README.md", false)];
    state.inspector.file_filter = "review".into();
    let mut harness = Harness::new(1280.0);
    let painted = harness.settled(&mut |ui| draw_files(ui, &mut state));
    label(&painted, "Unstaged (1 of 2)");
    label(&painted, "Bulk action includes files hidden by the filter.");
    let target = label(&painted, "Stage All 2 files");
    harness.click(target.rect.center(), &mut |ui| draw_files(ui, &mut state));
    assert!(matches!(state.ui.actions.as_slice(), [UiAction::StageAll]));
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
