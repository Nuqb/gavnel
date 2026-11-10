#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod audio_check;
mod bookmarks;
mod branding;
mod capture_hotkeys;
mod config;
mod diagnostics;
mod engine;
mod export;
mod feedback;
mod i18n;
mod icons;
mod library;
mod microphone;
mod motion_test;
mod pacing;
mod platform;
mod player;
mod recording;
mod replay;
mod screenshot;
mod sources;
mod storage;
mod thumbnails;
mod typography;
mod ui;
mod update;
mod verify;
mod video;
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("Gavnel {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if update::early(&args)? {
        return Ok(());
    }

    if args.iter().any(|arg| arg == "--onboarding-preview") {
        return ui::onboarding::preview::run();
    }
    if diagnostics::early(&args)? {
        return Ok(());
    }
    let diagnostic = args.iter().any(|a| {
        a == "--probe"
            || a == "--self-test"
            || a == "--verify"
            || a == "--audio-check"
            || a == "--audio-formats"
            || a == "--audio-diagnostic"
            || a == "--pc-audio-diagnostic"
            || a == "--audio-reencode"
    });
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            if diagnostic {
                windows::Win32::System::Com::COINIT_MULTITHREADED
            } else {
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED
            },
        );
    }
    if diagnostics::run(&args)? {
        return Ok(());
    }
    let background = args.iter().any(|a| a == "--background");
    let lifecycle_test = args.iter().any(|a| a == "--lifecycle-test");
    let lifecycle_worker = std::sync::Arc::new(std::sync::Mutex::new(None));
    let test_worker = lifecycle_worker.clone();
    let Some(_instance) = platform::AppInstance::acquire()? else {
        return Ok(());
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Gavnel")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([1000.0, 650.0])
            .with_visible(!background)
            .with_icon(eframe::egui::IconData {
                rgba: ui::icon_rgba(),
                width: 32,
                height: 32,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "Gavnel",
        options,
        Box::new(move |cc| {
            let app = ui::Gavnel::new(cc, background, lifecycle_test)?;
            if lifecycle_test {
                *test_worker.lock().unwrap() = Some(app.verify_lifecycle());
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Gavnel could not open: {e}"))?;
    if let Some(worker) = lifecycle_worker.lock().unwrap().take() {
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("Lifecycle test worker failed"))??;
        println!("Lifecycle: hidden Exit returned cleanly; recording engine and tray were dropped");
    }
    Ok(())
}
