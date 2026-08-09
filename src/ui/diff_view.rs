//! Read-only diff renderers shared by the Changes tab and the commit view.
//!
//! Nothing here owns state or emits actions: every function takes already-parsed
//! rows and paints them. That is what keeps the commit view strictly read-only.

use eframe::egui;

use crate::shared::diff::{DiffLineKind, ParsedDiffLine, SideBySideEntry, SideCell};

const LINE_NUMBER_WIDTH: f32 = 44.0;

/// The classic unified patch table: old / new / change badge / content.
pub(crate) fn show_diff_table(ui: &mut egui::Ui, rows: &[ParsedDiffLine], wrap_lines: bool) {
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
                render_diff_content(ui, &row.content, row.kind, wrap_lines);
                ui.end_row();
            }
        });
}

/// A side-by-side diff document rendered as two panes sharing one vertical
/// scroll offset, old on the left and new on the right.
pub(crate) struct SideBySideView<'a> {
    pub entries: &'a [SideBySideEntry],
    pub scroll: f32,
}

/// Paint the two panes and return the scroll offset they should share next
/// frame — whichever pane the user actually moved wins.
pub(crate) fn show_side_by_side(ui: &mut egui::Ui, view: SideBySideView<'_>) -> f32 {
    let scroll = view.scroll;
    let mut offsets = [scroll; 2];

    ui.columns(2, |columns| {
        offsets[0] = render_pane(&mut columns[0], view.entries, scroll, Side::Old);
        offsets[1] = render_pane(&mut columns[1], view.entries, scroll, Side::New);
    });

    if (offsets[0] - scroll).abs() > 0.5 {
        offsets[0]
    } else if (offsets[1] - scroll).abs() > 0.5 {
        offsets[1]
    } else {
        scroll
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Old,
    New,
}

impl Side {
    fn title(self) -> &'static str {
        match self {
            Self::Old => "Before",
            Self::New => "After",
        }
    }

    fn scroll_id(self) -> &'static str {
        match self {
            Self::Old => "side_by_side_old",
            Self::New => "side_by_side_new",
        }
    }

    fn grid_id(self) -> &'static str {
        match self {
            Self::Old => "side_by_side_old_grid",
            Self::New => "side_by_side_new_grid",
        }
    }

    fn cell(self, entry: &SideBySideEntry) -> Option<&SideCell> {
        match entry {
            SideBySideEntry::Row(row) => Some(match self {
                Self::Old => &row.old,
                Self::New => &row.new,
            }),
            SideBySideEntry::Header(_) => None,
        }
    }
}

/// Rows are never wrapped: both panes must produce identical row heights for the
/// two sides to stay aligned, so long lines scroll horizontally instead.
///
/// Only the visible rows are built. A commit touching a few thousand lines would
/// otherwise spend every frame emitting widgets for rows nobody can see, twice
/// over — once per pane.
fn render_pane(ui: &mut egui::Ui, entries: &[SideBySideEntry], scroll: f32, side: Side) -> f32 {
    ui.push_id(side.scroll_id(), |ui| {
        ui.weak(side.title());
        ui.separator();

        // Every row is one unwrapped monospace line, so this is exact — which is
        // what `show_rows` needs to map a scroll offset to a row range.
        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);

        let output = egui::ScrollArea::both()
            .id_salt(side.scroll_id())
            .auto_shrink([false, false])
            .vertical_scroll_offset(scroll)
            .show_rows(ui, row_height, entries.len(), |ui, range| {
                egui::Grid::new(side.grid_id())
                    .num_columns(2)
                    .spacing([8.0, ui.spacing().item_spacing.y])
                    .striped(true)
                    // Keeps the stripe phase tied to the absolute row index, so
                    // the banding does not invert as the range scrolls.
                    .start_row(range.start)
                    .min_col_width(LINE_NUMBER_WIDTH)
                    .show(ui, |ui| {
                        for entry in &entries[range] {
                            match side.cell(entry) {
                                Some(SideCell::Line {
                                    number,
                                    kind,
                                    content,
                                }) => {
                                    render_line_number(ui, *number);
                                    render_diff_content(ui, content, *kind, false);
                                }
                                Some(SideCell::Empty) => {
                                    render_line_number(ui, None);
                                    ui.weak(egui::RichText::new(" ").monospace());
                                }
                                None => {
                                    render_line_number(ui, None);
                                    render_header_line(ui, entry);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });

        output.state.offset.y
    })
    .inner
}

fn render_header_line(ui: &mut egui::Ui, entry: &SideBySideEntry) {
    let SideBySideEntry::Header(text) = entry else {
        return;
    };
    let kind = crate::shared::diff::classify_diff_line(text);
    render_diff_content(ui, text, kind, false);
}

pub(crate) fn render_line_number(ui: &mut egui::Ui, line_number: Option<usize>) {
    let text = line_number.map(|line| line.to_string()).unwrap_or_default();
    ui.label(
        egui::RichText::new(text)
            .monospace()
            .color(egui::Color32::from_gray(140)),
    );
}

pub(crate) fn render_diff_badge(ui: &mut egui::Ui, kind: DiffLineKind) {
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

pub(crate) fn render_diff_content(
    ui: &mut egui::Ui,
    content: &str,
    kind: DiffLineKind,
    wrap_lines: bool,
) {
    let content = if content.is_empty() { " " } else { content };
    let mut label = egui::Label::new(
        egui::RichText::new(content)
            .monospace()
            .color(diff_line_color(kind, ui)),
    );
    label = if wrap_lines {
        label.wrap()
    } else {
        label.extend()
    };
    ui.add(label);
}

pub(crate) fn diff_line_color(kind: DiffLineKind, ui: &egui::Ui) -> egui::Color32 {
    match kind {
        DiffLineKind::Added => egui::Color32::from_rgb(120, 230, 160),
        DiffLineKind::Removed => egui::Color32::from_rgb(255, 150, 150),
        DiffLineKind::HunkHeader => egui::Color32::from_rgb(150, 200, 255),
        DiffLineKind::FileHeader => egui::Color32::from_gray(210),
        DiffLineKind::Note => egui::Color32::from_rgb(255, 220, 120),
        DiffLineKind::Context | DiffLineKind::Other => ui.style().visuals.text_color(),
    }
}

/// Status badge shared by the working-tree file list and the commit file list.
pub(crate) fn render_status_badge(ui: &mut egui::Ui, display_status: &str, is_conflicted: bool) {
    let (fill, text) = if is_conflicted {
        (egui::Color32::from_rgb(160, 92, 32), "CONFLICT")
    } else {
        match display_status {
            "new" => (egui::Color32::from_rgb(48, 128, 88), "NEW"),
            "untracked" => (egui::Color32::from_rgb(48, 128, 88), "ADDED"),
            "modified" => (egui::Color32::from_rgb(52, 96, 160), "MODIFIED"),
            "deleted" => (egui::Color32::from_rgb(152, 64, 64), "DELETED"),
            "renamed" => (egui::Color32::from_rgb(108, 76, 156), "RENAMED"),
            _ => (egui::Color32::from_rgb(92, 92, 92), "CHANGED"),
        }
    };

    egui::Frame::new()
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .small()
                    .color(egui::Color32::WHITE),
            );
        });
}
