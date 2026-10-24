pub mod agents_panel;
pub mod bottom_bar;
pub mod change_set_view;
pub mod commit_panel;
pub mod commit_view;
pub mod dialogs;
pub mod diff_panel;
pub mod diff_view;
pub mod file_panel;
pub mod history_panel;
pub mod review_view;
pub mod right_panel;
pub mod tab_bar;
pub mod task_details_panel;
pub mod worktree_chips;
pub mod worktree_panel;

#[cfg(test)]
mod ux_tests;

use eframe::egui;

/// Prepare a `Ui` to host a table whose whole rows are clickable.
///
/// egui labels are selectable by default, which makes them sense click-and-drag.
/// Inside a table cell that puts every label *on top of* the row, so the row's own
/// response never sees the pointer: `contains_pointer()` is true while `hovered()`
/// and `clicked()` stay false, and the row reads as a dead, invisible button.
/// Turning selection off for the rows' subtree hands the interaction back to them.
pub fn prepare_clickable_rows(ui: &mut egui::Ui) {
    ui.style_mut().interaction.selectable_labels = false;
}

/// Which row of a table the pointer was over on the previous frame.
///
/// A table row paints its background before any of its cells exist, but the row's
/// own response only exists after them — so the hovered index has to be carried
/// across frames. `egui_extras` does this internally yet never asks for the extra
/// repaint, so its highlight only flickers while the pointer moves and disappears
/// the moment it stops, which makes a clickable row look like an invisible button.
/// Tracking the index here lets us request that repaint and keep the row lit.
///
/// Use it around any table whose rows are clickable:
///
/// ```ignore
/// let mut hover = HoveredRow::load(ui, "my_rows");
/// TableBuilder::new(ui).sense(egui::Sense::click()).body(|body| {
///     body.rows(height, count, |mut row| {
///         let index = row.index();
///         row.set_hovered(hover.is_hovered(index)); // before the first `col`
///         row.col(|ui| { /* ... */ });
///         hover.observe(index, &row.response());
///     });
/// });
/// hover.store(ui);
/// ```
pub struct HoveredRow {
    id: egui::Id,
    previous: Option<usize>,
    current: Option<usize>,
}

impl HoveredRow {
    /// `salt` separates tables that share one parent `Ui`.
    pub fn load(ui: &egui::Ui, salt: &str) -> Self {
        let id = ui.id().with(salt);
        let previous = ui.data(|data| data.get_temp::<usize>(id));
        Self {
            id,
            previous,
            current: None,
        }
    }

    /// Whether this row should paint itself as hovered. Call before adding cells.
    pub fn is_hovered(&self, index: usize) -> bool {
        self.previous == Some(index)
    }

    /// Record the row the pointer turned out to be over. Call after adding cells.
    pub fn observe(&mut self, index: usize, response: &egui::Response) {
        if response.hovered() {
            self.current = Some(index);
        }
    }

    /// Persist for the next frame, repainting once more when the row changed so the
    /// highlight settles even if the pointer has come to a stop.
    pub fn store(self, ui: &egui::Ui) {
        if self.current != self.previous {
            ui.ctx().request_repaint();
        }

        ui.data_mut(|data| match self.current {
            Some(index) => {
                data.insert_temp(self.id, index);
            }
            None => {
                data.remove_temp::<usize>(self.id);
            }
        });
    }
}

/// A small filled pill carrying one short word.
///
/// Painted rather than assembled from glyph characters: the bundled fonts have
/// no `●`/`✓`/`✗`, and a missing glyph renders as a box that the headless tests
/// — which scrape painted *text* — would happily accept.
pub fn render_pill(ui: &mut egui::Ui, text: &str, fill: egui::Color32) -> egui::Response {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(text)
                        .small()
                        .color(egui::Color32::WHITE),
                )
                .truncate(),
            );
        })
        .response
}

pub fn show_inline_busy(ui: &mut egui::Ui, label: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(12.0));
        ui.label(egui::RichText::new(label).small().weak());
    });
}
