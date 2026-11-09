//! Temporary display thumbnails used only by the source picker.
use super::*;
/// Small, local-only display preview for the source picker. No image is saved.
pub fn monitor_preview(index: usize) -> Result<eframe::egui::ColorImage> {
    unsafe {
        let monitor = Monitor::from_index(index)?;
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        anyhow::ensure!(
            GetMonitorInfoW(HMONITOR(monitor.as_raw_hmonitor()), &mut info).as_bool(),
            "Display unavailable"
        );
        let screen = GetDC(None);
        anyhow::ensure!(!screen.is_invalid(), "Display preview unavailable");
        let memory = CreateCompatibleDC(Some(screen));
        if memory.is_invalid() {
            ReleaseDC(None, screen);
            anyhow::bail!("Display preview unavailable");
        }
        let width = 448;
        let height = 280;
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        if bitmap.is_invalid() {
            let _ = DeleteDC(memory);
            ReleaseDC(None, screen);
            anyhow::bail!("Display preview unavailable");
        }
        let old = SelectObject(memory, bitmap.into());
        let result = (|| -> Result<_> {
            let _ = SetStretchBltMode(memory, HALFTONE);
            StretchBlt(
                memory,
                0,
                0,
                width,
                height,
                Some(screen),
                info.rcMonitor.left,
                info.rcMonitor.top,
                info.rcMonitor.right - info.rcMonitor.left,
                info.rcMonitor.bottom - info.rcMonitor.top,
                SRCCOPY,
            )
            .ok()?;
            SelectObject(memory, old);
            let mut pixels = vec![0u8; (width * height * 4) as usize];
            let mut format = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            anyhow::ensure!(
                GetDIBits(
                    memory,
                    bitmap,
                    0,
                    height as u32,
                    Some(pixels.as_mut_ptr().cast()),
                    &mut format,
                    DIB_RGB_COLORS
                ) == height,
                "Could not read display preview"
            );
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            Ok(eframe::egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                &pixels,
            ))
        })();
        SelectObject(memory, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        result
    }
}
