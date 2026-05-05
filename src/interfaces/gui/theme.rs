use egui::{Color32, Context, FontData, FontDefinitions, FontFamily};

use super::assets;

/// Must match the SVG fill in `assets/frame/art_nouveau.svg` — recolour
/// the SVG and update this constant together. Used for the FICFLOW
/// wordmark and view-title heading so decorative type reads in the
/// same family as the frame border.
pub const ACCENT: Color32 = Color32::from_rgb(0xC9, 0xC4, 0xBC);

pub const NEUE_FAMILY: &str = "neue";
pub const COMFORTAA_FAMILY: &str = "comfortaa";

pub fn install(ctx: &Context) {
    install_fonts(ctx);
}

fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        NEUE_FAMILY.to_owned(),
        FontData::from_static(assets::NEUE_FONT),
    );
    fonts.font_data.insert(
        COMFORTAA_FAMILY.to_owned(),
        FontData::from_static(assets::COMFORTAA_FONT),
    );

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, COMFORTAA_FAMILY.to_owned());
    fonts.families.insert(
        FontFamily::Name(NEUE_FAMILY.into()),
        vec![NEUE_FAMILY.to_owned()],
    );

    // Fallbacks for glyphs Comfortaa doesn't carry (sidebar icons
    // ⏸ ◆ ▶ ✓ ○ ✗, CJK ideographs).
    append_system_fallbacks(&mut fonts);

    ctx.set_fonts(fonts);
}

/// Probed in priority order. egui falls through the family list
/// glyph by glyph, so each entry only covers what earlier ones miss.
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

fn append_system_fallbacks(fonts: &mut FontDefinitions) {
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
    }
}
