use eframe::egui;

use crate::state::{AppState, CenterView, ConflictChoice, ConflictPart, UiAction};

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    // View toggle
    ui.horizontal(|ui| {
        if ui
            .selectable_label(state.center_view == CenterView::Diff, "Changes")
            .clicked()
        {
            state.actions.push(UiAction::ShowDiff);
        }
        if ui
            .selectable_label(state.center_view == CenterView::History, "History")
            .clicked()
        {
            state.actions.push(UiAction::ShowHistory);
        }
    });
    ui.separator();

    let view = state.center_view.clone();
    match view {
        CenterView::Diff => show_diff_or_conflict(ui, state),
        CenterView::History => super::history_panel::show(ui, state),
    }
}

fn show_diff_or_conflict(ui: &mut egui::Ui, state: &mut AppState) {
    if state.conflict_data.is_some() {
        show_conflict_view(ui, state);
    } else if state.selected_file.is_some() {
        show_diff_view(ui, state);
    } else {
        ui.centered_and_justified(|ui| {
            ui.weak("Select a file to view changes");
        });
    }
}

fn show_diff_view(ui: &mut egui::Ui, state: &AppState) {
    if let Some(sel) = &state.selected_file {
        ui.horizontal(|ui| {
            ui.strong(&sel.path);
            ui.weak(if sel.staged { "(staged)" } else { "(unstaged)" });
        });
        ui.separator();

        egui::ScrollArea::both()
            .id_salt("diff_scroll")
            .show(ui, |ui| {
                for line in state.diff_content.lines() {
                    let color = diff_line_color(line, ui);
                    ui.label(egui::RichText::new(line).monospace().color(color));
                }

                if state.diff_content.is_empty() {
                    ui.weak("No diff available (file may be binary or new)");
                }
            });
    }
}

fn show_conflict_view(ui: &mut egui::Ui, state: &mut AppState) {
    let mut save_clicked = false;
    let mut all_resolved = true;

    if let Some(data) = &mut state.conflict_data {
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

                            // Ours block
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

                            // Theirs block
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

                            // Resolution buttons
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
        state.actions.push(UiAction::SaveConflictResolution);
    }
}

fn diff_line_color(line: &str, ui: &egui::Ui) -> egui::Color32 {
    if line.starts_with('+') {
        egui::Color32::from_rgb(80, 200, 80)
    } else if line.starts_with('-') {
        egui::Color32::from_rgb(220, 80, 80)
    } else if line.starts_with('@') {
        egui::Color32::from_rgb(100, 160, 255)
    } else {
        ui.style().visuals.text_color()
    }
}
