use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::shared::actions::UiAction;
use crate::shared::git::FileEntry;
use crate::state::{DragFile, InspectorState, UiState, WorktreeState};

use super::HoveredRow;
use super::diff_view;

const STATUS_COL_WIDTH: f32 = 24.0;
const ACTION_COL_WIDTH: f32 = 72.0;
const PANEL_DEFAULT_WIDTH: f32 = 300.0;
const PANEL_MIN_WIDTH: f32 = 220.0;
/// Neither section may be squeezed below this, however the divider is dragged.
const MIN_SECTION_HEIGHT: f32 = 110.0;
/// The Unstaged section never claims more than this share of the panel on its
/// own, so the Staged list — and its drop target — is always visible.
const MAX_UNSTAGED_FRACTION: f32 = 0.65;
/// Height of a section's header row plus its separator.
const SECTION_CHROME: f32 = 32.0;
const CONFLICT_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 170, 80);

pub struct FilePanelState<'a> {
    pub worktree: &'a WorktreeState,
    pub inspector: &'a mut InspectorState,
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, mut state: FilePanelState<'_>) {
    let mut unstaged_rect = egui::Rect::NOTHING;
    let mut staged_rect = egui::Rect::NOTHING;

    // Copied out so the filtered lists borrow the worktree rather than `state`,
    // which the tables below need to borrow mutably.
    let worktree = state.worktree;

    egui::Panel::left("file_panel")
        .default_size(PANEL_DEFAULT_WIDTH)
        .min_size(PANEL_MIN_WIDTH)
        .show_inside(ui, |ui| {
            show_filter_row(ui, state.inspector);

            let filter = state.inspector.file_filter.clone();
            let unstaged = filtered(&worktree.unstaged, &filter);
            let staged = filtered(&worktree.staged, &filter);
            let row_height = row_height(ui);

            let unstaged_height =
                preferred_unstaged_height(unstaged.len(), row_height, ui.available_height());

            egui::Panel::top("unstaged_section")
                .resizable(true)
                .default_size(unstaged_height)
                .min_size(MIN_SECTION_HEIGHT)
                .show_inside(ui, |ui| {
                    // egui stores the panel's *content* rect each frame and uses
                    // it as next frame's size, ignoring `default_size` from then
                    // on. Content shorter than the section would therefore shrink
                    // it a little every frame, and a shorter section clips its
                    // content, so it could never grow back — filtering the list
                    // down to one row would strand the divider there for good.
                    // Claiming the whole height keeps the stored size the one the
                    // user actually set.
                    ui.set_min_height(ui.available_height());

                    show_section_header(
                        ui,
                        SectionHeader {
                            title: "Unstaged",
                            shown: unstaged.len(),
                            total: worktree.unstaged.len(),
                            button: "Stage All",
                            tooltip: "Stage all changes",
                        },
                        state.ui_state,
                        UiAction::stage_all,
                    );
                    unstaged_rect = show_file_list(ui, &mut state, &unstaged, false);
                });

            ui.add_space(4.0);

            show_section_header(
                ui,
                SectionHeader {
                    title: "Staged",
                    shown: staged.len(),
                    total: worktree.staged.len(),
                    button: "Unstage All",
                    tooltip: "Unstage all changes",
                },
                state.ui_state,
                UiAction::unstage_all,
            );
            staged_rect = show_file_list(ui, &mut state, &staged, true);

            handle_drop(ui, &mut state, unstaged_rect, staged_rect);
        });

    show_drag_ghost(ui.ctx(), &state);
}

fn row_height(ui: &egui::Ui) -> f32 {
    ui.spacing().interact_size.y.max(28.0)
}

/// Case-insensitive substring match on the whole path, so both a directory and a
/// filename fragment narrow the list. An empty filter matches everything.
fn matches_filter(path: &str, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return true;
    }

    path.to_lowercase().contains(&filter.to_lowercase())
}

fn filtered<'a>(files: &'a [FileEntry], filter: &str) -> Vec<&'a FileEntry> {
    files
        .iter()
        .filter(|file| matches_filter(&file.path, filter))
        .collect()
}

/// Split a repository-relative path into the filename and its directory.
///
/// The filename is what identifies a row, so it is rendered first and the
/// directory after it — the reverse of a plain truncating path label, which cuts
/// off exactly the half that matters.
fn split_display_path(path: &str) -> (&str, &str) {
    match path.rsplit_once('/') {
        // A trailing slash leaves nothing to name the row with; show the path as
        // it is rather than an empty label.
        Some((_, "")) => (path, ""),
        Some((dir, name)) => (name, dir),
        None => (path, ""),
    }
}

