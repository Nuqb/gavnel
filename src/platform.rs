use anyhow::Result;
use std::path::Path;

mod compatibility;
pub use compatibility::application_audio_supported;

const APP_INSTANCE: &str = "Local\\Gavnel.DesktopApp";
const SHOW_EVENT: &str = "Local\\Gavnel.ShowWindow";

/// Wake the native window before asking egui to repaint it. Windows does not
/// reliably deliver redraws to a hidden/minimized window, so a queued viewport
/// command alone cannot reopen it or finish an Exit request from the tray.
#[derive(Clone)]
pub struct WindowActions {
    hwnd: usize,
    ctx: eframe::egui::Context,
    visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
    quitting: std::sync::Arc<std::sync::atomic::AtomicBool>,
    exit_blocked: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl WindowActions {
    pub fn new(
        hwnd: HWND,
        ctx: eframe::egui::Context,
        visible: std::sync::Arc<std::sync::atomic::AtomicBool>,
        quitting: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            hwnd: hwnd.0 as usize,
            ctx,
            visible,
            quitting,
            exit_blocked: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub fn show(&self) {
        use std::sync::atomic::Ordering;
        if self.quitting.load(Ordering::Acquire) {
            return;
        }
        self.visible.store(true, Ordering::Release);
        restore_window(HWND(self.hwnd as _));
        self.ctx
            .send_viewport_cmd(eframe::egui::ViewportCommand::Visible(true));
        self.ctx
            .send_viewport_cmd(eframe::egui::ViewportCommand::Minimized(false));
        self.ctx
            .send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
        self.ctx.request_repaint();
    }
    pub fn hide(&self) {
        use std::sync::atomic::Ordering;
        if self.quitting.load(Ordering::Acquire) {
            return;
        }
        self.visible.store(false, Ordering::Release);
        self.ctx
            .send_viewport_cmd(eframe::egui::ViewportCommand::Visible(false));
        self.ctx.request_repaint();
    }
    pub fn set_exit_blocked(&self, blocked: bool) {
        self.exit_blocked
            .store(blocked, std::sync::atomic::Ordering::Release);
    }
    pub fn exit(&self) {
        use std::sync::atomic::Ordering;
        use windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::WindowsAndMessaging::*,
        };
        if self.exit_blocked.load(Ordering::Acquire) {
            self.show();
            return;
        }
        self.quitting.store(true, Ordering::Release);
        self.visible.store(true, Ordering::Release);
        // A native close event wakes the event loop even when no frame has
        // been drawn since hiding. The quit flag bypasses close-to-tray.
        unsafe {
            let hwnd = HWND(self.hwnd as _);
            if IsWindow(Some(hwnd)).as_bool() {
                let _ = ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        self.ctx
            .send_viewport_cmd(eframe::egui::ViewportCommand::Close);
        self.ctx.request_repaint();
    }
    /// Exercise the real app's close-to-tray and native wake paths without
    /// recording or modifying preferences. Only used by --lifecycle-test.
    pub fn verify_lifecycle(&self) -> std::thread::JoinHandle<Result<()>> {
        let actions = self.clone();
        std::thread::spawn(move || {
            use std::{
                sync::atomic::Ordering,
                time::{Duration, Instant},
            };
            use windows::Win32::{
                Foundation::{LPARAM, WPARAM},
                System::Threading::*,
                UI::WindowsAndMessaging::*,
            };
            let wait = |label: &str, predicate: &dyn Fn() -> bool| -> Result<()> {
                let deadline = Instant::now() + Duration::from_secs(5);
                while Instant::now() < deadline {
                    if predicate() {
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                anyhow::bail!("Window lifecycle test timed out: {label}");
            };
            let result = (|| -> Result<()> {
                let hwnd = HWND(actions.hwnd as _);
                wait("initial window", &|| unsafe {
                    IsWindowVisible(hwnd).as_bool()
                })?;
                for cycle in 0..4 {
                    unsafe {
                        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))?;
                    }
                    wait("close to tray", &|| {
                        !actions.visible.load(Ordering::Acquire)
                            && unsafe { !IsWindowVisible(hwnd).as_bool() }
                    })?;
                    // Allow the hidden window's outstanding paints to finish.
                    std::thread::sleep(Duration::from_millis(300));
                    if cycle % 2 == 0 {
                        actions.show();
                    } else {
                        let name: Vec<u16> = SHOW_EVENT.encode_utf16().chain(Some(0)).collect();
                        unsafe {
                            let event =
                                OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name.as_ptr()))?;
                            let result = SetEvent(event);
                            let _ = windows::Win32::Foundation::CloseHandle(event);
                            result?;
                        }
                    }
                    wait("reopen from tray/launcher", &|| {
                        actions.visible.load(Ordering::Acquire)
                            && unsafe { IsWindowVisible(hwnd).as_bool() }
                    })?;
                    println!(
                        "Lifecycle cycle {}: close-to-tray and {} reopen passed",
                        cycle + 1,
                        if cycle % 2 == 0 { "tray" } else { "launcher" }
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
                unsafe {
                    PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))?;
                }
                wait("hide before Exit", &|| {
                    !actions.visible.load(Ordering::Acquire)
                        && unsafe { !IsWindowVisible(hwnd).as_bool() }
                })?;
                std::thread::sleep(Duration::from_millis(300));
                println!("Lifecycle: requesting Exit while hidden");
                Ok(())
            })();
            actions.exit();
            result
        })
    }
}

/// Use native restore messages as well as egui commands. A hidden window may
/// not receive a redraw until Windows has made it visible again.
fn restore_window(hwnd: HWND) {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::*,
    };
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return;
        }
        let command = if IsIconic(hwnd).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        let _ = ShowWindowAsync(hwnd, command);
        let _ = PostMessageW(
            Some(hwnd),
            WM_SYSCOMMAND,
            WPARAM(SC_RESTORE as usize),
            LPARAM(0),
        );
        let _ = SetForegroundWindow(hwnd);
    }
}
pub struct AppInstance {
    mutex: windows::Win32::Foundation::HANDLE,
    show: windows::Win32::Foundation::HANDLE,
}
impl AppInstance {
    pub fn acquire() -> Result<Option<Self>> {
        use windows::Win32::{
            Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
            System::Threading::{CreateEventW, CreateMutexW, SetEvent},
        };
        let name: Vec<u16> = APP_INSTANCE.encode_utf16().chain(Some(0)).collect();
        let event: Vec<u16> = SHOW_EVENT.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let mutex = CreateMutexW(None, false, PCWSTR(name.as_ptr()))?;
            let existing = GetLastError() == ERROR_ALREADY_EXISTS;
            let show = match CreateEventW(None, false, false, PCWSTR(event.as_ptr())) {
                Ok(show) => show,
                Err(e) => {
                    let _ = CloseHandle(mutex);
                    return Err(e.into());
                }
            };
            if existing {
                // A desktop launch may grant foreground activation to the
                // running instance; restrict the grant to Gavnel's process.
                use windows::Win32::UI::WindowsAndMessaging::*;
                if let Ok(hwnd) = FindWindowW(None, w!("Gavnel")) {
                    let mut pid = 0;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    if pid != 0 {
                        let _ = AllowSetForegroundWindow(pid);
                    }
                    restore_window(hwnd);
                }
                let _ = SetEvent(show);
                let _ = CloseHandle(show);
                let _ = CloseHandle(mutex);
                return Ok(None);
            }
            Ok(Some(Self { mutex, show }))
        }
    }
}
impl Drop for AppInstance {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.show);
            let _ = windows::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

