//! Screen pixels from the diagnostic's own window, including its layered controls.
use anyhow::{Result, ensure};
use windows::Win32::{
    Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::*,
};
use windows::core::w;

pub(super) struct Surface(HWND, HBRUSH);
impl Surface {
    pub fn new(parent: HWND) -> Result<Self> {
        unsafe {
            let instance = HINSTANCE(GetWindowLongPtrW(parent, GWLP_HINSTANCE) as _);
            let brush = CreateSolidBrush(COLORREF(0x00506937));
            let class = w!("Gavnel.OverlayValidationSurface");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(surface_proc),
                hInstance: instance,
                lpszClassName: class,
                hbrBackground: brush,
                ..Default::default()
            });
            let mut rect = Default::default();
            GetClientRect(parent, &mut rect)?;
            let window = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                rect.right,
                rect.bottom,
                Some(parent),
                None,
                Some(instance),
                None,
            )?;
            Ok(Self(window, brush))
        }
    }
}

unsafe extern "system" fn surface_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
            let _ = UnregisterClassW(w!("Gavnel.OverlayValidationSurface"), None);
            let _ = DeleteObject(self.1.into());
        }
    }
}

pub(super) fn capture(window: HWND) -> Result<(u32, u32, Vec<u8>)> {
    unsafe {
        let mut rect = Default::default();
        GetClientRect(window, &mut rect)?;
        let mut origin = POINT::default();
        ClientToScreen(window, &mut origin).ok()?;
        let width = rect.right;
        let height = rect.bottom;
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let previous = SelectObject(memory, bitmap.into());
        let result = (|| {
            BitBlt(
                memory,
                0,
                0,
                width,
                height,
                Some(screen),
                origin.x,
                origin.y,
                SRCCOPY | CAPTUREBLT,
            )?;
            SelectObject(memory, previous);
            let mut pixels = vec![0; (width * height * 4) as usize];
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
            ensure!(
                GetDIBits(
                    memory,
                    bitmap,
                    0,
                    height as u32,
                    Some(pixels.as_mut_ptr().cast()),
                    &mut format,
                    DIB_RGB_COLORS
                ) == height,
                "Could not capture the player diagnostic"
            );
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            Ok((width as u32, height as u32, pixels))
        })();
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        result
    }
}

pub(super) fn verify(window: HWND) -> Result<()> {
    let (width, height, pixels) = capture(window)?;
    std::fs::create_dir_all("work")?;
    let file = std::fs::File::create("work/player-overlay-validation.png")?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    let bottom = &pixels[((height - 88) * width * 4) as usize..];
    let offset = (((height - 16) * width + width / 2) * 4) as usize;
    ensure!(
        pixels[offset..offset + 3] == [55, 105, 80],
        "Control background obscures the native video surface: got {:?}",
        &pixels[offset..offset + 3]
    );
    ensure!(
        bottom
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 220 && p[1] > 220 && p[2] > 220),
        "The player controls disappeared instead of overlaying the video"
    );
    Ok(())
}
