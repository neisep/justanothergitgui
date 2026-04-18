use eframe::egui;

use crate::shared::actions::UiAction;
use crate::shared::conflicts::{ConflictChoice, ConflictPart};
use crate::state::{CenterView, InspectorState, RepoState, UiState, WorktreeState};

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

fn show_conflict_view(ui: &mut egui::Ui, state: &mut DiffPanelState<'_>) {
    let mut save_clicked = false;
    let mut all_resolved = true;

    if let Some(data) = &mut state.inspector.conflict_data {
        ui.horizontal(|ui| {
            ui.strong(format!("Conflict: {}", &data.path));
            ui.colored_label(
                egui::Color32::from_rgb(255, 150, 50),
                "Resolve all conflicts then save",
            );
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("conflict_scroll")
            .show(ui, |ui| {
                for section in &mut data.sections {
                    match section {
                        ConflictPart::Common(text) => {
                            for line in text.lines() {
                                ui.label(egui::RichText::new(line).monospace());
                            }
                        }
                        ConflictPart::Conflict {
                            ours,
                            theirs,
                            resolution,
                        } => {
                            if *resolution == ConflictChoice::Unresolved {
                                all_resolved = false;
                            }

                            ui.add_space(4.0);

                            let ours_frame = egui::Frame::new()
                                .fill(egui::Color32::from_rgba_premultiplied(0, 80, 0, 40))
                                .corner_radius(4.0)
                                .inner_margin(6.0);
                            ours_frame.show(ui, |ui| {
                                ui.strong("Ours:");
                                for line in ours.lines() {
                                    ui.label(
                                        egui::RichText::new(line)
                                            .monospace()
                                            .color(egui::Color32::from_rgb(80, 200, 80)),
                                    );
                                }
                            });

                            let theirs_frame = egui::Frame::new()
                                .fill(egui::Color32::from_rgba_premultiplied(80, 0, 0, 40))
                                .corner_radius(4.0)
                                .inner_margin(6.0);
                            theirs_frame.show(ui, |ui| {
                                ui.strong("Theirs:");
                                for line in theirs.lines() {
                                    ui.label(
                                        egui::RichText::new(line)
                                            .monospace()
                                            .color(egui::Color32::from_rgb(220, 80, 80)),
                                    );
                                }
                            });

                            ui.horizontal(|ui| {
                                let is = |c: &ConflictChoice, t: ConflictChoice| *c == t;
                                if ui
                                    .selectable_label(
                                        is(resolution, ConflictChoice::Ours),
                                        "Accept Ours",
                                    )
                                    .clicked()
                                {
                                    *resolution = ConflictChoice::Ours;
                                }
                                if ui
                                    .selectable_label(
                                        is(resolution, ConflictChoice::Theirs),
                                        "Accept Theirs",
                                    )
                                    .clicked()
                                {
                                    *resolution = ConflictChoice::Theirs;
                                }
                                if ui
                                    .selectable_label(
                                        is(resolution, ConflictChoice::Both),
                                        "Accept Both",
                                    )
                                    .clicked()
                                {
                                    *resolution = ConflictChoice::Both;
                                }
                            });

                            ui.add_space(4.0);
                            ui.separator();
                        }
                    }
                }
            });

        ui.add_space(8.0);
        ui.add_enabled_ui(all_resolved, |ui| {
            if ui
                .button("Save Resolution")
                .on_hover_text(if all_resolved {
                    "Write resolved file and stage it"
                } else {
                    "Resolve all conflicts first"
                })
                .clicked()
            {
                save_clicked = true;
            }
        });
    }

    if save_clicked {
        state
            .ui_state
            .actions
            .push(UiAction::save_conflict_resolution());
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