pub struct WindowActivator {
    exit: windows::Win32::Foundation::HANDLE,
    show: windows::Win32::Foundation::HANDLE,
    stop: windows::Win32::Foundation::HANDLE,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl WindowActivator {
    pub fn new(actions: WindowActions) -> Result<Self> {
        use windows::Win32::{
            Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
            System::Threading::*,
        };
        let event: Vec<u16> = SHOW_EVENT.encode_utf16().chain(Some(0)).collect();
        let exit_name: Vec<u16> = "Local\\Gavnel.ExitForUpdate"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            let show = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(event.as_ptr()))?;
            let stop = match CreateEventW(None, true, false, None) {
                Ok(stop) => stop,
                Err(e) => {
                    let _ = CloseHandle(show);
                    return Err(e.into());
                }
            };
            let exit = match CreateEventW(None, false, false, PCWSTR(exit_name.as_ptr())) {
                Ok(exit) => exit,
                Err(e) => {
                    let _ = CloseHandle(show);
                    let _ = CloseHandle(stop);
                    return Err(e.into());
                }
            };
            let exit_id = exit.0 as usize;
            // The worker borrows these event handles. Drop signals it and
            // joins it before closing either handle. No polling while hidden.
            let show_id = show.0 as usize;
            let stop_id = stop.0 as usize;
            let thread = std::thread::Builder::new()
                .name("gavnel-launcher".into())
                .spawn(move || {
                    loop {
                        let event = WaitForMultipleObjects(
                            &[
                                HANDLE(stop_id as _),
                                HANDLE(show_id as _),
                                HANDLE(exit_id as _),
                            ],
                            false,
                            INFINITE,
                        );
                        if event.0 == WAIT_OBJECT_0.0 + 2 {
                            actions.exit();
                            break;
                        }
                        if event.0 != WAIT_OBJECT_0.0 + 1 {
                            break;
                        }
                        actions.show();
                    }
                });
            match thread {
                Ok(thread) => Ok(Self {
                    exit,
                    show,
                    stop,
                    thread: Some(thread),
                }),
                Err(e) => {
                    let _ = CloseHandle(show);
                    let _ = CloseHandle(stop);
                    let _ = CloseHandle(exit);
                    Err(e.into())
                }
            }
        }
    }
}
impl Drop for WindowActivator {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::System::Threading::SetEvent(self.stop);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.exit);
            let _ = windows::Win32::Foundation::CloseHandle(self.show);
            let _ = windows::Win32::Foundation::CloseHandle(self.stop);
        }
    }
}
use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, HWND},
        System::Registry::*,
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
    core::{PCWSTR, w},
};
pub fn choose_clip_folder(initial: &Path) -> Result<Option<std::path::PathBuf>> {
    use windows::Win32::{
        System::Com::*,
        UI::{Input::KeyboardAndMouse::GetActiveWindow, Shell::*},
    };
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR,
        )?;
        let title: Vec<u16> = crate::i18n::tr("Choose where Gavnel saves clips")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let button: Vec<u16> = crate::i18n::tr("Use this folder")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        dialog.SetTitle(PCWSTR(title.as_ptr()))?;
        dialog.SetOkButtonLabel(PCWSTR(button.as_ptr()))?;
        if initial.is_dir() {
            let wide: Vec<u16> = initial
                .as_os_str()
                .to_string_lossy()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            if let Ok(item) =
                SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(wide.as_ptr()), None)
            {
                let _ = dialog.SetFolder(&item);
            }
        }
        match dialog.Show(Some(GetActiveWindow())) {
            Err(e) if e.code() == windows::core::HRESULT::from_win32(1223) => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(()) => {}
        }
        let item = dialog.GetResult()?;
        let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0 as _));
        Ok(Some(std::path::PathBuf::from(path?)))
    }
}

