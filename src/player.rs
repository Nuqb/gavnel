use anyhow::{Context, Result};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BLACK_BRUSH, CombineRgn, CreateRectRgn, DeleteObject, GetStockObject, HBRUSH, RGN_DIFF,
            SetWindowRgn,
        },
        Media::MediaFoundation::*,
        System::Com::*,
        UI::WindowsAndMessaging::*,
    },
    core::{BSTR, Interface, implement, w},
};

#[implement(IMFMediaEngineNotify)]
struct Events {
    ctx: egui::Context,
    visible: Arc<AtomicBool>,
    error: Arc<AtomicU32>,
}
impl IMFMediaEngineNotify_Impl for Events_Impl {
    fn EventNotify(&self, event: u32, _param1: usize, param2: u32) -> windows::core::Result<()> {
        if event == MF_MEDIA_ENGINE_EVENT_ERROR.0 as u32 {
            self.error.store(param2.max(1), Ordering::Relaxed);
        }
        if self.visible.load(Ordering::Relaxed) {
            self.ctx.request_repaint();
        }
        Ok(())
    }
}
struct Foundation;
impl Drop for Foundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}
struct VideoWindow(HWND);
impl Drop for VideoWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// Windows' streaming media engine owns playback timing, audio, and decoding.
/// Video is rendered into a child window inside Gavnel, without a CPU-frame upload loop.
pub struct Player {
    engine: IMFMediaEngineEx,
    window: VideoWindow,
    clicked: Box<AtomicBool>,
    _foundation: Foundation,
    pub path: PathBuf,
    pub bookmarks: Vec<f64>,
    pub volume: f64,
    pub muted: bool,
    pub seek_preview: Option<f64>,
    error: Arc<AtomicU32>,
    pending_seek: std::cell::Cell<Option<f64>>,
}
impl Player {
    pub fn new(
        path: PathBuf,
        parent: HWND,
        ctx: egui::Context,
        visible: Arc<AtomicBool>,
    ) -> Result<Self> {
        unsafe {
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
            let foundation = Foundation;
            let instance = HINSTANCE(GetWindowLongPtrW(parent, GWLP_HINSTANCE) as _);
            let class = w!("Gavnel.VideoSurface");
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(video_window_proc),
                hInstance: instance,
                lpszClassName: class,
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                ..Default::default()
            });
            let window = VideoWindow(CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                Some(parent),
                None,
                Some(instance),
                None,
            )?);
            let clicked = Box::new(AtomicBool::new(false));
            SetWindowLongPtrW(
                window.0,
                GWLP_USERDATA,
                (&*clicked as *const AtomicBool) as isize,
            );
            let error = Arc::new(AtomicU32::new(0));
            let events: IMFMediaEngineNotify = Events {
                ctx,
                visible,
                error: error.clone(),
            }
            .into();
            let factory: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)?;
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 3)?;
            let attrs = attrs.unwrap();
            attrs.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &events)?;
            attrs.SetUINT64(&MF_MEDIA_ENGINE_PLAYBACK_HWND, window.0.0 as u64)?;
            let engine = factory.CreateInstance(0, &attrs)?;
            let engine: IMFMediaEngineEx = engine.cast()?;
            engine.SetAutoPlay(true)?;
            let absolute =
                std::fs::canonicalize(&path).context("This clip is no longer available")?;
            let absolute = absolute.to_string_lossy();
            let source = absolute.strip_prefix("\\\\?\\").unwrap_or(&absolute);
            engine.SetSource(&BSTR::from(source))?;
            Ok(Self {
                engine,
                window,
                clicked,
                _foundation: foundation,
                bookmarks: crate::bookmarks::load(&path)?,
                path,
                volume: 1.0,
                muted: false,
                seek_preview: None,
                error,
                pending_seek: std::cell::Cell::new(None),
            })
        }
    }
    pub fn layout(&self, rect: egui::Rect, pixels: f32, visible: bool) -> Result<()> {
        self.layout_cropped(rect, pixels, visible, None)
    }
    pub fn layout_cropped(
        &self,
        rect: egui::Rect,
        pixels: f32,
        visible: bool,
        crop: Option<[f64; 4]>,
    ) -> Result<()> {
        if self.duration() > 0.0
            && let Some(time) = self.pending_seek.take()
        {
            self.seek(time)?;
        }
        unsafe {
            if !visible {
                let _ = ShowWindow(self.window.0, SW_HIDE);
                return Ok(());
            }
            let width = (rect.width() * pixels).round().max(1.0) as i32;
            let height = (rect.height() * pixels).round().max(1.0) as i32;
            SetWindowPos(
                self.window.0,
                None,
                (rect.left() * pixels).round() as i32,
                (rect.top() * pixels).round() as i32,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
            )?;
            let source = crop.map(|[x, y, w, h]| MFVideoNormalizedRect {
                left: x as f32,
                top: y as f32,
                right: (x + w) as f32,
                bottom: (y + h) as f32,
            });
            if self.duration() <= 0.0 {
                return Ok(());
            }
            let (mut native_w, mut native_h) = (0, 0);
            let _ = self
                .engine
                .GetNativeVideoSize(Some(&mut native_w), Some(&mut native_h));
            let [_, _, crop_w, crop_h] = crop.unwrap_or([0.0, 0.0, 1.0, 1.0]);
            let source_w = native_w as f64 * crop_w;
            let source_h = native_h as f64 * crop_h;
            let destination = if source_w > 0.0 && source_h > 0.0 {
                let scale = (width as f64 / source_w).min(height as f64 / source_h);
                let w = (source_w * scale).round() as i32;
                let h = (source_h * scale).round() as i32;
                RECT {
                    left: (width - w) / 2,
                    top: (height - h) / 2,
                    right: (width + w) / 2,
                    bottom: (height + h) / 2,
                }
            } else {
                RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                }
            };
            self.engine.UpdateVideoStream(
                source.as_ref().map(|r| r as *const _),
                Some(&destination),
                Some(&MFARGB {
                    rgbAlpha: 255,
                    rgbRed: 0,
                    rgbGreen: 0,
                    rgbBlue: 0,
                }),
            )?;
        }
        Ok(())
    }
    pub fn take_click(&self) -> bool {
        self.clicked.swap(false, Ordering::Relaxed)
    }
    pub fn set_occlusion(&self, rect: egui::Rect, pixels: f32, popup: Option<egui::Rect>) {
        unsafe {
            let Some(overlap) = popup.map(|r| r.intersect(rect)).filter(|r| r.is_positive()) else {
                SetWindowRgn(self.window.0, None, true);
                return;
            };
            let full = CreateRectRgn(
                0,
                0,
                (rect.width() * pixels).ceil() as i32,
                (rect.height() * pixels).ceil() as i32,
            );
            let cut = CreateRectRgn(
                ((overlap.left() - rect.left()) * pixels).floor() as i32,
                ((overlap.top() - rect.top()) * pixels).floor() as i32,
                ((overlap.right() - rect.left()) * pixels).ceil() as i32,
                ((overlap.bottom() - rect.top()) * pixels).ceil() as i32,
            );
            CombineRgn(Some(full), Some(full), Some(cut), RGN_DIFF);
            let _ = DeleteObject(cut.into());
            if SetWindowRgn(self.window.0, Some(full), true) == 0 {
                let _ = DeleteObject(full.into());
            }
        }
    }
    pub fn duration(&self) -> f64 {
        let d = unsafe { self.engine.GetDuration() };
        if d.is_finite() { d.max(0.0) } else { 0.0 }
    }
    pub fn position(&self) -> f64 {
        unsafe { self.engine.GetCurrentTime().max(0.0) }
    }
    pub fn paused(&self) -> bool {
        unsafe { self.engine.IsPaused().as_bool() }
    }
    pub fn ended(&self) -> bool {
        unsafe { self.engine.IsEnded().as_bool() }
    }
    pub fn play(&self) -> Result<()> {
        unsafe {
            self.engine.Play()?;
        }
        Ok(())
    }
    pub fn pause(&self) -> Result<()> {
        unsafe {
            self.engine.Pause()?;
        }
        Ok(())
    }
    pub fn toggle(&self) -> Result<()> {
        unsafe {
            if self.paused() || self.ended() {
                if self.ended() {
                    self.seek(0.0)?;
                }
                self.engine.Play()?;
            } else {
                self.engine.Pause()?;
            }
        }
        Ok(())
    }
    pub fn seek(&self, seconds: f64) -> Result<()> {
        if self.duration() <= 0.0 {
            self.pending_seek.set(Some(seconds.max(0.0)));
            return Ok(());
        }
        unsafe {
            self.engine
                .SetCurrentTime(seconds.clamp(0.0, self.duration()))?;
        }
        Ok(())
    }
    pub fn audio(&self) -> Result<()> {
        unsafe {
            self.engine.SetVolume(self.volume)?;
            self.engine.SetMuted(self.muted)?;
        }
        Ok(())
    }
    pub fn error(&self) -> Option<String> {
        let error = self.error.load(Ordering::Relaxed);
        (error!=0).then(||format!("This clip could not be played (Windows media error 0x{error:08X}). Try opening it in your default player."))
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            let _ = self.engine.Shutdown();
        }
    }
}

