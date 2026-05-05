//! Compiled-in binary blobs for Phase 12 chrome. Bundled via
//! `include_bytes!` so the resulting binary is self-contained — no
//! `~/.local/share/ficflow/fonts/` to ship, no FreeType dance, just a
//! single `ficflow` executable that runs anywhere.

pub const NEUE_FONT: &[u8] = include_bytes!("../../../assets/fonts/Neue_mod.ttf");
pub const COMFORTAA_FONT: &[u8] =
    include_bytes!("../../../assets/fonts/Comfortaa-VariableFont_wght.ttf");
pub const FRAME_SVG: &[u8] = include_bytes!("../../../assets/frame/art_nouveau.svg");
/// Window icon (taskbar / window-list / about dialog). PNG decoded at
/// startup via `eframe::icon_data::from_png_bytes` and handed to
/// `ViewportBuilder::with_icon`.
pub const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");
