use eframe::egui;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::{
    ConflictChoice, ConflictData, ConflictPart, Eol, MergeSegment, SegmentOrigin,
};
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

    match state.inspector.center_view {
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
        // Already parsed when the file was selected — this only paints.
        let parsed = &state.inspector.parsed_diff;

        ui.horizontal(|ui| {
            ui.strong(&sel.path);
            ui.weak(if sel.staged { "(staged)" } else { "(unstaged)" });
            ui.separator();
            ui.weak(format!(
                "+{} / -{}",
                parsed.added_lines, parsed.removed_lines
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut state.inspector.diff_wrap, "Wrap lines");
            });
        });
        ui.separator();

        if state.inspector.diff_content.is_empty() {
            ui.weak("No diff available (file may be binary or new)");
            return;
        }

        diff_view::show_diff_table(
            ui,
            &state.inspector.parsed_diff.rows,
            state.inspector.diff_wrap,
        );
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
    let save_error = state.inspector.resolution_save_error();
    let save_clicked = {
        let InspectorState {
            conflict_data,
            conflict_edit,
            conflict_scroll,
            conflict_focus,
            ..
        } = &mut *state.inspector;

        let Some(data) = conflict_data.as_mut() else {
            return;
        };

        let unresolved = data.unresolved_count();

        let mut save_clicked = false;
        ui.vertical(|ui| {
            ui.strong(format!("Conflict: {}", &data.path));
            if unresolved > 0 {
                ui.colored_label(
                    UNRESOLVED_ACCENT,
                    format!("{unresolved} conflict(s) left — pick a side or edit"),
                );
            } else {
                ui.colored_label(CURRENT_TEXT, "Resolution selected — not saved");
            }
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    save_clicked = ui
                        .add_enabled(
                            save_error.is_none(),
                            egui::Button::new("Save and stage resolution")
                                .fill(ui.visuals().selection.bg_fill),
                        )
                        .on_hover_text(save_error.unwrap_or("Write the merged file and stage it"))
                        .clicked();
                });
            });
        });
        if conflict_edit.is_some() {
            ui.colored_label(EDIT_ACCENT, "Apply or cancel your edit before saving.");
        }
        let count = data
            .sections()
            .iter()
            .filter(|part| matches!(part, ConflictPart::Conflict { .. }))
            .count();
        *conflict_focus = (*conflict_focus).min(count.saturating_sub(1));
        let mut scroll_target = None;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(*conflict_focus > 0, egui::Button::new("Previous conflict"))
                .clicked()
            {
                *conflict_focus -= 1;
                scroll_target = Some(*conflict_focus);
            }
            ui.label(format!(
                "Conflict {} of {count}",
                if count == 0 { 0 } else { *conflict_focus + 1 }
            ));
            if ui
                .add_enabled(
                    *conflict_focus + 1 < count,
                    egui::Button::new("Next conflict"),
                )
                .clicked()
            {
                *conflict_focus += 1;
                scroll_target = Some(*conflict_focus);
            }
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
                result_action = render_result_document(ui, data, conflict_edit, scroll_target);
            });
        apply_result_action(data, result_action, ui.ctx());

        // Top: the two input panes, sharing one vertical scroll offset. Each
        // line the user can choose from has its own checkbox; ticking a line
        // adds it to the result below.
        let scroll = *conflict_scroll;
        let mut pane_action: Option<ResultAction> = None;
        let mut offsets = [scroll; 2];
        let source_rect = ui.available_rect_before_wrap();
        ui.columns(2, |columns| {
            for column in columns.iter_mut() {
                column.set_clip_rect(column.clip_rect().intersect(source_rect));
            }
            offsets[0] = render_input_pane(
                &mut columns[0],
                data,
                MergeSide::Current,
                scroll,
                conflict_edit.is_none(),
                scroll_target,
                &mut pane_action,
            );
            offsets[1] = render_input_pane(
                &mut columns[1],
                data,
                MergeSide::Incoming,
                scroll,
                conflict_edit.is_none(),
                scroll_target,
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
    if let Some((index, choice)) = choice {
        data.set_resolution(index, choice);
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
    scroll_target: Option<usize>,
) -> Option<ResultAction> {
    let mut action: Option<ResultAction> = None;

    egui::ScrollArea::vertical()
        .id_salt("merge_result_doc")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut conflict_no = 0usize;
            for (index, section) in data.sections().iter().enumerate() {
                match section {
                    ConflictPart::Common(text) => {
                        render_plain_lines(ui, text, ui.visuals().text_color());
                    }
                    ConflictPart::Conflict { resolution, .. } => {
                        if scroll_target == Some(conflict_no) {
                            ui.scroll_to_cursor(Some(egui::Align::TOP));
                        }
                        conflict_no += 1;
                        ui.strong(format!("Conflict {conflict_no}"));
                        render_resolution_buttons(ui, data, index, conflict_edit, &mut action);
                        if conflict_edit
                            .as_ref()
                            .is_some_and(|edit| edit.index == index)
                        {
                            render_edit_zone(ui, conflict_no, conflict_edit, &mut action);
                        } else if let ConflictChoice::Custom(text) = resolution {
                            conflict_zone_frame(ui, CUSTOM_ACCENT, |ui| {
                                if text.is_empty() {
                                    ui.label("Region removed — neither side kept");
                                } else {
                                    render_plain_lines(ui, text, ui.visuals().text_color());
                                }
                            });
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
    if matches!(
        data.sections()[index],
        ConflictPart::Conflict {
            resolution: ConflictChoice::Both,
            ..
        }
    ) {
        render_plain_lines(ui, &data.resolution_text(index), ui.visuals().text_color());
        return;
    }
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

/// Seed text for the inline editor.
///
/// The multiline text box only ever inserts `\n`, so the buffer is kept in LF
/// throughout and put back on the file's own terminator when the edit is stored
/// (see `ConflictData::set_resolution`). Feeding it CRLF would leave stray `\r`
/// characters sitting in the text the user is typing into.
fn edit_buffer(text: &str) -> String {
    Eol::Lf.normalize(text)
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
    picking_enabled: bool,
    scroll_target: Option<usize>,
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

    let source = match side {
        MergeSide::Current => &data.current_label,
        MergeSide::Incoming => &data.incoming_label,
    };
    let title = source
        .as_ref()
        .map_or_else(|| title.to_string(), |source| format!("{title}: {source}"));
    ui.colored_label(text_color, egui::RichText::new(title).strong());
    ui.weak("tick the lines you want in the result");
    ui.separator();

    let output = egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .min_scrolled_height(0.0)
        .max_height(ui.available_height())
        .vertical_scroll_offset(scroll_offset)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut conflict_no = 0;
            for (index, section) in data.sections().iter().enumerate() {
                match section {
                    ConflictPart::Common(text) => {
                        render_plain_lines(ui, text, ui.visuals().weak_text_color())
                    }
                    ConflictPart::Conflict { resolution, .. } => {
                        if scroll_target == Some(conflict_no) {
                            ui.scroll_to_cursor(Some(egui::Align::TOP));
                        }
                        conflict_no += 1;
                        let Some((segments, mask)) = data.conflict_segments(index) else {
                            continue;
                        };
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0, text_color))
                            .corner_radius(4.0)
                            .inner_margin(6.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(format!("Conflict {conflict_no}"));
                                let custom = matches!(resolution, ConflictChoice::Custom(_));
                                ui.add_enabled_ui(picking_enabled && !custom, |ui| {
                                    render_input_conflict_lines(
                                        ui, index, segments, mask, mine, text_color, action,
                                    );
                                });
                                if custom {
                                    ui.weak(
                                        "Custom result — use Reset resolution to pick lines again.",
                                    );
                                }
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

fn render_resolution_buttons(
    ui: &mut egui::Ui,
    data: &ConflictData,
    index: usize,
    conflict_edit: &mut Option<ConflictEdit>,
    action: &mut Option<ResultAction>,
) {
    ui.add_enabled_ui(conflict_edit.is_none(), |ui| {
        ui.horizontal_wrapped(|ui| {
            for (label, choice) in [
                ("Use current", ConflictChoice::Ours),
                ("Use incoming", ConflictChoice::Theirs),
                ("Keep both", ConflictChoice::Both),
                ("Reset resolution", ConflictChoice::Unresolved),
                ("Keep neither", ConflictChoice::Custom(String::new())),
            ] {
                let hint = match label {
                    "Keep both" => {
                        "Keep the entire current region followed by the entire incoming region."
                    }
                    "Reset resolution" => {
                        "Remove this choice and mark the conflict unresolved again."
                    }
                    "Keep neither" => {
                        "Delete this entire conflict region from the result and mark it resolved."
                    }
                    _ => "Replace this conflict region with the selected side.",
                };
                if ui.button(label).on_hover_text(hint).clicked() {
                    *action = Some(ResultAction::Choose(index, choice));
                }
            }
            if ui.button("Edit result").clicked() {
                *conflict_edit = Some(ConflictEdit {
                    index,
                    buffer: edit_buffer(&data.resolution_text(index)),
                });
            }
        });
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

#[cfg(test)]
mod merge_render_tests {
    use super::*;
    use crate::shared::conflicts::FileStyle;
    use crate::state::AppState;
    use eframe::egui::{Event, Pos2, Rect, Shape};
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
            for _ in 0..20 {
                self.frame(vec![], draw);
            }
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

    fn state() -> AppState {
        let mut state = AppState::default();
        let mut data = ConflictData::new(
            "merge.rs".into(),
            vec![
                ConflictPart::Conflict {
                    ours: "ours".into(),
                    theirs: "theirs".into(),
                    resolution: ConflictChoice::Unresolved,
                },
                ConflictPart::Common(
                    (0..60)
                        .map(|n| format!("context {n}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                ConflictPart::Conflict {
                    ours: "second ours".into(),
                    theirs: "second theirs".into(),
                    resolution: ConflictChoice::Unresolved,
                },
            ],
            FileStyle::default(),
        );
        data.current_label = Some("main".into());
        data.incoming_label = Some("feature".into());
        state.inspector.set_conflict(Some(data));
        state
    }

    fn draw(ui: &mut egui::Ui, state: &mut AppState) {
        show(
            ui,
            DiffPanelState {
                repo: &state.repo,
                worktree: &state.worktree,
                inspector: &mut state.inspector,
                ui_state: &mut state.ui,
            },
        );
    }

    fn assert_visible(painted: &[PaintedText], text: &str, width: f32) {
        let label = label(painted, text);
        assert!(
            label.clip.expand(1.0).contains_rect(label.rect),
            "{text} clipped: {:?}, {:?}",
            label.rect,
            label.clip
        );
        assert!(
            Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 820.0)).contains_rect(label.rect),
            "{text} outside viewport: {:?}",
            label.rect
        );
    }

    #[test]
    fn merge_header_keeps_sources_result_and_navigation_in_viewport() {
        for width in [640.0, 960.0] {
            let mut state = state();
            let mut harness = Harness::new(width);
            let painted = harness.settled(&mut |ui| draw(ui, &mut state));
            for text in [
                "Current (ours): main",
                "Incoming (theirs): feature",
                "Result",
                "Previous conflict",
                "Next conflict",
                "Save and stage resolution",
            ] {
                assert_visible(&painted, text, width);
            }
            let result_top = label(&painted, "Result").rect.top();
            for source in ["Current (ours): main", "Incoming (theirs): feature"] {
                assert!(
                    label(&painted, source).clip.bottom() <= result_top,
                    "source pane clip must stop before Result header"
                );
            }
        }
    }

    #[test]
    fn navigation_scrolls_to_second_conflict_and_back() {
        let mut state = state();
        let mut harness = Harness::new(960.0);
        let painted = harness.settled(&mut |ui| draw(ui, &mut state));
        let next = label(&painted, "Next conflict").rect.center();
        let painted = harness.click(next, &mut |ui| draw(ui, &mut state));
        assert_eq!(state.inspector.conflict_focus, 1);
        let second = painted
            .iter()
            .find(|item| item.text == "Conflict 2" && item.clip.contains_rect(item.rect));
        assert!(second.is_some(), "second conflict must scroll into view");
        let previous = label(&painted, "Previous conflict").rect.center();
        let painted = harness.click(previous, &mut |ui| draw(ui, &mut state));
        assert_eq!(state.inspector.conflict_focus, 0);
        assert!(
            painted
                .iter()
                .any(|item| item.text == "Conflict 1" && item.clip.contains_rect(item.rect))
        );
    }

    #[test]
    fn editing_blocks_save_until_apply_and_dispatches_selected_preview() {
        let mut state = state();
        state
            .inspector
            .conflict_data
            .as_mut()
            .unwrap()
            .set_resolution(0, ConflictChoice::Theirs);
        state
            .inspector
            .conflict_data
            .as_mut()
            .unwrap()
            .set_resolution(2, ConflictChoice::Ours);
        let mut harness = Harness::new(960.0);
        let painted = harness.settled(&mut |ui| draw(ui, &mut state));
        let edit = painted
            .iter()
            .find(|item| item.text == "Edit result" && item.clip.contains_rect(item.rect))
            .unwrap()
            .rect
            .center();
        let painted = harness.click(edit, &mut |ui| draw(ui, &mut state));
        assert_eq!(
            state.inspector.conflict_edit.as_ref().unwrap().buffer,
            "theirs"
        );
        assert_visible(&painted, "Apply or cancel your edit before saving.", 960.0);
        let save = label(&painted, "Save and stage resolution").rect.center();
        let painted = harness.click(save, &mut |ui| draw(ui, &mut state));
        assert!(state.ui.actions.is_empty());
        state.inspector.conflict_edit.as_mut().unwrap().buffer = "custom resolution".into();
        let apply = label(&painted, "Apply").rect.center();
        let painted = harness.click(apply, &mut |ui| draw(ui, &mut state));
        assert!(state.inspector.conflict_edit.is_none());
        assert_eq!(
            state
                .inspector
                .conflict_data
                .as_ref()
                .unwrap()
                .resolution_text(0),
            "custom resolution"
        );
        let save = label(&painted, "Save and stage resolution").rect.center();
        harness.click(save, &mut |ui| draw(ui, &mut state));
        assert!(matches!(
            state.ui.actions.as_slice(),
            [UiAction::SaveConflictResolution]
        ));
    }
}
