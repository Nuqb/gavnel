mod preview;
use crate::config::{CaptureMode, Settings};
use anyhow::{Context, Result, bail};
pub use preview::monitor_preview;
use std::collections::{HashMap, HashSet};
use windows::{
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        SizeInt32,
    },
    Win32::{
        Foundation::{CloseHandle, HWND, RECT},
        Graphics::{
            Direct3D11::*,
            Dxgi::{
                DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIDevice,
                IDXGIOutputDuplication,
            },
            Gdi::*,
        },
        System::{
            Diagnostics::ToolHelp::*,
            WinRT::{Direct3D11::*, Graphics::Capture::IGraphicsCaptureItemInterop},
        },
        UI::WindowsAndMessaging::*,
    },
    core::Interface,
};
use windows_capture::{
    d3d11::create_d3d_device,
    dxgi_duplication_api::{DxgiDuplicationApi, DxgiDuplicationFormat},
    monitor::Monitor,
    window::Window,
};

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub handle: usize,
    pub title: String,
    pub executable: String,
}
#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub index: usize,
    pub label: String,
}
pub fn windows() -> Vec<WindowInfo> {
    let mut processes = HashMap::new();
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let len = entry
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    processes.insert(
                        entry.th32ProcessID,
                        String::from_utf16_lossy(&entry.szExeFile[..len]),
                    );
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
    let mut list: Vec<_> = Window::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|w| {
            let title = w.title().ok()?;
            if title.is_empty() {
                return None;
            }
            let executable = w
                .process_id()
                .ok()
                .and_then(|pid| processes.get(&pid).cloned())
                .unwrap_or_default();
            if executable.eq_ignore_ascii_case("gavnel.exe") {
                return None;
            }
            Some(WindowInfo {
                handle: w.as_raw_hwnd() as usize,
                title,
                executable,
            })
        })
        .collect();
    list.sort_by_key(|a| a.title.to_lowercase());
    list
}
pub fn monitors() -> Vec<MonitorInfo> {
    Monitor::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let index = m.index().ok()?;
            Some(MonitorInfo {
                index,
                label: format!(
                    "Display {index} · {} × {}",
                    m.width().unwrap_or(0),
                    m.height().unwrap_or(0)
                ),
            })
        })
        .collect()
}

#[cfg(test)]
mod recording_label_tests {
    use super::*;
    #[test]
    fn automatic_matching_preserves_custom_games_and_prefers_foreground() {
        let mut settings = Settings::default();
        settings.add_game_profile("Custom", "MyGame.exe").unwrap();
        let window = |handle, executable: &str| WindowInfo {
            handle,
            title: "Window".into(),
            executable: executable.into(),
        };
        let windows = vec![
            window(1, "chrome.exe"),
            window(2, "STONESHard.EXE"),
            window(3, "mygame.EXE"),
        ];
        assert_eq!(
            automatic_window(windows.clone(), &settings, 3)
                .unwrap()
                .handle,
            3
        );
        assert_eq!(automatic_window(windows, &settings, 1).unwrap().handle, 2);
        assert!(automatic_window(vec![window(4, "javaw.exe")], &settings, 4).is_none());
    }

    #[test]
    fn game_folders_follow_profile_names_and_other_windows_follow_the_app() {
        let settings = Settings::default();
        let mut window = WindowInfo {
            handle: 0,
            title: "A changing lobby title".into(),
            executable: "FortniteClient-Win64-Shipping.exe".into(),
        };
        assert_eq!(
            Target::Window(window.clone()).recording_label(&settings),
            "Fortnite"
        );
        window.title = "A different match title".into();
        assert_eq!(
            Target::Window(window.clone()).recording_label(&settings),
            "Fortnite"
        );
        window.executable = "MyApp.exe".into();
        assert_eq!(Target::Window(window).recording_label(&settings), "MyApp");
    }
}

