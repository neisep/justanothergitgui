use eframe::egui;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::{
    ConflictChoice, ConflictData, ConflictPart, MergeSegment, SegmentOrigin,
};
use crate::shared::diff::{DiffLineKind, parse_diff_rows};
use crate::state::{CenterView, ConflictEdit, InspectorState, RepoState, UiState, WorktreeState};

use super::diff_view;

pub struct DiffPanelState<'a> {
    pub repo: &'a RepoState,
    pub worktree: &'a WorktreeState,
    pub inspector: &'a mut InspectorState,
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, mut state: DiffPanelState<'_>) {
    ui.horizontal(|ui| {
        if ui
            .selectable_label(state.inspector.center_view == CenterView::Diff, "Changes")
            .clicked()
        {
            state.ui_state.actions.push(UiAction::show_diff());
        }
        if ui
            .selectable_label(
                state.inspector.center_view == CenterView::History,
                "History",
            )
            .clicked()
        {
            state.ui_state.actions.push(UiAction::show_history());
        }
    });
    ui.separator();

    match state.inspector.center_view.clone() {
        CenterView::Diff => show_diff_or_conflict(ui, &mut state),
        CenterView::History => show_history(ui, &mut state),
    }
}

/// The History tab shows either the commit list or, once a commit is picked, the
/// read-only commit view in its place.
fn show_history(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {
    let DiffPanelState {
        repo,
        inspector,
        ui_state,
        ..
    } = state;

    match inspector.selected_commit.as_mut() {
        Some(commit) => {
            super::commit_view::show(ui, super::commit_view::CommitViewState { commit, ui_state })
        }
        None => super::history_panel::show(
            ui,
            super::history_panel::HistoryPanelView {
                repo_path: repo.path.as_deref(),
                commit_history: &repo.commit_history,
                ui_state,
            },
        ),
    }
}

fn show_diff_or_conflict(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {
    if state.inspector.conflict_data.is_some() {
        show_conflict_view(ui, state);
    } else if state.inspector.selected_file.is_some() {
        show_diff_view(ui, state);
    } else {
        show_diff_empty_state(ui, state);
    }
}

fn show_diff_empty_state(ui: &mut egui::Ui, state: &DiffPanelState<'_>) {
    let (title, hint) = if state.repo.path.is_none() {
        (
            "No repository open",
            "Use the top bar to open, clone, or init a repository.",
        )
    } else if state.worktree.unstaged.is_empty() && state.worktree.staged.is_empty() {
        (
            "Nothing to show",
            "Edit a file in your project — changes will appear on the left.",
        )
    } else {
        (
            "Pick a file to inspect",
            "Click any file on the left to see what changed.",
        )
    };

    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.35);
        ui.weak(title);
        ui.add_space(4.0);
        let weak = ui.visuals().weak_text_color();
        ui.label(egui::RichText::new(hint).small().color(weak));
    });
}

fn show_diff_view(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {
    if let Some(sel) = &state.inspector.selected_file {
        let rows = parse_diff_rows(&state.inspector.diff_content);
        let added_lines = rows
            .iter()
            .filter(|row| row.kind == DiffLineKind::Added)
            .count();
        let removed_lines = rows
            .iter()
            .filter(|row| row.kind == DiffLineKind::Removed)
            .count();

        ui.horizontal(|ui| {
            ui.strong(&sel.path);
            ui.weak(if sel.staged { "(staged)" } else { "(unstaged)" });
            ui.separator();
            ui.weak(format!("+{} / -{}", added_lines, removed_lines));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut state.inspector.diff_wrap, "Wrap lines");
            });
        });
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("diff_scroll")
            .show(ui, |ui| {
                if state.inspector.diff_content.is_empty() {
                    ui.weak("No diff available (file may be binary or new)");
                    return;
                }

                diff_view::show_diff_table(ui, &rows, state.inspector.diff_wrap);
            });
    }
}

#[derive(Clone, Copy, PartialEq)]
enum MergeSide {
    Current,
    Incoming,
}

// Shared merge-editor palette. "Current" (ours) is green, "Incoming" (theirs)
// is blue, matching the VSCode merge editor's convention.
const CURRENT_TEXT: egui::Color32 = egui::Color32::from_rgb(120, 220, 130);
const INCOMING_TEXT: egui::Color32 = egui::Color32::from_rgb(125, 180, 255);
const CUSTOM_ACCENT: egui::Color32 = egui::Color32::from_rgb(190, 190, 190);
const UNRESOLVED_ACCENT: egui::Color32 = egui::Color32::from_rgb(240, 180, 70);
const EDIT_ACCENT: egui::Color32 = egui::Color32::from_rgb(230, 160, 230);