pub fn validate_clip_folder(path: &Path) -> Result<()> {
    use anyhow::Context;
    if !path.is_absolute() {
        anyhow::bail!("Choose a complete folder path, such as F:\\Clips.");
    }
    std::fs::create_dir_all(path).context(
        "Could not create this folder. Choose another location or check its permissions.",
    )?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let probe = path.join(format!(
        ".gavnel-write-check-{}-{nonce}",
        std::process::id()
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .context("Gavnel cannot save to this folder. Choose a writable location.")?;
    drop(file);
    std::fs::remove_file(probe)?;
    Ok(())
}
pub fn open(path: &Path) -> Result<()> {
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            w!("open"),
            PCWSTR(wide.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        anyhow::bail!("Windows could not open this file or folder.");
    }
    Ok(())
}
pub fn reveal(path: &Path) -> Result<()> {
    open(path.parent().unwrap_or(path))
}
pub fn startup(enabled: bool) -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let result = if enabled {
            let exe = std::env::current_exe()?;
            let launcher = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/launch.vbs");
            let command = startup_command(&exe, &launcher);
            let data: Vec<u8> = command
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            RegSetValueExW(key, w!("Gavnel"), None, REG_SZ, Some(&data)).ok()
        } else {
            let code = RegDeleteValueW(key, w!("Gavnel"));
            if code == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                code.ok()
            }
        };
        let _ = RegCloseKey(key);
        result?;
        Ok(())
    }
}

fn startup_command(exe: &Path, launcher: &Path) -> String {
    if launcher.is_file()
        && launcher
            .parent()
            .and_then(Path::parent)
            .is_some_and(|root| exe.starts_with(root.join("target")))
    {
        let windows = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"));
        format!(
            "\"{}\" //B //Nologo \"{}\" --background",
            windows.join("System32/wscript.exe").display(),
            launcher.display()
        )
    } else {
        format!("\"{}\" --background", exe.display())
    }
}

pub fn save_video(initial: &Path) -> Result<Option<std::path::PathBuf>> {
    save_file(initial, "mp4")
}