#[derive(Clone, Debug)]
pub enum Target {
    Monitor(usize),
    Window(WindowInfo),
}
impl Target {
    pub fn recording_label(&self, settings: &Settings) -> String {
        match self {
            Self::Monitor(_) => self.label(),
            Self::Window(window) => settings
                .profiles
                .iter()
                .find(|p| p.executable.eq_ignore_ascii_case(&window.executable))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| {
                    std::path::Path::new(&window.executable)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| window.title.clone())
                }),
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Monitor(i) => format!("Display {i}"),
            Self::Window(w) => w.title.clone(),
        }
    }
    pub fn handle(&self) -> Option<HWND> {
        match self {
            Self::Window(w) => Some(HWND(w.handle as _)),
            _ => None,
        }
    }
    pub fn foreground(&self) -> bool {
        self.handle().is_none_or(|h| unsafe {
            IsWindow(Some(h)).as_bool() && !IsIconic(h).as_bool() && GetForegroundWindow() == h
        })
    }
    pub fn exists(&self) -> bool {
        self.handle()
            .is_none_or(|h| unsafe { IsWindow(Some(h)).as_bool() })
    }
    pub fn fullscreen(&self) -> bool {
        let Some(hwnd) = self.handle() else {
            return false;
        };
        unsafe {
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let mut rect = RECT::default();
            if !GetMonitorInfoW(monitor, &mut info).as_bool()
                || GetWindowRect(hwnd, &mut rect).is_err()
            {
                return false;
            }
            (rect.left - info.rcMonitor.left).abs() <= 8
                && (rect.top - info.rcMonitor.top).abs() <= 8
                && (rect.right - info.rcMonitor.right).abs() <= 8
                && (rect.bottom - info.rcMonitor.bottom).abs() <= 8
        }
    }
}
fn automatic_window(
    windows: Vec<WindowInfo>,
    settings: &Settings,
    foreground: usize,
) -> Option<WindowInfo> {
    // One profile pass per scan instead of one pass for every open window.
    let executables: HashSet<_> = settings
        .profiles
        .iter()
        .map(|profile| profile.executable.to_ascii_lowercase())
        .collect();
    windows
        .into_iter()
        .filter(|window| executables.contains(&window.executable.to_ascii_lowercase()))
        .min_by_key(|window| window.handle != foreground)
}

pub fn resolve(settings: &Settings) -> Result<Option<Target>> {
    if settings.mode == CaptureMode::Monitor {
        return Ok(Some(Target::Monitor(settings.monitor_index)));
    }
    let windows = windows();
    if settings.mode == CaptureMode::Automatic {
        let foreground = unsafe { GetForegroundWindow().0 as usize };
        return Ok(automatic_window(windows, settings, foreground).map(Target::Window));
    }
    let window = windows.into_iter().find(|w| {
        w.title == settings.window_title
            && w.executable
                .eq_ignore_ascii_case(&settings.window_executable)
    });
    Ok(window.map(Target::Window))
}

