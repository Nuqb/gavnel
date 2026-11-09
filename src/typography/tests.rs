use super::*;

#[test]
fn mixed_rtl_text_keeps_latin_shortcuts_and_numbers_in_their_own_direction() {
    let ctx = egui::Context::default();
    ctx.set_fonts(definitions_for("notosanshebrew"));
    let mut output = ctx.run_ui(Default::default(), |ui| {
        ui.label("שלום F8 123 עולם");
    });
    output.textures_delta.clear();
    ctx.fonts_mut(|fonts| {
        let galley = fonts.layout_no_wrap(
            "שלום F8 123 עולם".into(),
            egui::FontId::proportional(18.0),
            egui::Color32::WHITE,
        );
        let glyphs = &galley.rows[0].glyphs;
        assert!(glyphs[0].is_rtl());
        assert!(glyphs[0].pos.x > glyphs.last().unwrap().pos.x);
        let digits: Vec<_> = glyphs
            .iter()
            .filter(|glyph| glyph.chr.is_ascii_digit() && glyph.chr != '8')
            .collect();
        assert!(digits.windows(2).all(|pair| pair[0].pos.x < pair[1].pos.x));
        assert!(digits.iter().all(|glyph| !glyph.is_rtl()));
    });
}

#[test]
fn native_notifications_select_bundled_script_fonts_instead_of_windows_fallbacks() {
    use windows::{Win32::Graphics::Gdi::*, core::PCWSTR};
    for code in [
        "ar", "he-IL", "hi-IN", "bn-BD", "gu-IN", "pa-IN", "kn-IN", "ml-IN", "or-IN", "ta-IN",
        "te-IN", "si-LK", "km-KH", "lo-LA", "th-TH", "my-MM", "am-ET", "hy-AM", "ka-GE", "ja-JP",
        "ko-KR", "zh-CN", "zh-TW",
    ] {
        let family = native_family(code);
        assert_ne!(family, "Segoe UI", "{code}: registration failed");
        assert_eq!(native_family(code), family);
        let wide: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
        let mut actual = [0u16; 128];
        unsafe {
            let dc = CreateCompatibleDC(None);
            let font = CreateFontW(
                -16,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                PCWSTR(wide.as_ptr()),
            );
            let old = SelectObject(dc, font.into());
            GetTextFaceW(dc, Some(&mut actual));
            SelectObject(dc, old);
            let _ = DeleteObject(font.into());
            let _ = DeleteDC(dc);
        }
        let length = actual.iter().position(|c| *c == 0).unwrap();
        assert_eq!(
            String::from_utf16_lossy(&actual[..length]),
            family,
            "{code}"
        );
    }
}