fn save_file(initial: &Path, extension: &str) -> Result<Option<std::path::PathBuf>> {
    use windows::Win32::{
        System::Com::*,
        UI::{Input::KeyboardAndMouse::GetActiveWindow, Shell::*},
    };
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR | FOS_OVERWRITEPROMPT,
        )?;
        let name: Vec<u16> = initial
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        dialog.SetFileName(PCWSTR(name.as_ptr()))?;
        let extension: Vec<u16> = extension.encode_utf16().chain(Some(0)).collect();
        dialog.SetDefaultExtension(PCWSTR(extension.as_ptr()))?;
        if let Some(parent) = initial.parent().filter(|p| p.is_dir()) {
            let folder: Vec<u16> = parent
                .as_os_str()
                .to_string_lossy()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(folder.as_ptr()), None)?;
            dialog.SetFolder(&item)?;
        }
        match dialog.Show(Some(GetActiveWindow())) {
            Err(e) if e.code() == windows::core::HRESULT::from_win32(1223) => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(()) => {}
        }
        let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = name.to_string();
        CoTaskMemFree(Some(name.0 as _));
        Ok(Some(std::path::PathBuf::from(path?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_startup_uses_source_launcher_when_available_and_executable_when_packaged() {
        let dir = tempfile::tempdir().unwrap();
        let launcher = dir.path().join("scripts/launch.vbs");
        std::fs::create_dir_all(launcher.parent().unwrap()).unwrap();
        let exe = dir.path().join("target/release/Gavnel.exe");
        assert_eq!(
            startup_command(&exe, &launcher),
            format!("\"{}\" --background", exe.display())
        );
        std::fs::write(&launcher, b"test launcher").unwrap();
        let command = startup_command(&exe, &launcher);
        assert!(command.contains("wscript.exe"));
        assert!(command.contains(&format!("\"{}\"", launcher.display())));
        assert!(command.ends_with("--background"));
        assert!(!command.contains("Gavnel.exe"));
    }
    #[test]
    fn hidden_window_reopens_and_exit_cannot_be_reversed_by_late_tray_clicks() {
        use eframe::egui::{Context, ViewportCommand, ViewportId};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let ctx = Context::default();
        let visible = Arc::new(AtomicBool::new(false));
        let quitting = Arc::new(AtomicBool::new(false));
        // An invalid native handle keeps this state/dispatch regression test off the desktop.
        let actions = WindowActions::new(
            HWND::default(),
            ctx.clone(),
            visible.clone(),
            quitting.clone(),
        );
        actions.show();
        let mut output = ctx.run_pass(Default::default(), |_| {});
        output.textures_delta.clear();
        let commands = &output.viewport_output[&ViewportId::ROOT].commands;
        assert!(visible.load(Ordering::Acquire));
        assert!(commands.contains(&ViewportCommand::Visible(true)));
        assert!(commands.contains(&ViewportCommand::Minimized(false)));
        assert!(commands.contains(&ViewportCommand::Focus));
        actions.hide();
        assert!(!visible.load(Ordering::Acquire));
        ctx.run_pass(Default::default(), |_| {})
            .drop_without_applying_deltas();
        actions.exit();
        actions.hide();
        actions.show();
        let mut output = ctx.run_pass(Default::default(), |_| {});
        output.textures_delta.clear();
        let commands = &output.viewport_output[&ViewportId::ROOT].commands;
        assert!(quitting.load(Ordering::Acquire));
        assert!(visible.load(Ordering::Acquire));
        assert!(commands.contains(&ViewportCommand::Close));
        assert!(!commands.contains(&ViewportCommand::Visible(false)));
        assert!(!commands.contains(&ViewportCommand::Focus));
    }
    #[test]
    fn active_video_save_blocks_tray_and_launcher_exit_until_complete() {
        use eframe::egui::{Context, ViewportCommand, ViewportId};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let ctx = Context::default();
        let quitting = Arc::new(AtomicBool::new(false));
        let actions = WindowActions::new(
            HWND::default(),
            ctx.clone(),
            Arc::new(AtomicBool::new(false)),
            quitting.clone(),
        );
        actions.set_exit_blocked(true);
        actions.clone().exit();
        let mut output = ctx.run_pass(Default::default(), |_| {});
        output.textures_delta.clear();
        assert!(!quitting.load(Ordering::Acquire));
        assert!(
            !output.viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::Close)
        );
        actions.set_exit_blocked(false);
        actions.exit();
        let mut output = ctx.run_pass(Default::default(), |_| {});
        output.textures_delta.clear();
        assert!(quitting.load(Ordering::Acquire));
        assert!(
            output.viewport_output[&ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::Close)
        );
    }

    #[test]
    fn folder_validation_checks_writability_without_leaving_files() {
        let folder = tempfile::tempdir().unwrap();
        validate_clip_folder(folder.path()).unwrap();
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 0);
        let file = folder.path().join("not-a-folder");
        std::fs::write(&file, b"test").unwrap();
        assert!(validate_clip_folder(&file).is_err());
        assert!(validate_clip_folder(Path::new("relative-clips")).is_err());
    }
}
