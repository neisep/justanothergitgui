use eframe::egui;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::{
    ConflictChoice, ConflictData, ConflictPart, MergeSegment, SegmentOrigin,
};
use crate::state::{CenterView, ConflictEdit, InspectorState, RepoState, UiState, WorktreeState};

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
        CenterView::History => super::history_panel::show(
            ui,
            super::history_panel::HistoryPanelView {
                repo_path: state.repo.path.as_deref(),
                commit_history: &state.repo.commit_history,
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

                show_diff_table(ui, &rows, state.inspector.diff_wrap);
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
// The merge base (common ancestor) pane — a muted tan that reads as neutral
// context between the green "current" and blue "incoming" sides.
const BASE_TEXT: egui::Color32 = egui::Color32::from_rgb(190, 175, 135);
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
        // Slot a read-only Base pane between the two editable sides whenever we
        // recovered the ancestor text, turning the editor into a true 3-way view.
        let has_base = data
            .sections
            .iter()
            .any(|section| matches!(section, ConflictPart::Conflict { base: Some(_), .. }));
        if has_base {
            ui.columns(3, |columns| {
                offsets[0] = render_input_pane(
                    &mut columns[0],
                    data,
                    MergeSide::Current,
                    scroll,
                    conflict_edit,
                    &mut pane_action,
                );
                render_base_pane(&mut columns[1], data, scroll);
                offsets[1] = render_input_pane(
                    &mut columns[2],
                    data,
                    MergeSide::Incoming,
                    scroll,
                    conflict_edit,
                    &mut pane_action,
                );
            });
        } else {
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
        }

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

/// Render the read-only Base (ancestor) column: agreed lines as dimmed text and,
/// for each conflict, the ancestor version of that hunk framed for reference.
fn render_base_pane(ui: &mut egui::Ui, data: &ConflictData, scroll_offset: f32) {
    ui.colored_label(BASE_TEXT, egui::RichText::new("Base (ancestor)").strong());
    ui.weak("the common starting point — read only");
    ui.separator();

    let weak = ui.visuals().weak_text_color();
    egui::ScrollArea::vertical()
        .id_salt("merge_in_base")
        .vertical_scroll_offset(scroll_offset)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for section in &data.sections {
                match section {
                    ConflictPart::Common(text) => render_plain_lines(ui, text, weak),
                    ConflictPart::Conflict { base, .. } => {
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0, BASE_TEXT))
                            .corner_radius(4.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let note = match base {
                                    Some(text) if !text.is_empty() => {
                                        render_plain_lines(ui, text, weak);
                                        None
                                    }
                                    Some(_) => Some("(absent in base — added on both sides)"),
                                    None => Some("(base unavailable)"),
                                };
                                if let Some(note) = note {
                                    ui.label(
                                        egui::RichText::new(note).monospace().italics().color(weak),
                                    );
                                }
                            });
                        ui.add_space(6.0);
                    }
                }
            }
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

fn show_diff_table(ui: &mut egui::Ui, rows: &[ParsedDiffLine], wrap_lines: bool) {
    egui::Grid::new("diff_grid")
        .num_columns(4)
        .spacing([8.0, 3.0])
        .striped(true)
        .show(ui, |ui| {
            ui.weak(egui::RichText::new("old").monospace());
            ui.weak(egui::RichText::new("new").monospace());
            ui.weak(egui::RichText::new("chg").monospace());
            ui.weak(egui::RichText::new("content").monospace());
            ui.end_row();

            for row in rows {
                render_line_number(ui, row.old_line_number);
                render_line_number(ui, row.new_line_number);
                render_diff_badge(ui, row.kind);
                render_diff_content(ui, row, wrap_lines);
                ui.end_row();
            }
        });
}

fn render_line_number(ui: &mut egui::Ui, line_number: Option<usize>) {
    let text = line_number.map(|line| line.to_string()).unwrap_or_default();
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .color(egui::Color32::from_gray(140)),
    );
}

fn render_diff_badge(ui: &mut egui::Ui, kind: DiffLineKind) {
    let (fill, text_color, label) = match kind {
        DiffLineKind::Added => (
            egui::Color32::from_rgba_premultiplied(32, 110, 64, 72),
            egui::Color32::from_rgb(120, 230, 160),
            "ADD",
        ),
        DiffLineKind::Removed => (
            egui::Color32::from_rgba_premultiplied(140, 48, 48, 72),
            egui::Color32::from_rgb(255, 150, 150),
            "DEL",
        ),
        DiffLineKind::HunkHeader => (
            egui::Color32::from_rgba_premultiplied(52, 90, 140, 72),
            egui::Color32::from_rgb(150, 200, 255),
            "HUNK",
        ),
        DiffLineKind::FileHeader => (
            egui::Color32::from_rgba_premultiplied(90, 90, 90, 56),
            egui::Color32::from_gray(220),
            "META",
        ),
        DiffLineKind::Note => (
            egui::Color32::from_rgba_premultiplied(132, 100, 28, 72),
            egui::Color32::from_rgb(255, 220, 120),
            "NOTE",
        ),
        DiffLineKind::Context | DiffLineKind::Other => {
            ui.weak(egui::RichText::new(" ").monospace());
            return;
        }
    };

    egui::Frame::new()
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .small()
                    .strong()
                    .color(text_color),
            );
        });
}

