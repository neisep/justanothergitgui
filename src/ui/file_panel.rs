use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::state::{AppState, DragFile, FileEntry, UiAction};

const CONTROLS_COL_WIDTH: f32 = 64.0;
const STATUS_COL_WIDTH: f32 = 96.0;

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let mut unstaged_rect = egui::Rect::NOTHING;
    let mut staged_rect = egui::Rect::NOTHING;

    egui::Panel::left("file_panel")
        .default_size(280.0)
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
    let row_height = ui.spacing().interact_size.y.max(24.0);
    let empty_msg = if staged {
        "No staged changes"
    } else {
        "No unstaged changes"
    };

    TableBuilder::new(ui)
        .id_salt(if staged {
            "staged_file_table"
        } else {
            "unstaged_file_table"
        })
        .striped(true)
        .sense(egui::Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(CONTROLS_COL_WIDTH))
        .column(Column::remainder().at_least(120.0).clip(true))
        .column(Column::exact(STATUS_COL_WIDTH))
        .min_scrolled_height(0.0)
        .max_scroll_height(max_height.max(row_height * 2.0))
        .header(row_height, |mut header| {
            header.col(|ui| {
                ui.weak("Action");
            });
            header.col(|ui| {
                ui.weak("File");
            });
            header.col(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak("Status");
                });
            });
        })
        .body(|mut body| {
            if files.is_empty() {
                body.row(row_height, |mut row| {
                    row.col(|_ui| {});
                    row.col(|ui| {
                        ui.weak(empty_msg);
                    });
                    row.col(|_ui| {});
                });
                return;
            }

            body.rows(row_height, files.len(), |mut row| {
                let file = &files[row.index()];
                let is_selected = state.selected_file.as_ref().is_some_and(|selected| {
                    selected.path == file.path && selected.staged == staged
                });
                row.set_selected(is_selected);

                let mut action_clicked = false;
                let mut drag_started = false;

                row.col(|ui| {
                    ui.horizontal(|ui| {
                        let handle = ui.add(
                            egui::Label::new(egui::RichText::new("\u{2801}\u{2801}").weak())
                                .sense(egui::Sense::drag()),
                        );
                        if handle.drag_started() {
                            drag_started = true;
                            state.dragging = Some(DragFile {
                                path: file.path.clone(),
                                from_staged: staged,
                            });
                        }

                        let (btn_label, btn_tooltip) = if staged {
                            ("-", "Unstage this file")
                        } else {
                            ("+", "Stage this file")
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

                row.col(|ui| {
                    let label = if file.is_conflicted {
                        egui::RichText::new(&file.path).color(egui::Color32::from_rgb(255, 170, 80))
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

                if row.response().clicked() && !action_clicked && !drag_started {
                    state.actions.push(UiAction::SelectFile {
                        path: file.path.clone(),
                        staged,
                    });
                }
            });
        })
        .inner_rect
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