/// How tall the Unstaged section starts out: enough for its rows, but never so
/// tall that the Staged list below it disappears.
///
/// Only the very first frame's size. egui stores the panel's size after that and
/// ignores this, which is what makes a dragged divider stay put.
fn preferred_unstaged_height(rows: usize, row_height: f32, available: f32) -> f32 {
    // `rows + 1` covers the table's own header row.
    let content = SECTION_CHROME + row_height * (rows as f32 + 1.0);
    let max = (available * MAX_UNSTAGED_FRACTION).max(MIN_SECTION_HEIGHT);
    let min = MIN_SECTION_HEIGHT.min(max);

    content.clamp(min, max)
}

fn show_filter_row(ui: &mut egui::Ui, inspector: &mut InspectorState) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !inspector.file_filter.is_empty()
                && ui
                    .small_button("\u{2715}")
                    .on_hover_text("Clear the filter")
                    .clicked()
            {
                inspector.file_filter.clear();
            }

            ui.add(
                egui::TextEdit::singleline(&mut inspector.file_filter)
                    .desired_width(f32::INFINITY)
                    .hint_text("Filter files..."),
            );
        });
    });
    ui.add_space(4.0);
}

struct SectionHeader<'a> {
    title: &'a str,
    shown: usize,
    total: usize,
    button: &'a str,
    tooltip: &'a str,
}

fn show_section_header(
    ui: &mut egui::Ui,
    header: SectionHeader<'_>,
    ui_state: &mut UiState,
    action: fn() -> UiAction,
) {
    let filtered = header.shown != header.total;
    ui.horizontal(|ui| {
        if filtered {
            ui.strong(format!(
                "{} ({} of {})",
                header.title, header.shown, header.total
            ));
        } else {
            ui.strong(format!("{} ({})", header.title, header.total));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if header.total > 0
                && ui
                    .small_button(header.button)
                    .on_hover_text(if filtered {
                        // Bulk actions ignore the filter; saying so beats a
                        // surprise when a hidden file turns out to be included.
                        format!("{} — including files hidden by the filter", header.tooltip)
                    } else {
                        header.tooltip.to_string()
                    })
                    .clicked()
            {
                ui_state.actions.push(action());
            }
        });
    });
    ui.separator();
}

/// Render one section's list, empty state included.
fn show_file_list(
    ui: &mut egui::Ui,
    state: &mut FilePanelState<'_>,
    files: &[&FileEntry],
    staged: bool,
) -> egui::Rect {
    let max_height = ui.available_height();

    if files.is_empty() {
        return render_empty_section(ui, state, staged, max_height);
    }

    render_file_table(
        ui,
        FileTable {
            files,
            inspector: state.inspector,
            ui_state: state.ui_state,
        },
        staged,
        max_height,
    )
}

/// The file list borrowed apart, so the table can hold the entries and mutate the
/// inspector at the same time.
struct FileTable<'a, 'f> {
    files: &'a [&'f FileEntry],
    inspector: &'a mut InspectorState,
    ui_state: &'a mut UiState,
}

