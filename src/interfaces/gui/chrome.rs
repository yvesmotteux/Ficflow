//! Phase 12 Art Nouveau window chrome. 9-slice paint of an SVG atlas
//! onto the window edges + custom min/max/close buttons + drag-region
//! and resize-edge interaction (since `with_decorations(false)`
//! removes the OS title bar).

use egui::{
    Color32, Context, CursorIcon, Id, LayerId, Pos2, Rect, ResizeDirection, Sense, Stroke, Ui,
    Vec2, ViewportCommand,
};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

use super::{assets, theme};

/// Logical pixel size of the 9-slice corner tiles. Small and subtle — the
/// corners are a decorative accent, not a dominant border.
const CORNER_W: f32 = 32.0;
const CORNER_H: f32 = 32.0;

/// Fraction of the source SVG occupied by each corner tile. The NW curl's
/// decorative swirl extends rightward to about x_ratio ≈ 0.30 of the atlas
/// (measured), so `CW_RATIO` must be larger than that — otherwise the swirl
/// tip lands in the N-edge region and gets horizontally stretched along
/// with the straight line when the window widens.
const CW_RATIO: f32 = 0.32;
const CH_RATIO: f32 = 0.18;

/// Window-coordinate y/x where the innermost top/left line of the frame ends
/// (below/right of this value is interior). With the widened `CW_RATIO=0.32`,
/// the W tile is `0.32 × 2048 ≈ 655` atlas px wide; innermost left line at
/// atlas x=280 maps to tile ratio 0.427 × CORNER_W=32 ≈ 14. TOP_INNER_Y
/// stays the same because `CH_RATIO` didn't change.
const TOP_INNER_Y: f32 = 15.0;
const LEFT_INNER_X: f32 = 14.0;

/// Window-coordinate y/x of the outermost top/left line centerline — where
/// resize handles sit so the user grabs the visible frame edge.
const TOP_OUTER_Y: f32 = 11.0;
const LEFT_OUTER_X: f32 = 11.0;

/// Padding between the innermost frame line and the start of content.
const CONTENT_PADDING: f32 = 10.0;

/// Inset between the innermost frame line and the content background fill.
const BG_INSET: f32 = 2.0;

/// Hit-test thicknesses for resize handles on the outer frame lines.
const RESIZE_STRIP: f32 = 14.0;
const RESIZE_CORNER: f32 = 22.0;

const CONTROL_BUTTON_SIZE: Vec2 = Vec2::new(20.0, 16.0);
const CONTROL_BUTTON_GAP: f32 = 4.0;
const CONTROLS_TOP_OFFSET: f32 = 2.0;

/// Atlas render size. High resolution so the horizontal edges have enough
/// source pixels to downsample cleanly to any reasonable window width on
/// high-DPI displays (no visible pixelation).
const ATLAS_SIZE_PX: [u32; 2] = [4096, 5462];

pub struct FrameChrome {
    tree: usvg::Tree,
    atlas: Option<egui::TextureHandle>,
}

impl FrameChrome {
    pub fn new() -> Result<Self, usvg::Error> {
        let tree = usvg::Tree::from_data(assets::FRAME_SVG, &usvg::Options::default())?;
        Ok(Self { tree, atlas: None })
    }

    pub fn content_rect(&self, screen: Rect) -> Rect {
        Rect::from_min_max(
            Pos2::new(
                screen.left() + LEFT_INNER_X + CONTENT_PADDING,
                screen.top() + TOP_INNER_Y + CONTENT_PADDING,
            ),
            Pos2::new(
                screen.right() - LEFT_INNER_X - CONTENT_PADDING,
                screen.bottom() - TOP_INNER_Y - CONTENT_PADDING,
            ),
        )
    }