fn show_conflict_view(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {
    let save_clicked = {
        let InspectorState {
            conflict_data,
            conflict_edit,
            conflict_scroll,
            ..
        } = &mut *state.inspector;

        let Some(data) = conflict_data.as_mut() else {
            return;
        };

        let unresolved = data.unresolved_count();

        let mut save_clicked = false;
        ui.horizontal(|ui| {
            ui.strong(format!("Conflict: {}", &data.path));
            if unresolved > 0 {
                ui.colored_label(
                    UNRESOLVED_ACCENT,
                    format!("{unresolved} conflict(s) left — pick a side or edit"),
                );
            } else {
                ui.colored_label(CURRENT_TEXT, "All conflicts resolved — ready to save");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                save_clicked = ui
                    .add_enabled(unresolved == 0, egui::Button::new("Save Merge"))
                    .on_hover_text(if unresolved == 0 {
                        "Write the merged file and stage it"
                    } else {
                        "Resolve every conflict before saving"
                    })
                    .clicked();
            });
        });
        ui.separator();

        // Bottom: the merged Result document (VSCode-style inline resolution).
        // Declared before the top area so the two input panes fill the rest.
        let mut result_action: Option<ResultAction> = None;
        egui::Panel::bottom("merge_result_pane")
            .resizable(true)
            .default_size((ui.available_height() * 0.45).max(140.0))
            .show_inside(ui, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.strong("Result");
                    ui.weak("Tick the lines to keep, or use the buttons on each conflict.");
                });
                ui.separator();
                result_action = render_result_document(ui, data, conflict_edit);
            });
        apply_result_action(data, result_action, ui.ctx());

        // Top: the two input panes, sharing one vertical scroll offset. Each
        // line the user can choose from has its own checkbox; ticking a line
        // adds it to the result below.
        let scroll = *conflict_scroll;
        let mut pane_action: Option<ResultAction> = None;
        let mut offsets = [scroll; 2];
        ui.columns(2, |columns| {
            offsets[0] = render_input_pane(
                &mut columns[0],
                data,
                MergeSide::Current,
                scroll,
                conflict_edit,
                &mut pane_action,
            );
            offsets[1] = render_input_pane(
                &mut columns[1],
                data,
                MergeSide::Incoming,
                scroll,
                conflict_edit,
                &mut pane_action,
            );
        });

        // Adopt whichever pane the user actually scrolled this frame.
        *conflict_scroll = if (offsets[0] - scroll).abs() > 0.5 {
            offsets[0]
        } else if (offsets[1] - scroll).abs() > 0.5 {
            offsets[1]
        } else {
            scroll
        };
        apply_result_action(data, pane_action, ui.ctx());

        save_clicked
    };

    if save_clicked {
        state
            .ui_state
            .actions
            .push(UiAction::save_conflict_resolution());
    }
}

/// Apply a deferred `(section index, choice)` to the conflict data.
fn apply_choice(
    data: &mut ConflictData,
    choice: Option<(usize, ConflictChoice)>,
    ctx: &egui::Context,
) {
    if let Some((index, choice)) = choice
        && let Some(ConflictPart::Conflict { resolution, .. }) = data.sections.get_mut(index)
    {
        *resolution = choice;
        ctx.request_repaint();
    }
}

/// A deferred action produced while rendering the result document.
enum ResultAction {
    Choose(usize, ConflictChoice),
    Toggle(usize, usize),
}

/// Apply a deferred result-document action to the conflict data.
fn apply_result_action(data: &mut ConflictData, action: Option<ResultAction>, ctx: &egui::Context) {
    match action {
        Some(ResultAction::Choose(index, choice)) => apply_choice(data, Some((index, choice)), ctx),
        Some(ResultAction::Toggle(index, segment)) => {
            data.toggle_segment(index, segment);
            ctx.request_repaint();
        }
        None => {}
    }
}