fn render_file_table(
    ui: &mut egui::Ui,
    table: FileTable<'_, '_>,
    staged: bool,
    max_height: f32,
) -> egui::Rect {
    let FileTable {
        files,
        inspector,
        ui_state,
    } = table;
    let row_height = row_height(ui);

    let scope_id = if staged {
        "staged_file_rows"
    } else {
        "unstaged_file_rows"
    };

    ui.push_id(scope_id, |ui| {
        super::prepare_clickable_rows(ui);
        let mut hover = HoveredRow::load(ui, "hover");

        let inner_rect = TableBuilder::new(ui)
            .id_salt(if staged {
                "staged_file_table"
            } else {
                "unstaged_file_table"
            })
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::remainder().at_least(100.0).clip(true))
            .column(Column::exact(STATUS_COL_WIDTH))
            .column(Column::exact(ACTION_COL_WIDTH))
            .min_scrolled_height(0.0)
            .max_scroll_height(max_height.max(row_height * 2.0))
            .header(row_height, |mut header| {
                header.col(|ui| {
                    ui.weak("File");
                });
                header.col(|_ui| {});
                header.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak("Quick action");
                    });
                });
            })
            .body(|body| {
                body.rows(row_height, files.len(), |mut row| {
                    let index = row.index();
                    let file = files[index];
                    let is_selected = inspector.selected_file.as_ref().is_some_and(|selected| {
                        selected.path == file.path && selected.staged == staged
                    });
                    row.set_selected(is_selected);
                    row.set_hovered(hover.is_hovered(index));

                    let mut action_clicked = false;
                    let mut drag_started = false;

                    row.col(|ui| {
                        render_path(ui, file, is_selected);
                    });

                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            diff_view::render_status_chip(
                                ui,
                                &file.display_status,
                                file.is_conflicted,
                            );
                        });
                    });

                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let handle = drag_handle(ui);
                            handle
                                .clone()
                                .on_hover_cursor(egui::CursorIcon::Grab)
                                .on_hover_text(if staged {
                                    "Drag to move this file to unstaged"
                                } else {
                                    "Drag to move this file to staged"
                                });
                            if handle.drag_started() {
                                drag_started = true;
                                inspector.dragging = Some(DragFile {
                                    path: file.path.clone(),
                                    from_staged: staged,
                                });
                            }

                            let (btn_label, btn_tooltip) = if staged {
                                (
                                    "Unstage",
                                    "Unstage this file\nShortcut: Ctrl/Cmd+S when selected",
                                )
                            } else {
                                (
                                    "Stage",
                                    "Stage this file\nShortcut: Ctrl/Cmd+S when selected",
                                )
                            };
                            if ui
                                .small_button(btn_label)
                                .on_hover_text(btn_tooltip)
                                .clicked()
                            {
                                action_clicked = true;
                                if staged {
                                    ui_state
                                        .actions
                                        .push(UiAction::unstage_file(file.path.clone()));
                                } else {
                                    ui_state
                                        .actions
                                        .push(UiAction::stage_file(file.path.clone()));
                                }
                            }
                        });
                    });

                    let row_response = row.response().clone();
                    hover.observe(index, &row_response);
                    row_response
                        .clone()
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        // The row shows the filename first and elides the rest,
                        // so the full path has to stay reachable somewhere.
                        .on_hover_text(&file.path);

                    if row_response.clicked() && !action_clicked && !drag_started {
                        ui_state
                            .actions
                            .push(UiAction::select_file(file.path.clone(), staged));
                    }
                });
            })
            .inner_rect;

        hover.store(ui);
        inner_rect
    })
    .inner
}

/// Paint `src/ui/file_panel.rs` as a strong `file_panel.rs` followed by a dimmed
/// `src/ui`. The directory is the label that runs out of room first, which is
/// the right thing to lose in a narrow panel.
fn render_path(ui: &mut egui::Ui, file: &FileEntry, is_selected: bool) {
    let (name, dir) = split_display_path(&file.path);

    let name_text = if file.is_conflicted {
        egui::RichText::new(name).color(CONFLICT_TEXT)
    } else if is_selected {
        egui::RichText::new(name).strong()
    } else {
        egui::RichText::new(name)
    };

    ui.spacing_mut().item_spacing.x = 6.0;
    ui.add(egui::Label::new(name_text).truncate());

    if !dir.is_empty() {
        ui.add(
            egui::Label::new(
                egui::RichText::new(dir)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            )
            .truncate(),
        );
    }
}

fn render_empty_section(
    ui: &mut egui::Ui,
    state: &FilePanelState<'_>,
    staged: bool,
    max_height: f32,
) -> egui::Rect {
    let worktree = state.worktree;
    let list = if staged {
        &worktree.staged
    } else {
        &worktree.unstaged
    };

    // A list emptied by the filter is a different situation from one that is
    // genuinely empty, and the hint that fits one is misleading for the other.
    let (title, hint) = if !list.is_empty() {
        ("No matches", "No file here matches the filter above.")
    } else if staged {
        if worktree.unstaged.is_empty() {
            (
                "Nothing staged yet",
                "Edit a file in your project — changes will show up here.",
            )
        } else {
            (
                "Nothing staged yet",
                "Click Stage, or drag a file from Unstaged above.",
            )
        }
    } else if worktree.staged.is_empty() {
        (
            "Working tree is clean",
            "Edit any file in your project to see it here.",
        )
    } else {
        (
            "All changes are staged",
            "Write a message on the right and commit when ready.",
        )
    };

    let width = ui.available_width();
    let height = max_height.clamp(72.0, 140.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());

    let painter = ui.painter_at(rect);
    let weak = ui.visuals().weak_text_color();
    let strong = ui.visuals().text_color();
    let center = rect.center();

    painter.text(
        egui::pos2(center.x, center.y - 10.0),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(13.0),
        strong,
    );
    painter.text(
        egui::pos2(center.x, center.y + 10.0),
        egui::Align2::CENTER_CENTER,
        hint,
        egui::FontId::proportional(11.0),
        weak,
    );

    rect
}