unsafe extern "system" fn video_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_LBUTTONUP {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AtomicBool;
            if !ptr.is_null() {
                (*ptr).store(true, Ordering::Relaxed);
            }
            return LRESULT(0);
        }
        if matches!(message, WM_KEYDOWN | WM_KEYUP)
            && wparam.0 == 0x20
            && let Ok(parent) = GetParent(hwnd)
        {
            let _ = PostMessageW(Some(parent), message, wparam, lparam);
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pump() {
        unsafe {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    #[test]
    #[ignore = "requires the media runtime and NVIDIA encoder; verifies native playback in an invisible test window"]
    fn native_playback_seeks_pauses_and_resumes() {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).unwrap();
        }
        let tools = crate::media::Tools::locate().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.mp4");
        let output = tools
            .command(false)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=30:duration=3",
                "-c:v",
                "h264_nvenc",
                "-an",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        unsafe {
            let class = w!("Gavnel.NativePlaybackTest");
            let instance = HINSTANCE(
                windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                    .unwrap()
                    .0,
            );
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(video_window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            });
            let parent = VideoWindow(
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class,
                    w!("Playback test"),
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    640,
                    360,
                    None,
                    None,
                    Some(instance),
                    None,
                )
                .unwrap(),
            );
            let player = Player::new(
                path,
                parent.0,
                egui::Context::default(),
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
            player.seek(0.75).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while player.position() < 0.85 && std::time::Instant::now() < deadline {
                pump();
                player
                    .layout_cropped(
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 360.0)),
                        1.0,
                        true,
                        Some([0.2, 0.2, 0.4, 0.5]),
                    )
                    .unwrap();
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            assert!(player.error().is_none(), "{:?}", player.error());
            assert!(
                player.position() >= 0.75,
                "Deferred start seek did not apply"
            );
            player.pause().unwrap();
            let position = player.position();
            for _ in 0..6 {
                pump();
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            assert!((player.position() - position).abs() < 0.06);
            player.play().unwrap();
            for _ in 0..12 {
                pump();
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
            assert!(
                player.position() > position + 0.05,
                "Playback did not resume"
            );
            drop(player);
            drop(parent);
            CoUninitialize();
        }
    }
}
