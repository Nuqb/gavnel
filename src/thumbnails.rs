use crossbeam_channel::{Sender, bounded};
use eframe::egui::{self, ColorImage};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::*,
        System::{
            Com::*,
            Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL},
        },
        UI::Shell::*,
    },
    core::PCWSTR,
};

type Cache = Arc<Mutex<HashMap<PathBuf, Option<Arc<ColorImage>>>>>;
pub struct Thumbnails {
    cache: Cache,
    pending: Arc<Mutex<HashSet<PathBuf>>>,
    requests: Sender<PathBuf>,
}
impl Thumbnails {
    pub fn new(ctx: egui::Context, visible: Arc<AtomicBool>) -> Self {
        let cache: Cache = Arc::new(Mutex::new(HashMap::new()));
        let shared = cache.clone();
        let pending = Arc::new(Mutex::new(HashSet::new()));
        let worker_pending = pending.clone();
        let (requests, rx) = bounded::<PathBuf>(12);
        std::thread::Builder::new()
            .name("gavnel-thumbnails".into())
            .spawn(move || {
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
                }
                for path in rx {
                    // The shell uses Windows' video thumbnail cache. No video is
                    // played or decoded on the UI or capture thread.
                    let image = thumbnail(&path).ok().map(Arc::new);
                    let mut images = shared.lock().unwrap();
                    if images.len() >= 96 {
                        images.clear();
                    }
                    images.insert(path.clone(), image);
                    drop(images);
                    worker_pending.lock().unwrap().remove(&path);
                    if visible.load(Ordering::Relaxed) {
                        ctx.request_repaint();
                    }
                }
                unsafe {
                    CoUninitialize();
                }
            })
            .expect("Could not start thumbnail worker");
        Self {
            cache,
            pending,
            requests,
        }
    }
    pub fn get(&mut self, path: &Path) -> Option<Arc<ColorImage>> {
        if let Some(image) = self.cache.lock().unwrap().get(path) {
            return image.clone();
        }
        let mut pending = self.pending.lock().unwrap();
        if !pending.contains(path) && self.requests.try_send(path.to_owned()).is_ok() {
            pending.insert(path.to_owned());
        }
        None
    }
}
fn thumbnail(path: &Path) -> anyhow::Result<ColorImage> {
    unsafe {
        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let item: IShellItemImageFactory =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
        let bitmap = item.GetImage(SIZE { cx: 384, cy: 216 }, SIIGBF_THUMBNAILONLY)?;
        let result = (|| -> anyhow::Result<ColorImage> {
            let mut desc = BITMAP::default();
            if GetObjectW(
                bitmap.into(),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut desc as *mut _ as _),
            ) == 0
            {
                anyhow::bail!("Thumbnail dimensions unavailable");
            }
            let width = desc.bmWidth as usize;
            let height = desc.bmHeight.unsigned_abs() as usize;
            if width == 0 || height == 0 || width > 384 || height > 216 {
                anyhow::bail!("Invalid thumbnail size");
            }
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels = vec![0u8; width * height * 4];
            let dc = GetDC(None);
            let lines = GetDIBits(
                dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr() as _),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = ReleaseDC(None, dc);
            if lines == 0 {
                anyhow::bail!("Windows could not read the thumbnail");
            }
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            Ok(ColorImage::from_rgba_unmultiplied([width, height], &pixels))
        })();
        let _ = DeleteObject(bitmap.into());
        result
    }
}
