use eframe::egui;

use crate::state::AppState;

pub fn show(ui: &mut egui::Ui, state: &AppState) {
    if state.commit_history.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.weak("No commit history available");
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("history_scroll")
        .show(ui, |ui| {
            let last_index = state.commit_history.len().saturating_sub(1);
            for (index, commit) in state.commit_history.iter().enumerate() {
                ui.horizontal(|ui| {
                    let graph_color = if commit.is_merge {
                        egui::Color32::from_rgb(180, 100, 255)
                    } else if !commit.branch_labels.is_empty() {
                        egui::Color32::from_rgb(100, 200, 100)
                    } else {
                        egui::Color32::from_gray(150)
                    };
                    draw_graph_lane(ui, index, last_index, commit.is_merge, graph_color);

                    ui.monospace(
                        egui::RichText::new(&commit.short_oid).color(egui::Color32::from_gray(170)),
                    );

                    ui.label(&commit.message);

                    for label in &commit.branch_labels {
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgb(40, 80, 120))
                            .corner_radius(3.0)
                            .inner_margin(egui::Margin::symmetric(4, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(label)
                                        .small()
                                        .color(egui::Color32::WHITE),
                                );
                            });
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(&commit.time);
                        ui.weak(" \u{2022} ");
                        ui.weak(&commit.author);
                    });
                });
            }
        });
}

fn draw_graph_lane(
    ui: &mut egui::Ui,
    index: usize,
    last_index: usize,
    is_merge: bool,
    color: egui::Color32,
) {
    let lane_size = egui::vec2(24.0, ui.spacing().interact_size.y.max(22.0));
    let (rect, _) = ui.allocate_exact_size(lane_size, egui::Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let radius = 4.0;
    let stroke = egui::Stroke::new(1.5, color);

    if index > 0 {
        painter.line_segment(
            [
                egui::pos2(center.x, rect.top()),
                egui::pos2(center.x, center.y - radius),
            ],
            stroke,
        );
    }

    if index < last_index {
        painter.line_segment(
            [
                egui::pos2(center.x, center.y + radius),
                egui::pos2(center.x, rect.bottom()),
            ],
            stroke,
        );
    }

    if is_merge {
        painter.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(center.x, center.y - (radius + 1.0)),
                egui::pos2(center.x + (radius + 1.0), center.y),
                egui::pos2(center.x, center.y + (radius + 1.0)),
                egui::pos2(center.x - (radius + 1.0), center.y),
            ],
            color,
            egui::Stroke::NONE,
        ));
    } else {
        painter.circle_filled(center, radius, color);
    }
}
