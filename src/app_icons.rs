use eframe::egui;
use windows::{
    Win32::{
        Foundation::CloseHandle,
        Graphics::Gdi::*,
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
        System::Threading::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    },
    core::{PCWSTR, PWSTR},
};
pub fn texture(ctx: &egui::Context, pid: u32) -> Option<egui::TextureHandle> {
    let key = egui::Id::unique(("process-icon", pid));
    if let Some(cached) = ctx.data_mut(|d| d.get_temp::<Option<egui::TextureHandle>>(key)) {
        return cached;
    }
    let texture = read(pid)
        .map(|image| ctx.load_texture(format!("app-{pid}"), image, egui::TextureOptions::LINEAR));
    ctx.data_mut(|d| d.insert_temp(key, texture.clone()));
    texture
}
fn read(pid: u32) -> Option<egui::ColorImage> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path = [0u16; 32768];
        let mut length = path.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let mut info = SHFILEINFOW::default();
        if SHGetFileInfoW(
            PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        ) == 0
        {
            return None;
        }
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let _ = ReleaseDC(None, screen);
        if dc.is_invalid() {
            let _ = DestroyIcon(info.hIcon);
            return None;
        }
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = std::ptr::null_mut();
        let bitmap =
            match CreateDIBSection(Some(dc), &bitmap_info, DIB_RGB_COLORS, &mut pixels, None, 0) {
                Ok(b) => b,
                Err(_) => {
                    let _ = DeleteDC(dc);
                    let _ = DestroyIcon(info.hIcon);
                    return None;
                }
            };
        let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
        std::ptr::write_bytes(pixels, 0, 32 * 32 * 4);
        let result = DrawIconEx(dc, 0, 0, info.hIcon, 32, 32, 0, None, DI_NORMAL);
        let mut bytes = std::slice::from_raw_parts(pixels as *const u8, 32 * 32 * 4).to_vec();
        let _ = SelectObject(dc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = DestroyIcon(info.hIcon);
        result.ok()?;
        let has_alpha = bytes.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        for pixel in bytes.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            if !has_alpha {
                pixel[3] = 255;
            }
        }
        Some(egui::ColorImage::from_rgba_unmultiplied([32, 32], &bytes))
    }
}
