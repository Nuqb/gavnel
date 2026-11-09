//! Desktop confirmations on a dedicated message thread; never activate or intercept input.
mod paint;
mod sound;
use crate::config::ClipFeedback;
use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, bounded};
use paint::draw;
use sound::chime;
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        Media::Audio::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::{PCWSTR, w},
};

const WAKE: u32 = WM_APP + 62;
const LIFETIME: Duration = Duration::from_secs(3);
const STYLE: WINDOW_EX_STYLE = WINDOW_EX_STYLE(
    WS_EX_NOACTIVATE.0
        | WS_EX_TOOLWINDOW.0
        | WS_EX_TOPMOST.0
        | WS_EX_TRANSPARENT.0
        | WS_EX_LAYERED.0,
);

pub enum Event {
    Configure(ClipFeedback, crate::i18n::Language),
    Bookmarked {
        source: String,
        seconds: f64,
    },
    Captured {
        id: u64,
        source: String,
        seconds: f64,
    },
    Finished {
        id: u64,
        success: bool,
    },
    Failed(String),
    Preview(ClipFeedback, crate::i18n::Language),
}

#[derive(Clone)]
pub struct Feedback(Arc<Inner>);
struct Inner {
    tx: Sender<Event>,
    hwnd: usize,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl Feedback {
    pub fn new() -> Result<Self> {
        let (tx, rx) = bounded(64);
        let (ready, startup) = bounded(1);
        let worker = std::thread::Builder::new()
            .name("gavnel-clip-feedback".into())
            .spawn(move || {
                let mut state = Box::new(RefCell::new(State {
                    rx,
                    preferences: ClipFeedback::default(),
                    presentation: Presentation::default(),
                    sound: Vec::new(),
                    scale: 1.0,
                    language: crate::i18n::Language::default(),
                }));
                let window = unsafe { create_window((&mut *state) as *mut RefCell<State> as _) };
                match window {
                    Ok(hwnd) => {
                        let _ = ready.send(Ok(hwnd.0 as usize));
                        unsafe {
                            let mut message = MSG::default();
                            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                                let _ = TranslateMessage(&message);
                                DispatchMessageW(&message);
                            }
                            let _ = PlaySoundW(PCWSTR::null(), None, SND_FLAGS(0));
                        }
                    }
                    Err(error) => {
                        let _ = ready.send(Err(format!("{error:#}")));
                    }
                }
            })?;
        let hwnd = startup
            .recv()
            .context("Clip feedback did not start")?
            .map_err(anyhow::Error::msg)?;
        Ok(Self(Arc::new(Inner {
            tx,
            hwnd,
            worker: Mutex::new(Some(worker)),
        })))
    }
    pub fn send(&self, event: Event) {
        if self.0.tx.try_send(event).is_ok() {
            unsafe {
                let _ = PostMessageW(Some(HWND(self.0.hwnd as _)), WAKE, WPARAM(0), LPARAM(0));
            }
        }
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        // WM_CLOSE bypasses a full feedback queue and is handled by its owner thread.
        unsafe {
            let _ = PostMessageW(Some(HWND(self.hwnd as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

struct Toast {
    id: u64,
    title: String,
    body: String,
    born: Instant,
}
#[derive(Default)]
struct Presentation {
    current: Option<Toast>,
    completed: VecDeque<(u64, bool)>,
}
impl Presentation {
    fn captured(&mut self, id: u64, source: String, seconds: f64, now: Instant) {
        let title = match self.completed.iter().find(|(i, _)| *i == id) {
            Some((_, true)) => "Clip saved",
            Some((_, false)) => "Couldn't save clip",
            None => "Clip captured",
        };
        self.current = Some(Toast {
            id,
            title: title.into(),
            body: format!(
                "{source} · {}:{:02}",
                seconds.round() as u64 / 60,
                seconds.round() as u64 % 60
            ),
            born: now,
        });
    }
    fn finished(&mut self, id: u64, success: bool) {
        self.completed.push_back((id, success));
        while self.completed.len() > 16 {
            self.completed.pop_front();
        }
        if let Some(toast) = self.current.as_mut().filter(|toast| toast.id == id) {
            toast.title = if success {
                "Clip saved"
            } else {
                "Couldn't save clip"
            }
            .into();
            if !success {
                toast.body = "Open Gavnel for details. Your game keeps focus.".into();
            }
        }
    }
    fn alpha(&self, now: Instant) -> u8 {
        self.current.as_ref().map_or(0, |toast| {
            let elapsed = now.saturating_duration_since(toast.born).as_secs_f32();
            let fade = (elapsed / 0.14).min((3.0 - elapsed) / 0.18).clamp(0.0, 1.0);
            (fade * fade * (3.0 - 2.0 * fade) * 255.0) as u8
        })
    }
    fn expired(&self, now: Instant) -> bool {
        self.current
            .as_ref()
            .is_none_or(|toast| now.saturating_duration_since(toast.born) >= LIFETIME)
    }
}

struct State {
    rx: Receiver<Event>,
    preferences: ClipFeedback,
    presentation: Presentation,
    sound: Vec<u8>,
    scale: f32,
    language: crate::i18n::Language,
}
impl State {
    unsafe fn events(&mut self, hwnd: HWND) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Configure(preferences, language) => {
                    self.language = language;
                    if !preferences.notification {
                        unsafe {
                            self.hide(hwnd);
                        }
                    }
                    if !preferences.sound {
                        unsafe {
                            let _ = PlaySoundW(PCWSTR::null(), None, SND_FLAGS(0));
                        }
                    }
                    self.preferences = preferences;
                }
                Event::Bookmarked { source, seconds } => {
                    self.presentation.current = Some(Toast {
                        id: 0,
                        title: "Bookmark saved".into(),
                        body: format!(
                            "{source} · {}:{:02}",
                            seconds as u64 / 60,
                            seconds as u64 % 60
                        ),
                        born: Instant::now(),
                    });
                    let preferences = self.preferences.clone();
                    unsafe {
                        self.present(hwnd, &preferences);
                    }
                }
                Event::Captured {
                    id,
                    source,
                    seconds,
                } => {
                    self.presentation
                        .captured(id, source, seconds, Instant::now());
                    let mut preferences = self.preferences.clone();
                    if self
                        .presentation
                        .completed
                        .iter()
                        .any(|(done, success)| *done == id && !success)
                    {
                        preferences.sound = false;
                    }
                    unsafe {
                        self.present(hwnd, &preferences);
                    }
                }
                Event::Finished { id, success } => {
                    let now = Instant::now();
                    let expired = self.presentation.expired(now);
                    self.presentation.finished(id, success);
                    if !success
                        && expired
                        && self.preferences.notification
                        && let Some(toast) = self
                            .presentation
                            .current
                            .as_mut()
                            .filter(|toast| toast.id == id)
                    {
                        toast.born = now;
                        unsafe {
                            self.show(hwnd);
                        }
                    }
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                Event::Failed(reason) => {
                    self.presentation.current = Some(Toast {
                        id: 0,
                        title: "Clip not captured".into(),
                        body: reason,
                        born: Instant::now(),
                    });
                    if self.preferences.notification {
                        unsafe {
                            self.show(hwnd);
                        }
                    }
                }
                Event::Preview(preferences, language) => {
                    self.language = language;
                    self.presentation.current = Some(Toast {
                        id: 0,
                        title: "Clip saved".into(),
                        body: "Preview · no clip was created".into(),
                        born: Instant::now(),
                    });
                    unsafe {
                        self.present(hwnd, &preferences);
                    }
                }
            }
        }
    }
    unsafe fn present(&mut self, hwnd: HWND, preferences: &ClipFeedback) {
        if preferences.sound && preferences.volume > 0.0 {
            unsafe {
                // PlaySound keeps the memory pointer until playback finishes. Stop
                // before replacing it, and keep the new buffer alive on this thread.
                let _ = PlaySoundW(PCWSTR::null(), None, SND_FLAGS(0));
                self.sound = chime(preferences.volume);
                let _ = PlaySoundW(
                    PCWSTR(self.sound.as_ptr().cast()),
                    None,
                    SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
                );
            }
        }
        if preferences.notification {
            unsafe {
                self.show(hwnd);
            }
        }
    }
    unsafe fn show(&mut self, hwnd: HWND) {
        unsafe {
            let foreground = GetForegroundWindow();
            self.scale = (GetDpiForWindow(foreground).max(96) as f32 / 96.0).clamp(1.0, 4.0);
            let px = |value: f32| (value * self.scale).round() as i32;
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(
                MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            );
            let width = px(352.0);
            let height = px(90.0);
            let margin = px(22.0);
            let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, px(20.0), px(20.0));
            if SetWindowRgn(hwnd, Some(region), false) == 0 {
                let _ = DeleteObject(region.into());
            }
            let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 0, LWA_ALPHA);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                monitor.rcWork.right - width - margin,
                monitor.rcWork.bottom - height - margin,
                width,
                height,
                SWP_NOACTIVATE,
            );
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = InvalidateRect(Some(hwnd), None, false);
            SetTimer(Some(hwnd), 1, 16, None);
        }
    }
    unsafe fn hide(&mut self, hwnd: HWND) {
        unsafe {
            let _ = KillTimer(Some(hwnd), 1);
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
    unsafe fn tick(&mut self, hwnd: HWND) {
        let now = Instant::now();
        unsafe {
            if self.presentation.expired(now) {
                self.hide(hwnd);
            } else {
                let _ = SetLayeredWindowAttributes(
                    hwnd,
                    COLORREF(0),
                    self.presentation.alpha(now),
                    LWA_ALPHA,
                );
            }
        }
    }
}

unsafe fn create_window(state: *const std::ffi::c_void) -> Result<HWND> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("GavnelClipFeedback"),
            ..Default::default()
        };
        // Already-registered is normal when a hidden-window test creates another worker.
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            STYLE,
            class.lpszClassName,
            w!("Gavnel clip confirmation"),
            WS_POPUP,
            0,
            0,
            352,
            90,
            None,
            None,
            Some(instance),
            Some(state),
        )?;
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        Ok(hwnd)
    }
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        match message {
            WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
            WM_NCHITTEST => return LRESULT(HTTRANSPARENT as isize),
            WM_ERASEBKGND => return LRESULT(1),
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            _ => {}
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<State>;
        if let Some(cell) = ptr.as_ref() {
            match message {
                WAKE | WM_TIMER => {
                    if let Ok(mut state) = cell.try_borrow_mut() {
                        if message == WAKE {
                            state.events(hwnd);
                        } else {
                            state.tick(hwnd);
                        }
                    }
                    return LRESULT(0);
                }
                WM_PAINT => {
                    let mut paint = PAINTSTRUCT::default();
                    let dc = BeginPaint(hwnd, &mut paint);
                    if let Ok(state) = cell.try_borrow()
                        && let Some(toast) = &state.presentation.current
                    {
                        draw(dc, state.scale, toast, state.language);
                    }
                    let _ = EndPaint(hwnd, &paint);
                    return LRESULT(0);
                }
                _ => {}
            }
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "writes an offscreen native notification preview; no displayed window or sound"]
    fn export_native_notification_preview() {
        unsafe {
            let width = 704i32;
            let height = 180i32;
            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
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
            let mut pixels = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixels, None, 0).unwrap();
            let old = SelectObject(dc, bitmap.into());
            draw(
                dc,
                2.0,
                &Toast {
                    id: 1,
                    title: "Clip saved".into(),
                    body: "Fortnite · 60 seconds".into(),
                    born: Instant::now(),
                },
                crate::i18n::Language::EnglishUs,
            );
            let _ = GdiFlush();
            let bytes =
                std::slice::from_raw_parts(pixels as *const u8, (width * height * 4) as usize);
            let mut bmp = Vec::new();
            bmp.extend_from_slice(b"BM");
            bmp.extend_from_slice(&(54u32 + bytes.len() as u32).to_le_bytes());
            bmp.extend_from_slice(&[0; 4]);
            bmp.extend_from_slice(&54u32.to_le_bytes());
            bmp.extend_from_slice(&40u32.to_le_bytes());
            bmp.extend_from_slice(&width.to_le_bytes());
            bmp.extend_from_slice(&(-height).to_le_bytes());
            bmp.extend_from_slice(&1u16.to_le_bytes());
            bmp.extend_from_slice(&32u16.to_le_bytes());
            bmp.extend_from_slice(&[0; 24]);
            bmp.extend_from_slice(bytes);
            std::fs::create_dir_all("work").unwrap();
            std::fs::write("work/clip-feedback-preview.bmp", bmp).unwrap();
            SelectObject(dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
        }
    }
    #[test]
    fn confirmations_expire_in_three_seconds_and_results_do_not_extend_them() {
        let mut p = Presentation::default();
        let now = Instant::now();
        p.captured(1, "Game".into(), 60.0, now);
        assert_eq!(p.alpha(now), 0);
        assert_eq!(p.alpha(now + Duration::from_millis(200)), 255);
        p.finished(1, true);
        assert_eq!(p.current.as_ref().unwrap().title, "Clip saved");
        assert!(!p.expired(now + Duration::from_millis(2999)));
        assert!(p.expired(now + LIFETIME));
        assert_eq!(p.alpha(now + LIFETIME), 0);
        p.captured(2, "Next game".into(), 30.0, now + Duration::from_secs(2));
        p.finished(1, false);
        assert_eq!(p.current.as_ref().unwrap().id, 2);
        assert_eq!(p.current.as_ref().unwrap().title, "Clip captured");
    }
    #[test]
    fn fast_writer_completion_is_preserved_and_history_is_bounded() {
        let mut p = Presentation::default();
        for id in 1..100 {
            p.finished(id, id % 2 == 0);
        }
        p.captured(99, "Game".into(), 60.0, Instant::now());
        assert_eq!(p.current.as_ref().unwrap().title, "Couldn't save clip");
        assert_eq!(p.completed.len(), 16);
    }
    #[test]
    fn generated_chime_is_valid_bounded_pcm_and_volume_changes_only_samples() {
        let full = chime(1.0);
        let quiet = chime(0.5);
        let silent = chime(0.0);
        assert_eq!(&full[..4], b"RIFF");
        assert_eq!(&full[8..16], b"WAVEfmt ");
        assert_eq!(full.len(), 44 + 28_800);
        assert_eq!(&full[..44], &quiet[..44]);
        assert!(silent[44..].iter().all(|b| *b == 0));
        let samples: Vec<_> = full[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|s| i16::from_le_bytes([s[0], s[1]]))
            .collect();
        assert_eq!(samples[0], 0);
        assert!(samples.last().unwrap().abs() < 2);
        assert!(samples.iter().map(|s| s.abs()).max().unwrap() < 9_000);
        for (a, b) in samples.iter().zip(quiet[44..].as_chunks::<2>().0.iter()) {
            assert!((*a as i32 / 2 - i16::from_le_bytes([b[0], b[1]]) as i32).abs() <= 1);
        }
    }
    #[test]
    fn native_confirmation_is_hidden_and_cannot_activate_or_intercept_clicks() {
        let feedback = Feedback::new().unwrap();
        unsafe {
            let hwnd = HWND(feedback.0.hwnd as _);
            assert!(!IsWindowVisible(hwnd).as_bool());
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            assert_eq!(style & STYLE.0, STYLE.0);
            assert_eq!(
                SendMessageW(hwnd, WM_MOUSEACTIVATE, None, None).0,
                MA_NOACTIVATE as isize
            );
            assert_eq!(
                SendMessageW(hwnd, WM_NCHITTEST, None, None).0,
                HTTRANSPARENT as isize
            );
        }
        // No window is shown, sound played, or device captured by this test.
    }
}
