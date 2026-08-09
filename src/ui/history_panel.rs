use std::path::Path;

use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::shared::actions::UiAction;
use crate::shared::git::CommitEntry;
use crate::state::UiState;

use super::HoveredRow;

const GRAPH_COL_WIDTH: f32 = 24.0;
const OID_COL_WIDTH: f32 = 76.0;
const META_COL_WIDTH: f32 = 200.0;

pub struct HistoryPanelView<'a> {
    pub repo_path: Option<&'a Path>,
    pub commit_history: &'a [CommitEntry],
    pub ui_state: &'a mut UiState,
}

pub fn show(ui: &mut egui::Ui, view: HistoryPanelView<'_>) {
    if view.commit_history.is_empty() {
        let (title, hint) = if view.repo_path.is_none() {
            (
                "No repository open",
                "Use the top bar to open or clone a repository.",
            )
        } else {
            (
                "No commits yet",
                "Stage some files and commit — your history will appear here.",
            )
        };

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.35);
            ui.weak(title);
            ui.add_space(4.0);
            let weak = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(hint).small().color(weak));
        });
        return;
    }

    let last_index = view.commit_history.len().saturating_sub(1);
    let row_height = ui.spacing().interact_size.y.max(22.0);

    ui.push_id("history_rows", |ui| {
        super::prepare_clickable_rows(ui);
        let mut hover = HoveredRow::load(ui, "hover");

        TableBuilder::new(ui)
            .id_salt("history_table")
            .striped(true)
            .sense(egui::Sense::click())
            .column(Column::exact(GRAPH_COL_WIDTH))
            .column(Column::exact(OID_COL_WIDTH))
            .column(Column::remainder().clip(true))
            .column(Column::exact(META_COL_WIDTH))
            .body(|body| {
                body.rows(row_height, view.commit_history.len(), |mut row| {
                    let index = row.index();
                    let commit = &view.commit_history[index];
                    row.set_hovered(hover.is_hovered(index));

                    row.col(|ui| {
                        let graph_color = if commit.is_merge {
                            egui::Color32::from_rgb(180, 100, 255)
                        } else if !commit.branch_labels.is_empty() {
                            egui::Color32::from_rgb(100, 200, 100)
                        } else {
                            egui::Color32::from_gray(150)
                        };
                        draw_graph_lane(ui, index, last_index, commit.is_merge, graph_color);
                    });

                    row.col(|ui| {
                        ui.monospace(
                            egui::RichText::new(&commit.short_oid)
                                .color(egui::Color32::from_gray(170)),
                        );
                    });

                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(&commit.message).truncate());
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
                        });
                    });

                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.weak(&commit.time);
                            ui.weak(" \u{2022} ");
                            ui.add(
                                egui::Label::new(egui::RichText::new(&commit.author).weak())
                                    .truncate(),
                            );
                        });
                    });

                    let response = row.response();
                    hover.observe(index, &response);
                    response
                        .clone()
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        view.ui_state
                            .actions
                            .push(UiAction::select_commit(commit.oid.clone()));
                    }
                });
            });

        hover.store(ui);
    });
}

fn draw_graph_lane(
    ui: &mut egui::Ui,
    index: usize,
    last_index: usize,
    is_merge: bool,
    color: egui::Color32,
) {
    let lane_size = egui::vec2(GRAPH_COL_WIDTH, ui.spacing().interact_size.y.max(22.0));
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