fn render_diff_content(ui: &mut egui::Ui, row: &ParsedDiffLine, wrap_lines: bool) {
    let content = if row.content.is_empty() {
        " "
    } else {
        &row.content
    };
    let mut label = egui::Label::new(
        egui::RichText::new(content)
            .monospace()
            .color(diff_line_color(row.kind, ui)),
    );
    label = if wrap_lines {
        label.wrap()
    } else {
        label.extend()
    };
    ui.add(label);
}

fn diff_line_color(kind: DiffLineKind, ui: &egui::Ui) -> egui::Color32 {
    match kind {
        DiffLineKind::Added => egui::Color32::from_rgb(120, 230, 160),
        DiffLineKind::Removed => egui::Color32::from_rgb(255, 150, 150),
        DiffLineKind::HunkHeader => egui::Color32::from_rgb(150, 200, 255),
        DiffLineKind::FileHeader => egui::Color32::from_gray(210),
        DiffLineKind::Note => egui::Color32::from_rgb(255, 220, 120),
        DiffLineKind::Context | DiffLineKind::Other => ui.style().visuals.text_color(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiffLineKind {
    Context,
    Added,
    Removed,
    HunkHeader,
    FileHeader,
    Note,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedDiffLine {
    old_line_number: Option<usize>,
    new_line_number: Option<usize>,
    kind: DiffLineKind,
    content: String,
}

fn parse_diff_rows(diff_content: &str) -> Vec<ParsedDiffLine> {
    let mut rows = Vec::new();
    let mut old_line_number = None;
    let mut new_line_number = None;

    for line in diff_content.lines() {
        let kind = classify_diff_line(line);

        if kind == DiffLineKind::HunkHeader {
            if let Some((old_start, new_start)) = parse_hunk_header(line) {
                old_line_number = Some(old_start);
                new_line_number = Some(new_start);
            }
        }

        let row = match kind {
            DiffLineKind::Context => {
                let old = old_line_number;
                let new = new_line_number;
                old_line_number = old_line_number.map(|line| line + 1);
                new_line_number = new_line_number.map(|line| line + 1);
                ParsedDiffLine {
                    old_line_number: old,
                    new_line_number: new,
                    kind,
                    content: line[1..].to_string(),
                }
            }
            DiffLineKind::Added => {
                let new = new_line_number;
                new_line_number = new_line_number.map(|line| line + 1);
                ParsedDiffLine {
                    old_line_number: None,
                    new_line_number: new,
                    kind,
                    content: line[1..].to_string(),
                }
            }
            DiffLineKind::Removed => {
                let old = old_line_number;
                old_line_number = old_line_number.map(|line| line + 1);
                ParsedDiffLine {
                    old_line_number: old,
                    new_line_number: None,
                    kind,
                    content: line[1..].to_string(),
                }
            }
            _ => ParsedDiffLine {
                old_line_number: None,
                new_line_number: None,
                kind,
                content: line.to_string(),
            },
        };

        rows.push(row);
    }

    rows
}

fn classify_diff_line(line: &str) -> DiffLineKind {
    if line.starts_with("@@") {
        DiffLineKind::HunkHeader
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("similarity index ")
    {
        DiffLineKind::FileHeader
    } else if line.starts_with("\\ ") {
        DiffLineKind::Note
    } else if line.starts_with('+') {
        DiffLineKind::Added
    } else if line.starts_with('-') {
        DiffLineKind::Removed
    } else if line.starts_with(' ') {
        DiffLineKind::Context
    } else {
        DiffLineKind::Other
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let mut parts = line.split_whitespace();
    if parts.next()? != "@@" {
        return None;
    }

    let old_range = parts.next()?;
    let new_range = parts.next()?;
    if parts.next()? != "@@" {
        return None;
    }

    Some((
        parse_hunk_range(old_range, '-')?,
        parse_hunk_range(new_range, '+')?,
    ))
}

fn parse_hunk_range(range: &str, expected_prefix: char) -> Option<usize> {
    let trimmed = range.strip_prefix(expected_prefix)?;
    trimmed.split(',').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{DiffLineKind, parse_diff_rows, parse_hunk_header};

    #[test]
    fn parses_hunk_start_line_numbers() {
        assert_eq!(
            parse_hunk_header("@@ -14,3 +20,7 @@ fn render()"),
            Some((14, 20))
        );
    }

    #[test]
    fn assigns_old_and_new_line_numbers_to_diff_rows() {
        let rows = parse_diff_rows(concat!(
            "diff --git a/src/app.rs b/src/app.rs\n",
            "@@ -10,2 +10,3 @@\n",
            " line one\n",
            "-line removed\n",
            "+line added\n",
            "+line added too\n",
        ));

        assert_eq!(rows[0].kind, DiffLineKind::FileHeader);
        assert_eq!(rows[1].kind, DiffLineKind::HunkHeader);
        assert_eq!(rows[2].old_line_number, Some(10));
        assert_eq!(rows[2].new_line_number, Some(10));
        assert_eq!(rows[3].kind, DiffLineKind::Removed);
        assert_eq!(rows[3].old_line_number, Some(11));
        assert_eq!(rows[3].new_line_number, None);
        assert_eq!(rows[4].kind, DiffLineKind::Added);
        assert_eq!(rows[4].old_line_number, None);
        assert_eq!(rows[4].new_line_number, Some(11));
        assert_eq!(rows[5].new_line_number, Some(12));
    }
}