/// Render the merged result as a read-only document: agreed lines as plain
/// text and, for each conflict, the lines currently kept (colored by which side
/// they came from). Untouched conflicts show a placeholder; the conflict being
/// edited shows an inline text box. Returns a deferred action (e.g. edit apply).
fn render_result_document(
    ui: &mut egui::Ui,
    data: &ConflictData,
    conflict_edit: &mut Option<ConflictEdit>,
) -> Option<ResultAction> {
    let mut action: Option<ResultAction> = None;

    egui::ScrollArea::vertical()
        .id_salt("merge_result_doc")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut conflict_no = 0usize;
            for (index, section) in data.sections.iter().enumerate() {
                match section {
                    ConflictPart::Common(text) => {
                        render_plain_lines(ui, text, ui.visuals().text_color());
                    }
                    ConflictPart::Conflict { resolution, .. } => {
                        conflict_no += 1;
                        if conflict_edit
                            .as_ref()
                            .is_some_and(|edit| edit.index == index)
                        {
                            render_edit_zone(ui, conflict_no, conflict_edit, &mut action);
                        } else if let ConflictChoice::Custom(text) = resolution {
                            render_custom_zone(
                                ui,
                                index,
                                conflict_no,
                                text,
                                conflict_edit,
                                &mut action,
                            );
                        } else if !resolution.is_resolved() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "‹ Conflict {conflict_no} — tick lines in the panes above ›"
                                ))
                                .monospace()
                                .italics()
                                .color(UNRESOLVED_ACCENT),
                            );
                        } else {
                            render_result_conflict(ui, data, index);
                        }
                        ui.add_space(2.0);
                    }
                }
            }
        });

    action
}

/// Render the kept lines of one resolved conflict in the result, colored by the
/// side each line came from (green = current, blue = incoming, plain = shared).
fn render_result_conflict(ui: &mut egui::Ui, data: &ConflictData, index: usize) {
    let Some((segments, mask)) = data.conflict_segments(index) else {
        return;
    };
    let mut any_kept = false;
    for (segment, keep) in segments.iter().zip(mask.iter()) {
        if !*keep {
            continue;
        }
        any_kept = true;
        let color = match segment.origin {
            SegmentOrigin::Common => ui.visuals().text_color(),
            SegmentOrigin::Ours => CURRENT_TEXT,
            SegmentOrigin::Theirs => INCOMING_TEXT,
        };
        ui.label(
            egui::RichText::new(line_or_space(&segment.text))
                .monospace()
                .color(color),
        );
    }
    if !any_kept {
        ui.label(
            egui::RichText::new("(no lines kept)")
                .monospace()
                .italics()
                .color(ui.visuals().weak_text_color()),
        );
    }
}

/// One checkbox row for a changed line. Kept lines show in their side color;
/// dropped lines are dimmed and struck through so the result reads clearly.
/// Returns `true` when toggled.
fn line_checkbox(ui: &mut egui::Ui, keep: bool, text: &str, kept_color: egui::Color32) -> bool {
    let mut checked = keep;
    let label = if keep {
        egui::RichText::new(text).monospace().color(kept_color)
    } else {
        egui::RichText::new(text)
            .monospace()
            .color(ui.visuals().weak_text_color())
            .strikethrough()
    };
    ui.checkbox(&mut checked, label).changed()
}

fn render_custom_zone(
    ui: &mut egui::Ui,
    index: usize,
    conflict_no: usize,
    text: &str,
    conflict_edit: &mut Option<ConflictEdit>,
    action: &mut Option<ResultAction>,
) {
    conflict_zone_frame(ui, CUSTOM_ACCENT, |ui| {
        ui.horizontal(|ui| {
            ui.colored_label(
                CUSTOM_ACCENT,
                format!("✎ Conflict {conflict_no} — custom edit"),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Edit").clicked() {
                    *conflict_edit = Some(ConflictEdit {
                        index,
                        buffer: text.to_string(),
                    });
                }
                if ui
                    .small_button("Reopen")
                    .on_hover_text("Discard the custom edit and pick lines again")
                    .clicked()
                {
                    *action = Some(ResultAction::Choose(index, ConflictChoice::Unresolved));
                }
            });
        });
        if text.is_empty() {
            ui.label(
                egui::RichText::new("(empty)")
                    .monospace()
                    .italics()
                    .color(ui.visuals().weak_text_color()),
            );
        } else {
            render_plain_lines(ui, text, ui.visuals().text_color());
        }
    });
}

