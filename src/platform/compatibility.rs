//! Query the real OS build without depending on executable compatibility manifests.
use std::sync::OnceLock;
use windows::{
    Win32::System::{
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        SystemInformation::OSVERSIONINFOW,
    },
    core::{s, w},
};

pub fn application_audio_supported() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| windows_build().is_some_and(process_loopback_supported))
}

pub fn capture_border_suppression_supported() -> bool {
    // Border suppression and process loopback both arrived at build 20348.
    application_audio_supported()
}

fn process_loopback_supported(build: u32) -> bool {
    // Process-filtered WASAPI loopback starts at build 20348. Windows 10
    // desktop releases, including 22H2 (19045), do not provide it.
    build >= 20348
}

fn windows_build() -> Option<u32> {
    unsafe {
        let module = GetModuleHandleW(w!("ntdll.dll")).ok()?;
        let address = GetProcAddress(module, s!("RtlGetVersion"))?;
        // RtlGetVersion takes an OSVERSIONINFOW pointer and returns NTSTATUS.
        let version_fn: unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32 =
            std::mem::transmute(address);
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        (version_fn(&mut version) >= 0).then_some(version.dwBuildNumber)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_audio_requires_the_api_introduction_build() {
        for build in [18362, 19041, 19044, 19045, 20347] {
            assert!(!process_loopback_supported(build));
        }
        for build in [20348, 22000, 22621, 26100] {
            assert!(process_loopback_supported(build));
        }
    }

    #[test]
    fn real_windows_build_is_readable() {
        assert!(windows_build().is_some_and(|build| build >= 19045));
    }
}
