use eframe::egui;

use crate::state::{AppState, DragFile, UiAction};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    let mut unstaged_rect = egui::Rect::NOTHING;
    let mut staged_rect = egui::Rect::NOTHING;

    egui::Panel::left("file_panel")
        .default_size(260.0)
        .min_size(180.0)
        .show_inside(ui, |ui| {
            // -- Unstaged section --
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

            let unstaged_out = egui::ScrollArea::vertical()
                .id_salt("unstaged_scroll")
                .max_height(ui.available_height() / 2.0 - 20.0)
                .show(ui, |ui| {
                    render_file_list(ui, state, false);
                });
            unstaged_rect = unstaged_out.inner_rect;

            ui.add_space(8.0);

            // -- Staged section --
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

            let staged_out = egui::ScrollArea::vertical()
                .id_salt("staged_scroll")
                .show(ui, |ui| {
                    render_file_list(ui, state, true);
                });
            staged_rect = staged_out.inner_rect;

            // -- Drop zone handling --
            handle_drop(ui, state, unstaged_rect, staged_rect);
        });

    // Drag ghost (rendered on tooltip layer, outside the panel)
    show_drag_ghost(ui.ctx(), state);
}

fn render_file_list(ui: &mut egui::Ui, state: &mut AppState, staged: bool) {
    let files = if staged {
        state.staged.clone()
    } else {
        state.unstaged.clone()
    };

    for file in &files {
        ui.horizontal(|ui| {
            // Drag handle
            let handle = ui.add(egui::Label::new("\u{2801}\u{2801}").sense(egui::Sense::drag()));
            if handle.drag_started() {
                state.dragging = Some(DragFile {
                    path: file.path.clone(),
                    from_staged: staged,
                });
            }

            // Stage/unstage button
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
                if staged {
                    state.actions.push(UiAction::UnstageFile(file.path.clone()));
                } else {
                    state.actions.push(UiAction::StageFile(file.path.clone()));
                }
            }

            // File name
            let is_selected = state
                .selected_file
                .as_ref()
                .is_some_and(|s| s.path == file.path && s.staged == staged);

            let label_text = if file.is_conflicted {
                egui::RichText::new(&file.path).color(egui::Color32::from_rgb(255, 150, 50))
            } else {
                egui::RichText::new(&file.path)
            };

            if ui.selectable_label(is_selected, label_text).clicked() {
                state.actions.push(UiAction::SelectFile {
                    path: file.path.clone(),
                    staged,
                });
            }

            // Status label
            let status_color = if file.is_conflicted {
                egui::Color32::from_rgb(255, 150, 50)
            } else {
                ui.style().visuals.weak_text_color()
            };
            ui.label(egui::RichText::new(format!("[{}]", file.display_status)).color(status_color));
        });
    }

    if files.is_empty() {
        let msg = if staged {
            "No staged changes"
        } else {
            "No unstaged changes"
        };
        ui.weak(msg);
    }
}

fn handle_drop(
    ui: &mut egui::Ui,
    state: &mut AppState,
    unstaged_rect: egui::Rect,
    staged_rect: egui::Rect,
) {
    let pointer_released = ui.input(|i| i.pointer.any_released());
    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    // Clone drag info to avoid borrow conflicts
    let drag_info = state.dragging.clone();

    if let Some(drag) = &drag_info {
        let target_rect = if drag.from_staged {
            unstaged_rect
        } else {
            staged_rect
        };

        // Highlight drop zone
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

        // Handle drop
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
