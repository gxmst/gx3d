use wgpu_text::glyph_brush::ab_glyph::FontArc;

pub(super) fn load_ui_font() -> Option<FontArc> {
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\Noto Sans SC (TrueType).otf",
        "C:\\Windows\\Fonts\\NotoSansSC-VF.ttf",
        "C:\\Windows\\Fonts\\Deng.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ];
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        match FontArc::try_from_vec(bytes) {
            Ok(font) => return Some(font),
            Err(err) => log::warn!("Failed to load UI font {}: {}", path, err),
        }
    }
    log::warn!("No CJK UI font found; overlay text disabled");
    None
}
