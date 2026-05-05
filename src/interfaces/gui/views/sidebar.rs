use std::collections::HashMap;

use egui::{Color32, RichText, Stroke, Ui};

use crate::domain::fanfiction::ReadingStatus;
use crate::domain::shelf::Shelf;

use super::super::view::View;

/// Per-row counts for the Library section. Computed once per frame from the
/// in-memory fic list (cheap; the slice is small).
#[derive(Default)]
pub struct LibraryCounts {
    pub all: usize,
    pub in_progress: usize,
    pub read: usize,
    pub plan_to_read: usize,
    pub paused: usize,
    pub abandoned: usize,
}

pub struct SidebarState<'a> {
    /// In-place — sidebar row clicks just set this to the target view;
    /// the caller diffs against `prev_view` to know whether to refresh
    /// derived caches. (Not folded into `Outcome` because every row
    /// would otherwise have to thread a return value back up the
    /// loop, and the prev/post diff is the simpler shape.)
    pub current_view: &'a mut View,
    pub shelves: &'a [Shelf],
    pub library_counts: &'a LibraryCounts,
    /// Per-shelf fic counts, keyed by shelf id. Missing entries render as 0
    /// (e.g. a freshly created shelf before the next refresh tick).
    pub shelf_counts: &'a HashMap<u64, usize>,
    /// Live count of in-flight background tasks. Always shown next to the
    /// Tasks row (including 0) so the user has a stable place to watch
    /// without flipping into the Tasks view.
    pub running_tasks: usize,
}

/// What the user did this frame, beyond the implicit view change. At
/// most one variant fires per frame — "+" / right-click-delete / drop
/// are mutually-exclusive mouse interactions.
pub enum Outcome {
    None,
    /// User clicked the "+" button next to the SHELVES heading.
    OpenCreateShelfModal,
    /// User picked "Delete shelf" from a shelf row's right-click menu.
    OpenDeleteShelfConfirm(u64),
    /// A drag-and-drop release landed on a shelf row. Caller bulk-adds
    /// the carried fics to the shelf and refreshes derived caches.
    DropOnShelf {
        shelf_id: u64,
        fic_ids: Vec<u64>,
    },
}