fn drag_handle(ui: &mut egui::Ui) -> egui::Response {
    let size = egui::vec2(16.0, 18.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::drag());
    let color = if response.dragged() {
        ui.visuals().widgets.active.fg_stroke.color
    } else if response.hovered() {
        ui.visuals().widgets.hovered.fg_stroke.color
    } else {
        ui.visuals().widgets.noninteractive.fg_stroke.color
    };

    let painter = ui.painter();
    let center = rect.center();
    for offset_x in [-3.0, 3.0] {
        for offset_y in [-4.0, 0.0, 4.0] {
            painter.circle_filled(
                egui::pos2(center.x + offset_x, center.y + offset_y),
                1.2,
                color,
            );
        }
    }

    response
}

fn handle_drop(
    ui: &mut egui::Ui,
    state: &mut FilePanelState<'_>,
    unstaged_rect: egui::Rect,
    staged_rect: egui::Rect,
) {
    let pointer_released = ui.input(|i| i.pointer.any_released());
    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    let drag_info = state.inspector.dragging.clone();

    if let Some(drag) = &drag_info {
        let target_rect = if drag.from_staged {
            unstaged_rect
        } else {
            staged_rect
        };

        if let Some(pos) = hover_pos
            && target_rect.contains(pos)
        {
            ui.painter().rect_stroke(
                target_rect,
                4.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 200, 80)),
                egui::StrokeKind::Outside,
            );
        }

        if pointer_released
            && let Some(pos) = hover_pos
            && target_rect.contains(pos)
        {
            if drag.from_staged {
                state
                    .ui_state
                    .actions
                    .push(UiAction::unstage_file(drag.path.clone()));
            } else {
                state
                    .ui_state
                    .actions
                    .push(UiAction::stage_file(drag.path.clone()));
            }
        }
    }

    if pointer_released {
        state.inspector.dragging = None;
    }
}

fn show_drag_ghost(ctx: &egui::Context, state: &FilePanelState<'_>) {
    if let Some(drag) = &state.inspector.dragging
        && let Some(pos) = ctx.pointer_hover_pos()
    {
        egui::Area::new(egui::Id::new("drag_ghost"))
            .order(egui::Order::Tooltip)
            .fixed_pos(pos + egui::vec2(12.0, 12.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let arrow = if drag.from_staged {
                        "\u{2191} "
                    } else {
                        "\u{2193} "
                    };
                    ui.label(format!("{}{}", arrow, &drag.path));
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_SECTION_HEIGHT, matches_filter, preferred_unstaged_height, split_display_path,
    };

    #[test]
    fn split_display_path_puts_the_filename_first() {
        assert_eq!(
            split_display_path("src/ui/file_panel.rs"),
            ("file_panel.rs", "src/ui")
        );
    }

    #[test]
    fn split_display_path_handles_a_bare_filename() {
        assert_eq!(split_display_path("README.md"), ("README.md", ""));
    }

    #[test]
    fn split_display_path_keeps_a_trailing_slash_visible() {
        // Nothing after the slash means there is no name to lead with; showing
        // the path unchanged beats rendering an empty row.
        assert_eq!(split_display_path("src/ui/"), ("src/ui/", ""));
        assert_eq!(split_display_path(""), ("", ""));
    }

    #[test]
    fn an_empty_filter_matches_every_path() {
        assert!(matches_filter("src/main.rs", ""));
        assert!(matches_filter("src/main.rs", "   "));
    }

    #[test]
    fn filtering_ignores_case_and_matches_directories() {
        assert!(matches_filter("src/UI/File_Panel.rs", "file_panel"));
        assert!(matches_filter("src/ui/file_panel.rs", "SRC/UI"));
        assert!(!matches_filter("src/ui/file_panel.rs", "worker"));
    }

    #[test]
    fn a_short_list_does_not_shrink_below_the_section_minimum() {
        assert_eq!(
            preferred_unstaged_height(0, 28.0, 800.0),
            MIN_SECTION_HEIGHT
        );
    }

    #[test]
    fn a_long_list_leaves_room_for_the_staged_section() {
        let available = 800.0;
        let height = preferred_unstaged_height(500, 28.0, available);

        assert!(
            height <= available * super::MAX_UNSTAGED_FRACTION,
            "unstaged claimed {height} of {available}"
        );
    }

    #[test]
    fn a_cramped_panel_still_yields_a_usable_height() {
        // `available` smaller than the minimum must not invert the clamp bounds.
        let height = preferred_unstaged_height(3, 28.0, 40.0);

        assert_eq!(height, MIN_SECTION_HEIGHT);
    }
}
