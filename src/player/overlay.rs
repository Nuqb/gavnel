//! Transparent child controls move and animate with the main player window.
use anyhow::Result;
use eframe::egui;
use windows::{
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::{
            Dwm::{
                DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_NCRENDERING_POLICY,
                DWMWA_TRANSITIONS_FORCEDISABLED, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
                DwmSetWindowAttribute,
            },
            Gdi::{ClientToScreen, ScreenToClient},
        },
        System::Threading::GetCurrentProcessId,
        UI::WindowsAndMessaging::*,
    },
    core::{PCWSTR, w},
};

pub(crate) const TITLE: &str = "Gavnel video controls";

pub(crate) fn screen_rect(parent: HWND, rect: egui::Rect, pixels: f32) -> egui::Rect {
    let mut origin = POINT::default();
    unsafe {
        let _ = ClientToScreen(parent, &mut origin);
    }
    egui::Rect::from_min_size(
        egui::pos2(origin.x as f32, origin.y as f32) + rect.min.to_vec2() * pixels,
        rect.size() * pixels,
    )
}

pub(crate) fn pointer(parent: HWND, pixels: f32) -> Option<egui::Pos2> {
    unsafe {
        let mut position = POINT::default();
        GetCursorPos(&mut position).ok()?;
        let hovered = WindowFromPoint(position);
        let owned = GetWindowLongPtrW(hovered, GWLP_HWNDPARENT) == parent.0 as isize;
        if hovered != parent && !IsChild(parent, hovered).as_bool() && !owned {
            return None;
        }
        if !ScreenToClient(parent, &mut position).as_bool() {
            return None;
        }
        Some(egui::pos2(
            position.x as f32 / pixels,
            position.y as f32 / pixels,
        ))
    }
}

pub(crate) fn window(parent: HWND) -> Option<HWND> {
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    let mut after = None;
    unsafe {
        if let Ok(child) = FindWindowExW(Some(parent), None, None, PCWSTR(title.as_ptr())) {
            return Some(child);
        }
        loop {
            let window = FindWindowExW(None, after, None, PCWSTR(title.as_ptr())).ok()?;
            let mut process = 0;
            GetWindowThreadProcessId(window, Some(&mut process));
            if process == GetCurrentProcessId() {
                return Some(window);
            }
            after = Some(window);
        }
    }
}

pub(crate) fn attach(parent: HWND, rect: egui::Rect, show: bool) -> Result<()> {
    prepare(parent, rect)?;
    let Some(window) = window(parent) else {
        return Ok(());
    };
    unsafe {
        let visible = show && IsWindowVisible(parent).as_bool() && !IsIconic(parent).as_bool();
        if visible == IsWindowVisible(window).as_bool() {
            return Ok(());
        }
        SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE
                | SWP_NOZORDER
                | SWP_NOMOVE
                | SWP_NOSIZE
                | if visible {
                    SWP_SHOWWINDOW
                } else {
                    SWP_HIDEWINDOW
                },
        )?;
    }
    Ok(())
}

/// Configure the overlay before its first paint, without changing visibility.
pub(crate) fn prepare(parent: HWND, rect: egui::Rect) -> Result<()> {
    let Some(window) = window(parent) else {
        return Ok(());
    };
    unsafe {
        remove_window_frame(window)?;
        attach_child(window, parent)?;
        let mut origin = POINT {
            x: rect.left().round() as i32,
            y: rect.top().round() as i32,
        };
        ScreenToClient(parent, &mut origin).ok()?;
        SetWindowPos(
            window,
            Some(HWND_TOP),
            origin.x,
            origin.y,
            rect.width().round().max(1.0) as i32,
            rect.height().round().max(1.0) as i32,
            SWP_NOACTIVATE,
        )?;
    }
    Ok(())
}

unsafe fn attach_child(window: HWND, parent: HWND) -> Result<()> {
    unsafe {
        if IsChild(parent, window).as_bool() {
            return Ok(());
        }
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        SetWindowLongPtrW(
            window,
            GWL_STYLE,
            ((style & !(WS_POPUP | WS_CAPTION | WS_THICKFRAME).0) | (WS_CHILD | WS_CLIPSIBLINGS).0)
                as isize,
        );
        // A successful SetParent can return NULL when the previous parent was the desktop.
        let result = SetParent(window, Some(parent));
        if !IsChild(parent, window).as_bool() {
            result?;
            anyhow::bail!("Player controls could not attach to their window.");
        }
    }
    Ok(())
}

/// A transparent control overlay must not acquire a DWM outline, shadow, or rounded frame.
unsafe fn remove_window_frame(window: HWND) -> Result<()> {
    if !unsafe { GetPropW(window, w!("Gavnel.OverlayWindowProc")) }.is_invalid() {
        return Ok(());
    }
    unsafe {
        let disabled = 1_u32;
        DwmSetWindowAttribute(
            window,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            std::ptr::from_ref(&disabled).cast(),
            std::mem::size_of_val(&disabled) as u32,
        )?;
        let previous = GetWindowLongPtrW(window, GWLP_WNDPROC);
        SetPropW(
            window,
            w!("Gavnel.OverlayWindowProc"),
            Some(HANDLE(previous as _)),
        )?;
        SetWindowLongPtrW(window, GWLP_WNDPROC, frameless_proc as *const () as isize);
    }

    // The color/corner attributes are optional on Windows versions before Windows 11.
    for (attribute, value) in [
        (DWMWA_NCRENDERING_POLICY, DWMNCRP_DISABLED.0 as u32),
        (DWMWA_BORDER_COLOR, 0xffff_fffe_u32), // DWMWA_COLOR_NONE
        (DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND.0 as u32),
    ] {
        unsafe {
            let _ = DwmSetWindowAttribute(
                window,
                attribute,
                std::ptr::from_ref(&value).cast(),
                std::mem::size_of_val(&value) as u32,
            );
        }
    }
    unsafe {
        SetWindowPos(
            window,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER | SWP_FRAMECHANGED,
        )?;
    }
    Ok(())
}

/// Winit reserves a top pixel for undecorated-window shadows, even with DWM borders disabled.
/// This non-resizable overlay needs its client area to fill the entire window.
unsafe extern "system" fn frameless_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCALCSIZE {
        return LRESULT(0);
    }
    unsafe {
        let previous = GetPropW(window, w!("Gavnel.OverlayWindowProc"));
        if previous.is_invalid() {
            return DefWindowProcW(window, message, wparam, lparam);
        }
        // The property stores the WNDPROC returned by this window's GWLP_WNDPROC.
        let procedure = std::mem::transmute::<isize, WNDPROC>(previous.0 as isize);
        if message == WM_NCDESTROY {
            SetWindowLongPtrW(window, GWLP_WNDPROC, previous.0 as isize);
            let _ = RemovePropW(window, w!("Gavnel.OverlayWindowProc"));
        }
        CallWindowProcW(procedure, window, message, wparam, lparam)
    }
}