fn render_edit_zone(
    ui: &mut egui::Ui,
    conflict_no: usize,
    conflict_edit: &mut Option<ConflictEdit>,
    action: &mut Option<ResultAction>,
) {
    let mut apply = false;
    let mut cancel = false;
    conflict_zone_frame(ui, EDIT_ACCENT, |ui| {
        ui.colored_label(EDIT_ACCENT, format!("✎ Editing conflict {conflict_no}"));
        if let Some(edit) = conflict_edit.as_mut() {
            ui.add(
                egui::TextEdit::multiline(&mut edit.buffer)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(4),
            );
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            apply = ui.button("Apply").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });

    if apply {
        if let Some(edit) = conflict_edit.take() {
            *action = Some(ResultAction::Choose(
                edit.index,
                ConflictChoice::Custom(edit.buffer),
            ));
        }
    } else if cancel {
        *conflict_edit = None;
    }
}

fn line_or_space(line: &str) -> &str {
    if line.is_empty() { " " } else { line }
}

/// A rounded card outlined in a conflict zone's accent color.
fn conflict_zone_frame(
    ui: &mut egui::Ui,
    accent: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(egui::Stroke::new(1.0, accent))
        .corner_radius(5.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

fn render_input_pane(
    ui: &mut egui::Ui,
    data: &ConflictData,
    side: MergeSide,
    scroll_offset: f32,
    conflict_edit: &mut Option<ConflictEdit>,
    action: &mut Option<ResultAction>,
) -> f32 {
    let (title, text_color, id_salt, mine) = match side {
        MergeSide::Current => (
            "Current (ours)",
            CURRENT_TEXT,
            "merge_in_current",
            SegmentOrigin::Ours,
        ),
        MergeSide::Incoming => (
            "Incoming (theirs)",
            INCOMING_TEXT,
            "merge_in_incoming",
            SegmentOrigin::Theirs,
        ),
    };

    ui.colored_label(text_color, egui::RichText::new(title).strong());
    ui.weak("tick the lines you want in the result");
    ui.separator();

    let output = egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .vertical_scroll_offset(scroll_offset)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, section) in data.sections.iter().enumerate() {
                match section {
                    ConflictPart::Common(text) => {
                        render_plain_lines(ui, text, ui.visuals().weak_text_color())
                    }
                    ConflictPart::Conflict { .. } => {
                        let Some((segments, mask)) = data.conflict_segments(index) else {
                            continue;
                        };
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0, text_color))
                            .corner_radius(4.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                render_input_conflict_lines(
                                    ui, index, &segments, &mask, mine, text_color, action,
                                );
                                ui.add_space(2.0);
                                render_input_buttons(
                                    ui,
                                    side,
                                    index,
                                    &segments,
                                    conflict_edit,
                                    action,
                                );
                            });
                        ui.add_space(6.0);
                    }
                }
            }
        });

    output.state.offset.y
}

/// Render one conflict inside an input pane: this side's changed lines get a
/// checkbox, shared lines are dimmed (always kept), the other side is omitted.
fn render_input_conflict_lines(
    ui: &mut egui::Ui,
    index: usize,
    segments: &[MergeSegment],
    mask: &[bool],
    mine: SegmentOrigin,
    text_color: egui::Color32,
    action: &mut Option<ResultAction>,
) {
    let mut any_line = false;
    for (segment_index, segment) in segments.iter().enumerate() {
        let shown = line_or_space(&segment.text);
        if segment.origin == SegmentOrigin::Common {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(
                    egui::RichText::new(shown)
                        .monospace()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        } else if segment.origin == mine {
            any_line = true;
            let keep = mask.get(segment_index).copied().unwrap_or(false);
            if line_checkbox(ui, keep, shown, text_color) {
                *action = Some(ResultAction::Toggle(index, segment_index));
            }
        }
    }
    if !any_line {
        ui.label(
            egui::RichText::new("(no lines on this side)")
                .monospace()
                .italics()
                .color(text_color),
        );
    }
}

fn render_input_buttons(
    ui: &mut egui::Ui,
    side: MergeSide,
    index: usize,
    segments: &[MergeSegment],
    conflict_edit: &mut Option<ConflictEdit>,
    action: &mut Option<ResultAction>,
) {
    let (accept_label, accept_choice) = match side {
        MergeSide::Current => ("Take all current", ConflictChoice::Ours),
        MergeSide::Incoming => ("Take all incoming", ConflictChoice::Theirs),
    };

    ui.horizontal_wrapped(|ui| {
        if ui.button(accept_label).clicked() {
            *action = Some(ResultAction::Choose(index, accept_choice));
        }
        if ui.button("Both").clicked() {
            *action = Some(ResultAction::Choose(index, ConflictChoice::Both));
        }
        if ui
            .button("Clear")
            .on_hover_text("Untick every line on this conflict")
            .clicked()
        {
            let cleared = segments
                .iter()
                .map(|segment| segment.origin == SegmentOrigin::Common)
                .collect();
            *action = Some(ResultAction::Choose(index, ConflictChoice::Picked(cleared)));
        }
        if ui
            .button("Edit")
            .on_hover_text("Type a custom resolution")
            .clicked()
        {
            let current = segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            *conflict_edit = Some(ConflictEdit {
                index,
                buffer: current,
            });
        }
    });
}

fn render_plain_lines(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    for line in text.lines() {
        ui.label(
            egui::RichText::new(line_or_space(line))
                .monospace()
                .color(color),
        );
    }
}