    pub fn paint_background(&mut self, ctx: &Context, screen: Rect) {
        let painter = ctx.layer_painter(LayerId::background());

        // Read `window_fill` live so the bg fill matches whatever
        // colour the nested panels paint with — otherwise an 8px
        // strip of mismatched colour shows between SVG and panel
        // edges. Square corners for the same reason (rounded inner
        // corner vs square panel edge produces a visible seam).
        let bg_rect = Rect::from_min_max(
            Pos2::new(
                screen.left() + LEFT_INNER_X + BG_INSET,
                screen.top() + TOP_INNER_Y + BG_INSET,
            ),
            Pos2::new(
                screen.right() - LEFT_INNER_X - BG_INSET,
                screen.bottom() - TOP_INNER_Y - BG_INSET,
            ),
        );
        let bg_fill = ctx.style().visuals.window_fill;
        painter.rect_filled(bg_rect, 0.0, bg_fill);

        if self.atlas.is_none() {
            self.atlas = rasterize_atlas(&self.tree, ctx);
        }
        let Some(texture) = &self.atlas else {
            return;
        };

        // 9-slice: four corners fixed, four edges stretch between them.
        let left = screen.left();
        let right = screen.right();
        let top = screen.top();
        let bottom = screen.bottom();
        let inner_l = left + CORNER_W;
        let inner_r = right - CORNER_W;
        let inner_t = top + CORNER_H;
        let inner_b = bottom - CORNER_H;

        let tiles = [
            (
                Rect::from_min_max(Pos2::new(left, top), Pos2::new(inner_l, inner_t)),
                uv(0.0, 0.0, CW_RATIO, CH_RATIO),
            ),
            (
                Rect::from_min_max(Pos2::new(inner_r, top), Pos2::new(right, inner_t)),
                uv(1.0 - CW_RATIO, 0.0, 1.0, CH_RATIO),
            ),
            (
                Rect::from_min_max(Pos2::new(left, inner_b), Pos2::new(inner_l, bottom)),
                uv(0.0, 1.0 - CH_RATIO, CW_RATIO, 1.0),
            ),
            (
                Rect::from_min_max(Pos2::new(inner_r, inner_b), Pos2::new(right, bottom)),
                uv(1.0 - CW_RATIO, 1.0 - CH_RATIO, 1.0, 1.0),
            ),
            (
                Rect::from_min_max(Pos2::new(inner_l, top), Pos2::new(inner_r, inner_t)),
                uv(CW_RATIO, 0.0, 1.0 - CW_RATIO, CH_RATIO),
            ),
            (
                Rect::from_min_max(Pos2::new(inner_l, inner_b), Pos2::new(inner_r, bottom)),
                uv(CW_RATIO, 1.0 - CH_RATIO, 1.0 - CW_RATIO, 1.0),
            ),
            (
                Rect::from_min_max(Pos2::new(left, inner_t), Pos2::new(inner_l, inner_b)),
                uv(0.0, CH_RATIO, CW_RATIO, 1.0 - CH_RATIO),
            ),
            (
                Rect::from_min_max(Pos2::new(inner_r, inner_t), Pos2::new(right, inner_b)),
                uv(1.0 - CW_RATIO, CH_RATIO, 1.0, 1.0 - CH_RATIO),
            ),
        ];

        for (dest, src) in tiles {
            if dest.width() > 0.0 && dest.height() > 0.0 {
                painter.image(texture.id(), dest, src, Color32::WHITE);
            }
        }
    }

    pub fn handle_interactions(&self, ui: &mut Ui, screen: Rect, controls_rect: Rect) {
        self.handle_title_drag(ui, screen, controls_rect);
        if !is_maximized(ui.ctx()) {
            self.handle_resize_edges(ui, screen);
        }
        self.apply_cursor_hints(ui.ctx(), screen, controls_rect);
    }

    fn apply_cursor_hints(&self, ctx: &Context, screen: Rect, controls_rect: Rect) {
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !is_maximized(ctx) {
            for (rect, cursor) in self.resize_hit_regions(screen) {
                if rect.contains(pos) {
                    ctx.set_cursor_icon(cursor);
                    return;
                }
            }
        }
        let drag_rect = self.title_drag_rect(screen, controls_rect);
        if drag_rect.contains(pos) {
            ctx.set_cursor_icon(CursorIcon::Grab);
        }
    }

