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
        Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH},
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
    _foundation: Foundation,
    pub path: PathBuf,
    pub bookmarks: Vec<f64>,
    pub volume: f64,
    pub muted: bool,
    pub seek_preview: Option<f64>,
    error: Arc<AtomicU32>,
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
                _foundation: foundation,
                bookmarks: crate::bookmarks::load(&path)?,
                path,
                volume: 1.0,
                muted: false,
                seek_preview: None,
                error,
            })
        }
    }
    pub fn layout(&self, rect: egui::Rect, pixels: f32, visible: bool) -> Result<()> {
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
            self.engine.UpdateVideoStream(
                None,
                Some(&RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                }),
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
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
