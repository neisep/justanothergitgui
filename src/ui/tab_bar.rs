use eframe::egui;

/// Read-only view of the open repository tabs, passed to [`show`].
pub struct TabBarView<'a> {
    /// One `(title, full-path tooltip)` pair per tab, in display order.
    pub labels: &'a [(String, Option<String>)],
    /// Index of the currently active tab.
    pub active: usize,
}

/// What the user did to the tab bar this frame. The caller applies these as
/// deferred actions; the widget itself never mutates app state.
#[derive(Default)]
pub struct TabBarResponse {
    pub selected: Option<usize>,
    pub closed: Option<usize>,
    pub open_new: bool,
}

const PAD_X: f32 = 10.0;
const PAD_Y: f32 = 5.0;
const LABEL_CLOSE_GAP: f32 = 6.0;
const CLOSE_SIZE: f32 = 14.0;
/// Total horizontal gap between the last tab and the "new tab" button.
const NEW_TAB_GAP: f32 = 4.0;
/// Width of the "+" hit box. Kept narrow so the glyph, which is centred in it, sits
/// close to the strip; the height follows the tab row so the two line up.
const NEW_TAB_WIDTH: f32 = 20.0;
const NEW_TAB_CORNER: u8 = 4;
const NEW_TAB_GLYPH: f32 = 16.0;

/// Render the browser-style repository tab strip and report interactions.
pub fn show(ui: &mut egui::Ui, view: &TabBarView) -> TabBarResponse {
    let mut out = TabBarResponse::default();

    // One height for the whole strip, so every tab and the "+" button line up.
    let row_height = ui
        .text_style_height(&egui::TextStyle::Button)
        .max(CLOSE_SIZE)
        + PAD_Y * 2.0;

    egui::ScrollArea::horizontal()
        .id_salt("repo_tabs_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (index, (title, tooltip)) in view.labels.iter().enumerate() {
                    draw_tab(
                        ui,
                        index,
                        title,
                        tooltip.as_deref(),
                        index == view.active,
                        row_height,
                        &mut out,
                    );
                }

                draw_new_tab_button(ui, view.labels.is_empty(), row_height, &mut out);
            });
        });

    out
}

/// Draw the "open another repository" button as a square control that sits clearly
/// beside the strip: wider gap, all-round corners and a hover fill of its own, so it
/// never reads as the trailing padding of the last tab.
fn draw_new_tab_button(
    ui: &mut egui::Ui,
    strip_is_empty: bool,
    row_height: f32,
    out: &mut TabBarResponse,
) {
    if !strip_is_empty {
        // The last tab already advanced the cursor by one `item_spacing`; add the rest.
        ui.add_space((NEW_TAB_GAP - ui.spacing().item_spacing.x).max(0.0));
    }

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(NEW_TAB_WIDTH, row_height), egui::Sense::click());

    let visuals = ui.visuals().clone();
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(NEW_TAB_CORNER),
            visuals.widgets.hovered.weak_bg_fill,
        );
    }

    let text_color = if hovered {
        visuals.strong_text_color()
    } else {
        visuals.weak_text_color()
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(NEW_TAB_GLYPH),
        text_color,
    );

    if response.on_hover_text("Open a repository").clicked() {
        out.open_new = true;
    }
}

fn draw_tab(
    ui: &mut egui::Ui,
    index: usize,
    title: &str,
    tooltip: Option<&str>,
    is_active: bool,
    row_height: f32,
    out: &mut TabBarResponse,
) {
    let visuals = ui.visuals().clone();
    let font_id = egui::TextStyle::Button.resolve(ui.style());

    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font_id, visuals.text_color());
    let text_size = galley.size();

    let tab_width = PAD_X + text_size.x + LABEL_CLOSE_GAP + CLOSE_SIZE + PAD_X;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(tab_width, row_height), egui::Sense::click());

    let fill = if is_active {
        visuals.panel_fill
    } else if response.hovered() {
        visuals.widgets.hovered.weak_bg_fill
    } else {
        visuals.faint_bg_color
    };

    // Clone so the painter borrow does not conflict with the later `ui.interact`.
    let painter = ui.painter().clone();
    painter.rect_filled(rect, egui::CornerRadius::ZERO, fill);

    if is_active {
        painter.hline(
            rect.x_range(),
            rect.top() + 1.0,
            egui::Stroke::new(2.0, visuals.selection.bg_fill),
        );
    }

    let text_color = if is_active {
        visuals.strong_text_color()
    } else {
        visuals.weak_text_color()
    };
    let text_pos = egui::pos2(rect.left() + PAD_X, rect.center().y - text_size.y / 2.0);
    painter.galley(text_pos, galley, text_color);

    let close_rect = egui::Rect::from_min_size(
        egui::pos2(
            rect.right() - PAD_X - CLOSE_SIZE,
            rect.center().y - CLOSE_SIZE / 2.0,
        ),
        egui::vec2(CLOSE_SIZE, CLOSE_SIZE),
    );
    // `interact` instead of `ui.put`: putting a widget rewinds the layout cursor to
    // that widget's rect, which ends `PAD_X` before the tab does — every following tab
    // and the "+" button would then be laid out on top of this tab.
    let close_response = ui.interact(
        close_rect,
        ui.id().with(("tab_close", index)),
        egui::Sense::click(),
    );
    painter.text(
        close_rect.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        egui::FontId::proportional(CLOSE_SIZE),
        if close_response.hovered() {
            visuals.strong_text_color()
        } else {
            visuals.weak_text_color()
        },
    );
    if close_response.clicked() {
        out.closed = Some(index);
    }

    let response = match tooltip {
        Some(path) => response.on_hover_text(path),
        None => response,
    };

    // Guard so clicking the close button never doubles as a tab selection.
    if response.clicked() && out.closed != Some(index) {
        out.selected = Some(index);
    }
    if response.middle_clicked() {
        out.closed = Some(index);
    }
}
