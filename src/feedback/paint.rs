//! Native notification painting.
use super::*;
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}
pub(super) unsafe fn draw(dc: HDC, scale: f32, toast: &Toast, language: crate::i18n::Language) {
    crate::i18n::with_language(language, || unsafe { draw_localized(dc, scale, toast) });
}

unsafe fn draw_localized(dc: HDC, scale: f32, toast: &Toast) {
    unsafe {
        let px = |v: i32| (v as f32 * scale).round() as i32;
        let rect = |x, y, r, b| RECT {
            left: px(x),
            top: px(y),
            right: px(r),
            bottom: px(b),
        };
        let fill = |r: RECT, color| {
            let brush = CreateSolidBrush(color);
            FillRect(dc, &r, brush);
            let _ = DeleteObject(brush.into());
        };
        fill(rect(0, 0, 352, 90), rgb(26, 26, 30));
        fill(rect(0, 18, 3, 72), rgb(255, 132, 49));
        let tile = CreateSolidBrush(rgb(57, 37, 28));
        let old_brush = SelectObject(dc, tile.into());
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        let _ = RoundRect(dc, px(18), px(24), px(59), px(65), px(13), px(13));
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        let _ = DeleteObject(tile.into());
        let pen = CreatePen(PS_SOLID, px(2).max(2), rgb(255, 150, 74));
        let old = SelectObject(dc, pen.into());
        let points = [
            POINT {
                x: px(29),
                y: px(45),
            },
            POINT {
                x: px(36),
                y: px(52),
            },
            POINT {
                x: px(49),
                y: px(37),
            },
        ];
        if toast.title == "Couldn't save clip" || toast.title == "Clip not captured" {
            let stem = [
                POINT {
                    x: px(39),
                    y: px(34),
                },
                POINT {
                    x: px(39),
                    y: px(48),
                },
            ];
            let dot = [
                POINT {
                    x: px(39),
                    y: px(53),
                },
                POINT {
                    x: px(39),
                    y: px(55),
                },
            ];
            let _ = Polyline(dc, &stem);
            let _ = Polyline(dc, &dot);
        } else {
            let _ = Polyline(dc, &points);
        }
        SelectObject(dc, old);
        let _ = DeleteObject(pen.into());
        SetBkMode(dc, TRANSPARENT);
        let family: Vec<u16> =
            crate::typography::native_family(crate::i18n::current_language().code())
                .encode_utf16()
                .chain(Some(0))
                .collect();
        for (text, size, weight, bounds, color) in [
            (
                &toast.title,
                16,
                600,
                rect(74, 20, 334, 44),
                rgb(247, 247, 249),
            ),
            (
                &toast.body,
                12,
                400,
                rect(74, 48, 334, 69),
                rgb(165, 165, 176),
            ),
        ] {
            let font = CreateFontW(
                -px(size),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                windows::core::PCWSTR(family.as_ptr()),
            );
            let old = SelectObject(dc, font.into());
            SetTextColor(dc, color);
            let mut bounds = bounds;
            let mut wide: Vec<_> = crate::i18n::tr(text).encode_utf16().collect();
            DrawTextW(
                dc,
                &mut wide,
                &mut bounds,
                DT_SINGLELINE
                    | DT_VCENTER
                    | DT_END_ELLIPSIS
                    | DT_NOPREFIX
                    | if crate::i18n::is_rtl() {
                        DT_RTLREADING | DT_RIGHT
                    } else {
                        DT_LEFT
                    },
            );
            SelectObject(dc, old);
            let _ = DeleteObject(font.into());
        }
    }
}

// An original soft, ascending two-note cue. No external assets or master-volume changes.
