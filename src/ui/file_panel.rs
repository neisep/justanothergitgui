use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::state::{AppState, DragFile, FileEntry, UiAction};

const STATUS_COL_WIDTH: f32 = 72.0;
const ACTION_COL_WIDTH: f32 = 72.0;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let mut unstaged_rect = egui::Rect::NOTHING;
    let mut staged_rect = egui::Rect::NOTHING;

    egui::Panel::left("file_panel")
        .default_size(240.0)
        .min_size(200.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Unstaged ({})", state.unstaged.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !state.unstaged.is_empty()
                        && ui
                            .small_button("Stage All")
                            .on_hover_text("Stage all changes")
                            .clicked()
                    {
                        state.actions.push(UiAction::StageAll);
                    }
                });
            });
            ui.separator();

            let first_list_height = (ui.available_height() - 8.0).max(0.0) / 2.0;
            unstaged_rect = render_file_table(ui, state, false, first_list_height);

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.strong(format!("Staged ({})", state.staged.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !state.staged.is_empty()
                        && ui
                            .small_button("Unstage All")
                            .on_hover_text("Unstage all changes")
                            .clicked()
                    {
                        state.actions.push(UiAction::UnstageAll);
                    }
                });
            });
            ui.separator();

            staged_rect = render_file_table(ui, state, true, ui.available_height());

            handle_drop(ui, state, unstaged_rect, staged_rect);
        });

    show_drag_ghost(ui.ctx(), state);
}

fn render_file_table(
    ui: &mut egui::Ui,
    state: &mut AppState,
    staged: bool,
    max_height: f32,
) -> egui::Rect {
    let files = if staged {
        state.staged.clone()
    } else {
        state.unstaged.clone()
    };
    let row_height = ui.spacing().interact_size.y.max(28.0);

    if files.is_empty() {
        return render_empty_section(ui, state, staged, max_height);
    }

    TableBuilder::new(ui)
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
            header.col(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak("Status");
                });
            });
            header.col(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak("Quick action");
                });
            });
        })
        .body(|body| {
            body.rows(row_height, files.len(), |mut row| {
                let file = &files[row.index()];
                let is_selected = state.selected_file.as_ref().is_some_and(|selected| {
                    selected.path == file.path && selected.staged == staged
                });
                row.set_selected(is_selected);

                let mut action_clicked = false;
                let mut drag_started = false;

                row.col(|ui| {
                    let label = if file.is_conflicted {
                        egui::RichText::new(&file.path).color(egui::Color32::from_rgb(255, 170, 80))
                    } else if is_selected {
                        egui::RichText::new(&file.path).strong()
                    } else {
                        egui::RichText::new(&file.path)
                    };
                    ui.add(egui::Label::new(label).truncate());
                });

                row.col(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        render_status_badge(ui, file);
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
                            state.dragging = Some(DragFile {
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
                                state.actions.push(UiAction::UnstageFile(file.path.clone()));
                            } else {
                                state.actions.push(UiAction::StageFile(file.path.clone()));
                            }
                        }
                    });
                });

                let row_response = row.response().clone();
                row_response
                    .clone()
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                if row_response.clicked() && !action_clicked && !drag_started {
                    state.actions.push(UiAction::SelectFile {
                        path: file.path.clone(),
                        staged,
                    });
                }
            });
        })
        .inner_rect
}

fn render_empty_section(
    ui: &mut egui::Ui,
    state: &AppState,
    staged: bool,
    max_height: f32,
) -> egui::Rect {
    let (title, hint) = if staged {
        if state.unstaged.is_empty() {
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
    } else if state.staged.is_empty() {
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
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, height),
        egui::Sense::hover(),
    );

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

fn render_status_badge(ui: &mut egui::Ui, file: &FileEntry) {
    let (fill, text) = if file.is_conflicted {
        (egui::Color32::from_rgb(160, 92, 32), "CONFLICT")
    } else {
        match file.display_status.as_str() {
            "new" => (egui::Color32::from_rgb(48, 128, 88), "NEW"),
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
    state: &mut AppState,
    unstaged_rect: egui::Rect,
    staged_rect: egui::Rect,
) {
    let pointer_released = ui.input(|i| i.pointer.any_released());
    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    let drag_info = state.dragging.clone();

    if let Some(drag) = &drag_info {
        let target_rect = if drag.from_staged {
            unstaged_rect
        } else {
            staged_rect
        };

        if let Some(pos) = hover_pos {
            if target_rect.contains(pos) {
                ui.painter().rect_stroke(
                    target_rect,
                    4.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 200, 80)),
                    egui::StrokeKind::Outside,
                );
            }
        }

        if pointer_released {
            if let Some(pos) = hover_pos {
                if target_rect.contains(pos) {
                    if drag.from_staged {
                        state.actions.push(UiAction::UnstageFile(drag.path.clone()));
                    } else {
                        state.actions.push(UiAction::StageFile(drag.path.clone()));
                    }
                }
            }
        }
    }

    if pointer_released {
        state.dragging = None;
    }
}

fn show_drag_ghost(ctx: &egui::Context, state: &AppState) {
    if let Some(drag) = &state.dragging {
        if let Some(pos) = ctx.pointer_hover_pos() {
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
}