pub fn draw(ui: &mut Ui, state: SidebarState<'_>) -> Outcome {
    let SidebarState {
        current_view,
        shelves,
        library_counts,
        shelf_counts,
        running_tasks,
    } = state;
    let mut outcome = Outcome::None;

    // Pin Tasks/Settings to the bottom.
    // `Frame::none()` because the panel's default inner_margin (~8px each
    // side) was making Library + Tasks/Settings rows narrower than the
    // edge-to-edge shelf rows in the (Frame::none) central panel below.
    // With all three panels using Frame::none, the only horizontal inset
    // is INNER_MARGIN_X applied uniformly inside view_row.
    egui::TopBottomPanel::bottom("ficflow-sidebar-bottom")
        .resizable(false)
        .show_separator_line(true)
        .frame(egui::Frame::none())
        .show_inside(ui, |ui| {
            ui.add_space(6.0);
            view_row(
                ui,
                current_view,
                View::Tasks,
                "Tasks",
                None,
                Some(running_tasks),
            );
            view_row(ui, current_view, View::Settings, "Settings", None, None);
            ui.add_space(6.0);
        });

    // Pin LIBRARY section + the SHELVES header to the top so the user
    // never loses access to status filters or the "+ shelf" button when
    // a long shelf list scrolls. Only the shelf rows themselves go in
    // the scrollable Central area below.
    egui::TopBottomPanel::top("ficflow-sidebar-top")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::none())
        .show_inside(ui, |ui| {
            ui.add_space(6.0);
            section_label(ui, "LIBRARY");
            ui.add_space(2.0);
            view_row(
                ui,
                current_view,
                View::AllFics,
                "All Fanfictions",
                Some(LIBRARY_ICON_ALL),
                Some(library_counts.all),
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::InProgress),
                "In Progress",
                Some(LIBRARY_ICON_IN_PROGRESS),
                Some(library_counts.in_progress),
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::Read),
                "Read",
                Some(LIBRARY_ICON_READ),
                Some(library_counts.read),
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::PlanToRead),
                "Plan to Read",
                Some(LIBRARY_ICON_PLAN),
                Some(library_counts.plan_to_read),
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::Paused),
                "Paused",
                Some(LIBRARY_ICON_PAUSED),
                Some(library_counts.paused),
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::Abandoned),
                "Abandoned",
                Some(LIBRARY_ICON_ABANDONED),
                Some(library_counts.abandoned),
            );

            // Match the line above Tasks/Settings — same separator
            // style so the three sections feel like peers.
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            if shelves_header(ui) {
                outcome = Outcome::OpenCreateShelfModal;
            }
            ui.add_space(2.0);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show_inside(ui, |ui| {
            // Floating scrollbar so the scrollbar overlays the content
            // instead of reserving horizontal space — without this, when
            // the shelf list overflows the bar shaves ~10px off the right
            // edge, pushing shelf count badges left of the Library badges
            // (which live in a panel with no scrollbar) and breaking the
            // shared right-edge column.
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::floating();
            egui::ScrollArea::vertical()
                .id_salt("sidebar-scroll")
                .show(ui, |ui| {
                    if shelves.is_empty() {
                        ui.label(RichText::new("(none yet)").italics().weak());
                    } else {
                        for shelf in shelves {
                            let count = shelf_counts.get(&shelf.id).copied().unwrap_or(0);
                            let resp = view_row(
                                ui,
                                current_view,
                                View::Shelf(shelf.id),
                                &shelf.name,
                                None,
                                Some(count),
                            );

                            // Drop target: highlight while a row is hovering
                            // with a payload, commit the drop on release.
                            // Inset to match the row highlight so the
                            // green outline doesn't get clipped at the
                            // panel edges.
                            if resp.dnd_hover_payload::<Vec<u64>>().is_some() {
                                let inner = resp.rect.shrink2(egui::vec2(INNER_MARGIN_X, 0.0));
                                ui.painter().rect_stroke(
                                    inner,
                                    4.0,
                                    Stroke::new(2.0, Color32::from_rgb(120, 200, 120)),
                                );
                            }
                            if let Some(payload) = resp.dnd_release_payload::<Vec<u64>>() {
                                outcome = Outcome::DropOnShelf {
                                    shelf_id: shelf.id,
                                    fic_ids: (*payload).clone(),
                                };
                            }

                            resp.context_menu(|ui| {
                                if ui.button("Delete shelf").clicked() {
                                    outcome = Outcome::OpenDeleteShelfConfirm(shelf.id);
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                });
        });

    outcome
}

// BMP-only Unicode glyphs that render in egui's default font stack (Noto
// Emoji is bundled, so symbols outside Ubuntu-Light fall back cleanly).
// Phase 12 is the natural place to swap these for proper SVG icons that
// match the Art Nouveau chrome.
const LIBRARY_ICON_ALL: &str = "\u{25C6}"; // ◆ filled diamond
const LIBRARY_ICON_IN_PROGRESS: &str = "\u{25B6}"; // ▶
const LIBRARY_ICON_READ: &str = "\u{2713}"; // ✓
const LIBRARY_ICON_PLAN: &str = "\u{25CB}"; // ○
const LIBRARY_ICON_PAUSED: &str = "\u{23F8}"; // ⏸
const LIBRARY_ICON_ABANDONED: &str = "\u{2717}"; // ✗

/// Horizontal breathing room between visual content (highlight rounding,
/// badges, the SHELVES "+" button) and the panel edges. Without it the
/// rounded corners on the row highlight get clipped by the parent UI's
/// clip rect, and the right-edge column doesn't have a stable reference
/// point that the `+` button and count badges can both align to.
const INNER_MARGIN_X: f32 = 4.0;
/// Right padding from the inner edge to the trailing element (badge or
/// `+` button). Both use this same offset so they line up vertically.
const RIGHT_GAP: f32 = 8.0;

fn section_label(ui: &mut Ui, text: &str) {
    // Match view_row's label x: INNER_MARGIN_X for the row inset, plus
    // the 8px gutter we use inside inner_rect. Without this, the
    // section label sat 12px to the left of the row labels below it.
    ui.horizontal(|ui| {
        ui.add_space(INNER_MARGIN_X + 8.0);
        ui.label(RichText::new(text).weak().size(11.0));
    });
}

/// Sidebar row that's clickable across its full width (not just the text),
/// shows an optional left-side icon, and right-aligns an optional count.
/// Returns the underlying response so callers can attach context menus
/// (used for shelf right-click → delete) and dnd payload checks.
///
/// The icon column is reserved unconditionally so labels in the same
/// section line up vertically even when some rows have no icon (currently
/// only the Shelves rows in Library/Shelves; Tasks/Settings live in a
/// separate panel where alignment doesn't matter).
fn view_row(
    ui: &mut Ui,
    current_view: &mut View,
    target: View,
    label: &str,
    icon: Option<&str>,
    count: Option<usize>,
) -> egui::Response {
    let selected = *current_view == target;
    // 22px gives the rows breathing room without making the sidebar feel
    // sparse. (Default interact_size.y is ~18.)
    let row_h = 22.0;
    let avail = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail, row_h), egui::Sense::click());
    // The clickable rect spans the full row so the user can hit the row
    // anywhere, but we paint inside `inner_rect` so the highlight's
    // rounded corners have room to render and so trailing elements share
    // a stable right edge with the SHELVES "+" button.
    let inner_rect = rect.shrink2(egui::vec2(INNER_MARGIN_X, 0.0));

    // Highlight on hover or when this row matches the active view. The
    // `interact_selectable` palette mirrors what `selectable_label` paints,
    // so this looks the same as the previous text-only selection target.
    let visuals = ui.style().interact_selectable(&resp, selected);
    if selected || resp.hovered() {
        ui.painter()
            .rect(inner_rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke);
    }

    let text_color = visuals.text_color();
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    // Slightly smaller than the label so the count reads as secondary
    // info, but uses the row's full text colour (not weak) so it stays
    // legible — and pops on the selected row, where text_color is the
    // selection palette's high-contrast colour.
    let count_font = egui::FontId::proportional(12.0);
    let cy = rect.center().y;

    // Right side: count badge (if any). Pre-laid-out here so we know its
    // width before clipping the label, then painted *after* the label so
    // it always sits on top.
    let badge = count.map(|n| {
        let galley = ui
            .painter()
            .layout_no_wrap(n.to_string(), count_font, text_color);
        let pad = egui::vec2(5.0, 1.0);
        let size = galley.size() + pad * 2.0;
        let min = egui::pos2(inner_rect.right() - RIGHT_GAP - size.x, cy - size.y / 2.0);
        (egui::Rect::from_min_size(min, size), galley, pad)
    });
    let count_reserve = match &badge {
        Some((r, _, _)) => inner_rect.right() - r.left() + 4.0,
        None => 4.0,
    };

    // Left side: icon column reserved only when there *is* an icon. Rows
    // without one (Shelves, Tasks, Settings) sit flush-left with no
    // phantom indent.
    let left_pad = 8.0;
    let icon_col_w = if icon.is_some() { 20.0 } else { 0.0 };
    if let Some(icon) = icon {
        ui.painter().text(
            egui::pos2(inner_rect.left() + left_pad + icon_col_w / 2.0, cy),
            egui::Align2::CENTER_CENTER,
            icon,
            body_font.clone(),
            text_color,
        );
    }
    let label_x = inner_rect.left() + left_pad + icon_col_w;
    let label_clip = egui::Rect::from_min_max(
        egui::pos2(label_x, inner_rect.top()),
        egui::pos2(inner_rect.right() - count_reserve, inner_rect.bottom()),
    );
    ui.painter().with_clip_rect(label_clip).text(
        egui::pos2(label_x, cy),
        egui::Align2::LEFT_CENTER,
        label,
        body_font,
        text_color,
    );

    // Paint the count badge on top of (the clip-stopped) label.
    if let Some((badge_rect, galley, pad)) = badge {
        let inactive = &ui.style().visuals.widgets.inactive;
        ui.painter()
            .rect(badge_rect, 4.0, inactive.weak_bg_fill, inactive.bg_stroke);
        ui.painter()
            .galley(badge_rect.min + pad, galley, text_color);
    }

    if resp.clicked() && !selected {
        *current_view = target;
    }

    resp
}

/// Manually-laid-out SHELVES header so the "+" button's right edge sits
/// exactly where the count badges below sit (same `RIGHT_GAP` from the
/// inner edge), instead of at egui's default layout-gutter offset.
/// Returns true if the user clicked the button.
fn shelves_header(ui: &mut Ui) -> bool {
    let row_h = 18.0;
    let avail = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(avail, row_h), egui::Sense::hover());
    let inner = rect.shrink2(egui::vec2(INNER_MARGIN_X, 0.0));
    let cy = inner.center().y;

    // Section label on the left, mirroring `section_label`'s styling.
    ui.painter().text(
        egui::pos2(inner.left() + 8.0, cy),
        egui::Align2::LEFT_CENTER,
        "SHELVES",
        egui::FontId::proportional(11.0),
        ui.style().visuals.weak_text_color(),
    );

    // "+" button on the right, same offset as the count badges. Painted
    // by hand so the glyph sits dead-centre — egui's `small_button`
    // puts the bounding-box centre, not the glyph's optical centre.
    let btn_size = egui::vec2(16.0, 16.0);
    let btn_rect = egui::Rect::from_min_size(
        egui::pos2(
            inner.right() - RIGHT_GAP - btn_size.x,
            cy - btn_size.y / 2.0,
        ),
        btn_size,
    );
    let resp = ui
        .interact(btn_rect, ui.id().with("shelves-add"), egui::Sense::click())
        .on_hover_text("New shelf");
    let visuals = ui.style().interact(&resp);
    ui.painter()
        .rect(btn_rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke);
    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(12.0),
        visuals.text_color(),
    );
    resp.clicked()
}