    fn title_drag_rect(&self, screen: Rect, controls_rect: Rect) -> Rect {
        let left = screen.left() + CORNER_W + CONTENT_PADDING;
        let right = controls_rect.left() - CONTROL_BUTTON_GAP;
        let top = screen.top() + TOP_INNER_Y + CONTENT_PADDING;
        let bottom = top + CONTROL_BUTTON_SIZE.y + 2.0 * CONTROLS_TOP_OFFSET;
        if right <= left {
            return Rect::NOTHING;
        }
        Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom))
    }

    fn resize_hit_regions(&self, screen: Rect) -> Vec<(Rect, CursorIcon)> {
        let half = RESIZE_STRIP / 2.0;
        let n_y = screen.top() + TOP_OUTER_Y;
        let s_y = screen.bottom() - TOP_OUTER_Y;
        let w_x = screen.left() + LEFT_OUTER_X;
        let e_x = screen.right() - LEFT_OUTER_X;

        vec![
            (
                Rect::from_min_size(screen.min, Vec2::splat(RESIZE_CORNER)),
                CursorIcon::ResizeNwSe,
            ),
            (
                Rect::from_min_size(
                    Pos2::new(screen.right() - RESIZE_CORNER, screen.top()),
                    Vec2::splat(RESIZE_CORNER),
                ),
                CursorIcon::ResizeNeSw,
            ),
            (
                Rect::from_min_size(
                    Pos2::new(screen.left(), screen.bottom() - RESIZE_CORNER),
                    Vec2::splat(RESIZE_CORNER),
                ),
                CursorIcon::ResizeNeSw,
            ),
            (
                Rect::from_min_size(
                    screen.max - Vec2::splat(RESIZE_CORNER),
                    Vec2::splat(RESIZE_CORNER),
                ),
                CursorIcon::ResizeNwSe,
            ),
            (
                Rect::from_min_max(
                    Pos2::new(screen.left() + CORNER_W, n_y - half),
                    Pos2::new(screen.right() - CORNER_W, n_y + half),
                ),
                CursorIcon::ResizeNorth,
            ),
            (
                Rect::from_min_max(
                    Pos2::new(screen.left() + CORNER_W, s_y - half),
                    Pos2::new(screen.right() - CORNER_W, s_y + half),
                ),
                CursorIcon::ResizeSouth,
            ),
            (
                Rect::from_min_max(
                    Pos2::new(w_x - half, screen.top() + CORNER_H),
                    Pos2::new(w_x + half, screen.bottom() - CORNER_H),
                ),
                CursorIcon::ResizeWest,
            ),
            (
                Rect::from_min_max(
                    Pos2::new(e_x - half, screen.top() + CORNER_H),
                    Pos2::new(e_x + half, screen.bottom() - CORNER_H),
                ),
                CursorIcon::ResizeEast,
            ),
        ]
    }

    fn handle_title_drag(&self, ui: &mut Ui, screen: Rect, controls_rect: Rect) {
        let drag_rect = self.title_drag_rect(screen, controls_rect);
        if drag_rect == Rect::NOTHING {
            return;
        }
        // `click_and_drag` + `drag_started` (not `primary_pressed`)
        // so a double-click without movement registers as such — on
        // press alone the OS-level drag would steal the pointer
        // before egui sees the second click.
        let resp = ui.interact(
            drag_rect,
            Id::new("ficflow-title-drag"),
            Sense::click_and_drag(),
        );
        if resp.double_clicked() {
            let cmd = ViewportCommand::Maximized(!is_maximized(ui.ctx()));
            ui.ctx().send_viewport_cmd(cmd);
        } else if resp.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            // Clear egui's interaction latch — the OS is handling the
            // drag now, otherwise no other widget can claim hover until
            // the user releases.
            ui.ctx().stop_dragging();
        }
    }

    fn handle_resize_edges(&self, ui: &mut Ui, screen: Rect) {
        let directions = [
            ResizeDirection::NorthWest,
            ResizeDirection::NorthEast,
            ResizeDirection::SouthWest,
            ResizeDirection::SouthEast,
            ResizeDirection::North,
            ResizeDirection::South,
            ResizeDirection::West,
            ResizeDirection::East,
        ];
        let regions = self.resize_hit_regions(screen);
        for (i, ((rect, _cursor), dir)) in regions.iter().zip(directions).enumerate() {
            if rect.width() <= 0.0 || rect.height() <= 0.0 {
                continue;
            }
            let resp = ui.interact(
                *rect,
                Id::new(("ficflow-resize", dir as u8, i)),
                Sense::click(),
            );
            if resp.hovered() && ui.ctx().input(|i| i.pointer.primary_pressed()) {
                ui.ctx()
                    .send_viewport_cmd(ViewportCommand::BeginResize(dir));
                // See `handle_title_drag` — clear interaction latch so
                // hover recovers after OS-managed resize.
                ui.ctx().stop_dragging();
            }
        }
    }

    pub fn draw_window_controls(&self, ui: &mut Ui, screen: Rect) -> Rect {
        let content = self.content_rect(screen);
        let right = content.right();
        let top = content.top() + CONTROLS_TOP_OFFSET;
        let button_count = 3.0_f32;
        let total_width =
            button_count * CONTROL_BUTTON_SIZE.x + (button_count - 1.0) * CONTROL_BUTTON_GAP;
        let origin = Pos2::new(right - total_width, top);
        let bounding = Rect::from_min_size(origin, Vec2::new(total_width, CONTROL_BUTTON_SIZE.y));

        struct ControlButton {
            label: &'static str,
            action: fn(&Context),
        }
        let buttons = [
            ControlButton {
                label: "\u{2212}",
                action: |ctx| ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
            },
            ControlButton {
                label: "\u{25A1}",
                action: |ctx| {
                    let cmd = ViewportCommand::Maximized(!is_maximized(ctx));
                    ctx.send_viewport_cmd(cmd);
                },
            },
            ControlButton {
                label: "\u{00D7}",
                action: |ctx| ctx.send_viewport_cmd(ViewportCommand::Close),
            },
        ];

        // Foreground `Area` so the FICFLOW header `TopBottomPanel`
        // (which lays its own background across the top of the
        // content rect) doesn't paint over the buttons.
        egui::Area::new(Id::new("ficflow-window-controls"))
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .interactable(true)
            .show(ui.ctx(), |area_ui| {
                for (i, button) in buttons.iter().enumerate() {
                    let rect = Rect::from_min_size(
                        Pos2::new(
                            origin.x + i as f32 * (CONTROL_BUTTON_SIZE.x + CONTROL_BUTTON_GAP),
                            origin.y,
                        ),
                        CONTROL_BUTTON_SIZE,
                    );
                    let resp = area_ui.interact(rect, Id::new(("ficflow-ctrl", i)), Sense::click());
                    let hover = resp.hovered();
                    if hover {
                        area_ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let fill = if resp.is_pointer_button_down_on() {
                        Color32::from_rgba_unmultiplied(0xC9, 0xC4, 0xBC, 80)
                    } else if hover {
                        Color32::from_rgba_unmultiplied(0xC9, 0xC4, 0xBC, 40)
                    } else {
                        Color32::TRANSPARENT
                    };
                    let painter = area_ui.painter();
                    painter.rect(rect, 4.0, fill, Stroke::new(1.0, theme::ACCENT));
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        button.label,
                        egui::FontId::proportional(11.0),
                        theme::ACCENT,
                    );
                    if resp.clicked() {
                        (button.action)(area_ui.ctx());
                    }
                }
            });
        bounding
    }
}

fn uv(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
}

fn rasterize_atlas(tree: &usvg::Tree, ctx: &Context) -> Option<egui::TextureHandle> {
    let mut pixmap = Pixmap::new(ATLAS_SIZE_PX[0], ATLAS_SIZE_PX[1])?;
    let tree_size = tree.size();
    let scale = Transform::from_scale(
        ATLAS_SIZE_PX[0] as f32 / tree_size.width(),
        ATLAS_SIZE_PX[1] as f32 / tree_size.height(),
    );
    resvg::render(tree, scale, &mut pixmap.as_mut());
    let image = egui::ColorImage::from_rgba_premultiplied(
        [ATLAS_SIZE_PX[0] as usize, ATLAS_SIZE_PX[1] as usize],
        pixmap.data(),
    );
    Some(ctx.load_texture("ficflow-frame", image, egui::TextureOptions::LINEAR))
}

fn is_maximized(ctx: &Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}
