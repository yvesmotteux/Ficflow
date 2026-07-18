use std::collections::HashMap;

use egui::{Color32, RichText, Stroke, StrokeKind, Ui};

use crate::domain::fanfiction::ReadingStatus;
use crate::domain::shelf::{Shelf, ShelfKind};

use super::super::theme;
use super::super::view::View;

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
    /// Mutated in-place by row clicks; caller diffs against `prev_view`.
    pub current_view: &'a mut View,
    pub shelves: &'a [Shelf],
    pub library_counts: &'a LibraryCounts,
    /// Missing shelf ids render as 0.
    pub shelf_counts: &'a HashMap<u64, usize>,
    pub running_tasks: usize,
}

/// At most one variant fires per frame — the three are mutually-exclusive
/// mouse interactions.
pub enum Outcome {
    None,
    OpenCreateShelfModal,
    OpenRenameShelfModal(u64),
    OpenEditAutoShelfModal(u64),
    OpenDeleteShelfConfirm(u64),
    DropOnShelf {
        shelf_id: u64,
        fic_ids: Vec<u64>,
    },
    MoveShelf {
        shelf_id: u64,
        new_parent: Option<u64>,
    },
    TogglePinShelf(u64),
}

/// Dnd payload for dragging a shelf row — distinct from the `Vec<u64>`
/// fic payload and the table's `ColumnKey` payload.
#[derive(Clone, Copy)]
pub struct ShelfDrag(pub u64);

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
    egui::Panel::bottom("ficflow-sidebar-bottom")
        .resizable(false)
        .show_separator_line(true)
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            ui.add_space(6.0);
            view_row(
                ui,
                current_view,
                View::Tasks,
                "Tasks",
                None,
                Some(running_tasks),
                None,
            );
            view_row(
                ui,
                current_view,
                View::Settings,
                "Settings",
                None,
                None,
                None,
            );
            ui.add_space(6.0);
        });

    // Pin LIBRARY section + the SHELVES header to the top so the user
    // never loses access to status filters or the "+ shelf" button when
    // a long shelf list scrolls. Only the shelf rows themselves go in
    // the scrollable Central area below.
    egui::Panel::top("ficflow-sidebar-top")
        .resizable(false)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
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
                None,
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::InProgress),
                "In Progress",
                Some(LIBRARY_ICON_IN_PROGRESS),
                Some(library_counts.in_progress),
                None,
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::Read),
                "Read",
                Some(LIBRARY_ICON_READ),
                Some(library_counts.read),
                None,
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::PlanToRead),
                "Plan to Read",
                Some(LIBRARY_ICON_PLAN),
                Some(library_counts.plan_to_read),
                None,
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::Paused),
                "Paused",
                Some(LIBRARY_ICON_PAUSED),
                Some(library_counts.paused),
                None,
            );
            view_row(
                ui,
                current_view,
                View::ByStatus(ReadingStatus::Abandoned),
                "Abandoned",
                Some(LIBRARY_ICON_ABANDONED),
                Some(library_counts.abandoned),
                None,
            );

            // Match the line above Tasks/Settings — same separator
            // style so the three sections feel like peers.
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            match shelves_header(ui) {
                HeaderOutcome::None => {}
                HeaderOutcome::AddClicked => {
                    outcome = Outcome::OpenCreateShelfModal;
                }
                HeaderOutcome::ShelfDropped(shelf_id) => {
                    outcome = Outcome::MoveShelf {
                        shelf_id,
                        new_parent: None,
                    };
                }
            }
            ui.add_space(2.0);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            ui.style_mut().spacing.scroll = egui::style::ScrollStyle::floating();
            egui::ScrollArea::vertical()
                .id_salt("sidebar-scroll")
                .show(ui, |ui| {
                    if shelves.is_empty() {
                        ui.label(RichText::new("(none yet)").italics().weak());
                    } else {
                        let mut children: HashMap<Option<u64>, Vec<&Shelf>> = HashMap::new();
                        for shelf in shelves {
                            children
                                .entry(shelf.parent_shelf_id)
                                .or_default()
                                .push(shelf);
                        }
                        shelf_rows(
                            ui,
                            current_view,
                            &children,
                            shelf_counts,
                            None,
                            0,
                            &mut outcome,
                        );
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

const INDENT_STEP: f32 = 14.0;
const TRIANGLE_COL_W: f32 = 14.0;

#[derive(Clone, Copy)]
struct TreeRow {
    depth: usize,
    expanded: Option<bool>,
    pinned: bool,
}

fn shelf_rows(
    ui: &mut Ui,
    current_view: &mut View,
    children: &HashMap<Option<u64>, Vec<&Shelf>>,
    shelf_counts: &HashMap<u64, usize>,
    parent: Option<u64>,
    depth: usize,
    outcome: &mut Outcome,
) {
    let Some(siblings) = children.get(&parent) else {
        return;
    };
    for shelf in siblings {
        let has_children = children.contains_key(&Some(shelf.id));
        let collapse_id = egui::Id::new(("ficflow-shelf-collapsed", shelf.id));
        let collapsed = ui.data_mut(|d| d.get_persisted(collapse_id).unwrap_or(false));
        let count = shelf_counts.get(&shelf.id).copied().unwrap_or(0);
        let is_auto = matches!(shelf.kind, ShelfKind::Auto(_));
        let (resp, triangle_clicked, pin_clicked) = view_row(
            ui,
            current_view,
            View::Shelf(shelf.id),
            &shelf.name,
            // U+FE0E forces the plain "text" glyph variant rather than a
            // wider emoji-style rendering, which otherwise bakes in extra
            // left padding that throws off alignment vs. other icons.
            is_auto.then_some("\u{2699}\u{FE0E}"),
            Some(count),
            Some(TreeRow {
                depth,
                expanded: has_children.then_some(!collapsed),
                pinned: shelf.pinned,
            }),
        );
        if triangle_clicked {
            ui.data_mut(|d| d.insert_persisted(collapse_id, !collapsed));
        }
        if pin_clicked {
            *outcome = Outcome::TogglePinShelf(shelf.id);
        }

        if resp.drag_started() {
            resp.dnd_set_drag_payload(ShelfDrag(shelf.id));
        }

        // `dnd_release_payload::<T>()` takes the payload out of egui's shared
        // drag-and-drop slot *unconditionally*, then tries to downcast it —
        // even a failed downcast still clears the slot. So we must only ever
        // call it for the payload type that's actually being carried (checked
        // non-destructively via `dnd_hover_payload` first); calling it for
        // both `Vec<u64>` and `ShelfDrag` unconditionally on the same row, in
        // sequence, silently destroyed real `ShelfDrag` drops before the
        // second check ever saw them — nesting a shelf never worked.
        if !is_auto && resp.dnd_hover_payload::<Vec<u64>>().is_some() {
            let inner = resp.rect.shrink2(egui::vec2(INNER_MARGIN_X, 0.0));
            ui.painter().rect_stroke(
                inner,
                4.0,
                Stroke::new(2.0_f32, Color32::from_rgb(120, 200, 120)),
                StrokeKind::Inside,
            );
            if let Some(payload) = resp.dnd_release_payload::<Vec<u64>>() {
                *outcome = Outcome::DropOnShelf {
                    shelf_id: shelf.id,
                    fic_ids: (*payload).clone(),
                };
            }
        } else if !is_auto
            && resp
                .dnd_hover_payload::<ShelfDrag>()
                .is_some_and(|dragged| dragged.0 != shelf.id)
        {
            let inner = resp.rect.shrink2(egui::vec2(INNER_MARGIN_X, 0.0));
            ui.painter().rect_stroke(
                inner,
                4.0,
                Stroke::new(2.0_f32, Color32::from_rgb(120, 160, 220)),
                StrokeKind::Inside,
            );
            if let Some(dragged) = resp.dnd_release_payload::<ShelfDrag>()
                && dragged.0 != shelf.id
            {
                *outcome = Outcome::MoveShelf {
                    shelf_id: dragged.0,
                    new_parent: Some(shelf.id),
                };
            }
        }

        resp.context_menu(|ui| {
            if ui.button("Rename shelf").clicked() {
                *outcome = Outcome::OpenRenameShelfModal(shelf.id);
                ui.close();
            }
            if is_auto && ui.button("Edit criteria").clicked() {
                *outcome = Outcome::OpenEditAutoShelfModal(shelf.id);
                ui.close();
            }
            let pin_label = if shelf.pinned {
                "Unpin shelf"
            } else {
                "Pin shelf"
            };
            if ui.button(pin_label).clicked() {
                *outcome = Outcome::TogglePinShelf(shelf.id);
                ui.close();
            }
            if ui.button("Delete shelf").clicked() {
                *outcome = Outcome::OpenDeleteShelfConfirm(shelf.id);
                ui.close();
            }
        });

        if has_children && !collapsed {
            shelf_rows(
                ui,
                current_view,
                children,
                shelf_counts,
                Some(shelf.id),
                depth + 1,
                outcome,
            );
        }
    }
}

fn section_label(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(INNER_MARGIN_X + 8.0);
        ui.label(RichText::new(text).weak().size(11.0));
    });
}

fn view_row(
    ui: &mut Ui,
    current_view: &mut View,
    target: View,
    label: &str,
    icon: Option<&str>,
    count: Option<usize>,
    tree: Option<TreeRow>,
) -> (egui::Response, bool, bool) {
    let selected = *current_view == target;
    // 22px gives the rows breathing room without making the sidebar feel
    // sparse. (Default interact_size.y is ~18.)
    let row_h = 22.0;
    let avail = ui.available_width();
    // Tree rows (shelves) are also drag sources for nesting; egui only
    // reports `drag_started` on click_and_drag senses.
    let sense = if tree.is_some() {
        egui::Sense::click_and_drag()
    } else {
        egui::Sense::click()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail, row_h), sense);
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
        ui.painter().rect(
            inner_rect,
            4.0,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
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

    // Pin toggle: only shelf rows (`tree.is_some()`) get one, positioned
    // just left of the count badge so it doesn't shift when the count
    // changes width.
    const PIN_COL_W: f32 = 16.0;
    const PIN_GAP: f32 = 4.0;
    let pin_rect = tree.map(|_| {
        let size = egui::vec2(PIN_COL_W, PIN_COL_W);
        let x = inner_rect.right() - count_reserve - PIN_GAP - size.x;
        egui::Rect::from_min_size(egui::pos2(x, cy - size.y / 2.0), size)
    });
    let total_reserve = match pin_rect {
        Some(_) => count_reserve + PIN_GAP + PIN_COL_W,
        None => count_reserve,
    };

    // Left side: icon column reserved only when there *is* an icon. Rows
    // without one (Shelves, Tasks, Settings) sit flush-left with no
    // phantom indent. Tree rows (shelves) get a depth indent plus a
    // disclosure-triangle column instead, reserved for leaves too so
    // sibling labels line up.
    let left_pad = 8.0;
    let (indent, tree_col_w) = match tree {
        Some(t) => (t.depth as f32 * INDENT_STEP, TRIANGLE_COL_W),
        None => (0.0, 0.0),
    };
    let icon_col_w = if icon.is_some() { 20.0 } else { 0.0 };
    if let Some(icon) = icon {
        ui.painter().text(
            egui::pos2(
                inner_rect.left() + left_pad + indent + tree_col_w + icon_col_w / 2.0,
                cy,
            ),
            egui::Align2::CENTER_CENTER,
            icon,
            body_font.clone(),
            text_color,
        );
    }
    let mut triangle_clicked = false;
    if let Some(TreeRow {
        expanded: Some(expanded),
        ..
    }) = tree
    {
        let tri_center = egui::pos2(
            inner_rect.left() + left_pad + indent + TRIANGLE_COL_W / 2.0,
            cy,
        );
        let tri_rect = egui::Rect::from_center_size(tri_center, egui::vec2(TRIANGLE_COL_W, row_h));
        let tri_resp = ui.interact(tri_rect, resp.id.with("disclosure"), egui::Sense::click());
        triangle_clicked = tri_resp.clicked();
        let glyph = if expanded { "\u{25BE}" } else { "\u{25B8}" };
        ui.painter().text(
            tri_center,
            egui::Align2::CENTER_CENTER,
            glyph,
            egui::FontId::proportional(10.0),
            ui.style().interact(&tri_resp).text_color(),
        );
    }
    let label_x = inner_rect.left() + left_pad + indent + tree_col_w + icon_col_w;
    let label_clip = egui::Rect::from_min_max(
        egui::pos2(label_x, inner_rect.top()),
        egui::pos2(inner_rect.right() - total_reserve, inner_rect.bottom()),
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
        ui.painter().rect(
            badge_rect,
            4.0,
            inactive.weak_bg_fill,
            inactive.bg_stroke,
            StrokeKind::Inside,
        );
        ui.painter()
            .galley(badge_rect.min + pad, galley, text_color);
    }

    // Pin toggle icon, painted on top of the label like the count badge.
    // Only shown for pinned shelves, or on hover so unpinned shelves don't
    // clutter the row — `theme::ACCENT` rather than the selection blue so
    // it stays visible when the row itself is selected/highlighted.
    let mut pin_clicked = false;
    if let (Some(pin_rect), Some(TreeRow { pinned, .. })) = (pin_rect, tree) {
        let pin_resp = ui.interact(pin_rect, resp.id.with("pin"), egui::Sense::click());
        pin_clicked = pin_resp.clicked();
        if pinned || resp.hovered() || pin_resp.hovered() {
            let color = if pinned {
                theme::ACCENT
            } else {
                ui.style().visuals.weak_text_color()
            };
            paint_pin_icon(ui.painter(), pin_rect.center(), color);
        }
        pin_resp.on_hover_text(if pinned { "Unpin shelf" } else { "Pin shelf" });
    }

    if resp.clicked() && !triangle_clicked && !pin_clicked && !selected {
        *current_view = target;
    }

    (resp, triangle_clicked, pin_clicked)
}

/// A small map-pin glyph drawn from primitives (circular head, triangular
/// point) rather than a font character — the bundled/fallback fonts don't
/// carry a pin symbol, only a flag one, which reads as the wrong icon.
fn paint_pin_icon(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let head_center = center - egui::vec2(0.0, 2.0);
    painter.circle_filled(head_center, 3.5, color);
    let tip = center + egui::vec2(0.0, 5.0);
    let base_l = head_center + egui::vec2(-3.0, 1.0);
    let base_r = head_center + egui::vec2(3.0, 1.0);
    painter.add(egui::Shape::convex_polygon(
        vec![base_l, base_r, tip],
        color,
        Stroke::NONE,
    ));
}

enum HeaderOutcome {
    None,
    AddClicked,
    ShelfDropped(u64),
}

/// Manually-laid-out SHELVES header so the "+" button's right edge sits
/// exactly where the count badges below sit (same `RIGHT_GAP` from the
/// inner edge), instead of at egui's default layout-gutter offset.
/// Doubles as the drop target that moves a dragged shelf back to the
/// top level.
fn shelves_header(ui: &mut Ui) -> HeaderOutcome {
    let row_h = 18.0;
    let avail = ui.available_width();
    let (rect, header_resp) =
        ui.allocate_exact_size(egui::vec2(avail, row_h), egui::Sense::hover());
    let inner = rect.shrink2(egui::vec2(INNER_MARGIN_X, 0.0));
    let cy = inner.center().y;

    let mut outcome = HeaderOutcome::None;
    if header_resp.dnd_hover_payload::<ShelfDrag>().is_some() {
        ui.painter().rect_stroke(
            inner,
            4.0,
            Stroke::new(2.0_f32, Color32::from_rgb(120, 160, 220)),
            StrokeKind::Inside,
        );
    }
    if let Some(dragged) = header_resp.dnd_release_payload::<ShelfDrag>() {
        outcome = HeaderOutcome::ShelfDropped(dragged.0);
    }

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
    ui.painter().rect(
        btn_rect,
        4.0,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(12.0),
        visuals.text_color(),
    );
    if resp.clicked() {
        outcome = HeaderOutcome::AddClicked;
    }
    outcome
}
