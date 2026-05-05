//! Best-effort system-font fallback so glyphs that egui's bundled fonts
//! (Ubuntu-Light + a NotoEmoji subset) don't cover render cleanly instead
//! of as tofu boxes. We don't bring in a font crate or bundle assets here
//! (that's Phase 12 territory); we just probe a handful of well-known
//! paths and append every one we find, so multiple gaps can be filled at
//! once — DejaVu Sans for BMP basics, Noto Symbols 2 for ⏸ etc., Noto
//! Sans CJK / Microsoft YaHei / PingFang for Han ideographs.

use egui::{FontData, FontDefinitions, FontFamily};

/// Probed in priority order. egui falls through the family list glyph by
/// glyph, so each entry only covers what the earlier ones miss. Entries
/// span Linux, macOS and Windows so a user opening the binary on any of
/// the three desktop targets gets the same coverage when the relevant
/// system font is installed.
const FALLBACK_FONT_PATHS: &[&str] = &[
    // Linux — broad BMP (Latin/Cyrillic/Greek + Geometric Shapes block).
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    // Linux — Miscellaneous Technical block (⏸ etc.).
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
    // Linux — CJK ideographs (Chinese, Japanese, Korean).
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    // macOS — broad coverage and CJK.
    "/Library/Fonts/Arial Unicode.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/STHeiti Light.ttc",
    // Windows — symbols and CJK.
    "C:/Windows/Fonts/seguisym.ttf",
    "C:/Windows/Fonts/segoeui.ttf",
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/msyhbd.ttc",
    "C:/Windows/Fonts/simsun.ttc",
    "C:/Windows/Fonts/simhei.ttf",
];

pub fn install_system_fallback(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut added = 0usize;

    for (i, path) in FALLBACK_FONT_PATHS.iter().enumerate() {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let name = format!("ficflow_fb_{}", i);
        fonts
            .font_data
            .insert(name.clone(), FontData::from_owned(bytes));
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
        added += 1;
    }

    if added > 0 {
        ctx.set_fonts(fonts);
    }
}