pub struct WgcSource {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    winrt_device: IDirect3DDevice,
    size: SizeInt32,
    last: Option<ID3D11Texture2D>,
}
impl WgcSource {
    fn new(target: &Target) -> Result<Self> {
        if !GraphicsCaptureSession::IsSupported()? {
            bail!("Windows screen capture is unavailable on this system.");
        }
        let (device, context) = create_d3d_device()?;
        unsafe {
            let _ = device
                .cast::<ID3D11Multithread>()?
                .SetMultithreadProtected(true);
        }
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe {
            match target {
                Target::Window(w) => interop.CreateForWindow(HWND(w.handle as _))?,
                Target::Monitor(index) => interop
                    .CreateForMonitor(HMONITOR(Monitor::from_index(*index)?.as_raw_hmonitor()))?,
            }
        };
        let size = item.Size()?;
        let winrt_device: IDirect3DDevice =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&device.cast::<IDXGIDevice>()?)? }
                .cast()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        // Explicitly request the fastest available source delivery on Windows 11.
        let _ = session.SetMinUpdateInterval(windows::Foundation::TimeSpan { Duration: 0 });
        session.StartCapture()?;
        Ok(Self {
            device,
            context,
            pool,
            session,
            winrt_device,
            size,
            last: None,
        })
    }
    fn next(&mut self) -> Result<(Option<ID3D11Texture2D>, bool)> {
        let mut fresh = false;
        // Drain stale frames so capture cannot build an unbounded backlog.
        for _ in 0..3 {
            let Ok(frame) = self.pool.TryGetNextFrame() else {
                break;
            };
            let size = frame.ContentSize()?;
            if size.Width <= 0 || size.Height <= 0 {
                let _ = frame.Close();
                continue;
            }
            if size != self.size {
                frame.Close()?;
                self.pool.Recreate(
                    &self.winrt_device,
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    2,
                    size,
                )?;
                self.size = size;
                self.last = None;
                break;
            }
            let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
            let tex: ID3D11Texture2D = unsafe { access.GetInterface()? };
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe {
                tex.GetDesc(&mut desc);
            }
            if self.last.is_none() {
                desc.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
                desc.CPUAccessFlags = 0;
                desc.MiscFlags = 0;
                desc.Usage = D3D11_USAGE_DEFAULT;
                let mut last = None;
                unsafe {
                    self.device.CreateTexture2D(&desc, None, Some(&mut last))?;
                }
                self.last = last;
            }
            unsafe {
                self.context.CopyResource(self.last.as_ref().unwrap(), &tex);
            }
            frame.Close()?;
            fresh = true;
        }
        Ok((self.last.clone(), fresh))
    }
}
impl Drop for WgcSource {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

pub enum CaptureSource {
    Wgc(WgcSource),
    Dxgi {
        api: DxgiDuplicationApi,
        last: Option<ID3D11Texture2D>,
    },
}
impl CaptureSource {
    pub fn new(target: &Target, force_dxgi: bool) -> Result<Self> {
        if force_dxgi
            || target.fullscreen()
            || (matches!(target, Target::Monitor(_))
                && !crate::platform::capture_border_suppression_supported())
        {
            return Self::dxgi(target).context(
                "Desktop capture failed. Check your display connection and graphics driver.",
            );
        }
        WgcSource::new(target).map(Self::Wgc).or_else(|original| {
            if matches!(target, Target::Monitor(_)) {
                Self::dxgi(target)
            } else {
                Err(original)
            }
        })
    }
    fn dxgi(target: &Target) -> Result<Self> {
        let monitor = match target {
            Target::Monitor(i) => Monitor::from_index(*i)?,
            Target::Window(w) => Window::from_raw_hwnd(w.handle as _)
                .monitor()
                .ok_or_else(|| anyhow::anyhow!("Game monitor unavailable"))?,
        };
        let api = DxgiDuplicationApi::new_options(monitor, &[DxgiDuplicationFormat::Bgra8])?;
        unsafe {
            let _ = api
                .device()
                .cast::<ID3D11Multithread>()?
                .SetMultithreadProtected(true);
        }
        Ok(Self::Dxgi { api, last: None })
    }
    pub fn device(&self) -> &ID3D11Device {
        match self {
            Self::Wgc(s) => &s.device,
            Self::Dxgi { api, .. } => api.device(),
        }
    }
    pub fn context(&self) -> &ID3D11DeviceContext {
        match self {
            Self::Wgc(s) => &s.context,
            Self::Dxgi { api, .. } => api.device_context(),
        }
    }
    pub fn backend(&self) -> &'static str {
        match self {
            Self::Wgc(_) => "Windows window/display capture",
            Self::Dxgi { .. } => "Fullscreen monitor compatibility capture",
        }
    }
    pub fn latest(&self) -> Option<ID3D11Texture2D> {
        match self {
            Self::Wgc(source) => source.last.clone(),
            Self::Dxgi { last, .. } => last.clone(),
        }
    }
    pub fn next(&mut self) -> Result<(Option<ID3D11Texture2D>, bool)> {
        match self {
            Self::Wgc(s) => s.next(),
            Self::Dxgi { api, last } => {
                let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
                let mut resource = None;
                // Release ownership after queuing the GPU copy, before the
                // capture timer sleeps. Holding it until the next zero-timeout
                // acquire can alternate new/stale images on NVIDIA drivers.
                match unsafe {
                    api.duplication()
                        .AcquireNextFrame(0, &mut info, &mut resource)
                } {
                    Ok(()) => {
                        let _release = AcquiredDesktop(api.duplication().clone());
                        let texture: ID3D11Texture2D = resource
                            .context("Capture returned no desktop texture")?
                            .cast()?;
                        if last.is_none() {
                            let mut desc = D3D11_TEXTURE2D_DESC::default();
                            unsafe {
                                texture.GetDesc(&mut desc);
                            }
                            desc.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
                            desc.CPUAccessFlags = 0;
                            desc.MiscFlags = 0;
                            desc.Usage = D3D11_USAGE_DEFAULT;
                            unsafe {
                                api.device().CreateTexture2D(&desc, None, Some(last))?;
                            }
                        }
                        unsafe {
                            api.device_context()
                                .CopyResource(last.as_ref().unwrap(), &texture);
                        }
                        Ok((last.clone(), info.LastPresentTime != 0))
                    }
                    Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => Ok((last.clone(), false)),
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

struct AcquiredDesktop(IDXGIOutputDuplication);
impl Drop for AcquiredDesktop {
    fn drop(&mut self) {
        unsafe {
            let _ = self.0.ReleaseFrame();
        }
    }
}
