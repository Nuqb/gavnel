# Development and diagnostics

[← Back to the README](../README.md)

## Before making changes

Read [AGENTS.md](../AGENTS.md) and [software-quality.md](software-quality.md) before planning or editing. This is a native Rust/egui Windows app using local files, not a server-backed application. Match its existing module boundaries and verify behavior after refactoring.

## Requirements

- Stable Rust with the `x86_64-pc-windows-msvc` toolchain.
- Visual Studio C++ Build Tools and the Windows SDK.
- Windows 11 x64, an SDR source, and an NVIDIA GPU with a compatible NVENC driver for live recording.

Building dependencies may require internet access. Running the recorder does not require a login or internet connection. NVENC loads from the installed NVIDIA driver; no separate FFmpeg installation is required.

## Build and run locally

Use the source launcher for local development:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1
```

It builds the current source into a separate output directory and runs an immutable copy identified by its hash. Later launches use incremental builds. The existing desktop and Start menu shortcuts use this launcher; do not replace them with direct executable links or copy binaries into `dist` for local development.

When a different build is running, the launcher requests a normal exit and waits for recordings to finish saving. It never force-stops the recorder. Build errors leave the current app running and are recorded in `work/launcher.log`.

To create launcher shortcuts:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-launcher.ps1
```

## Checks

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly
```

Run `cargo fmt` after code edits. The build-only launcher checks the runnable release build without opening the app. Some graphics and visual-export tests are opt-in; read their descriptions before running them.

## Source map

- `src/engine/` — recording lifecycle, command transitions, and the bounded background save queue.
- `src/audio/`, `src/video/`, `src/sources/` — audio capture/encoding, NVIDIA encoding, and Windows capture sources.
- `src/ui/` — library, playback, source selection, settings pages, shared controls, and native window integration.
- `src/replay.rs`, `src/recording.rs`, `src/export.rs` — retained packets, streamed sessions, and MP4 output.
- `src/config.rs`, `src/library.rs`, `src/storage.rs` — preference persistence, local file operations, and storage limits.
- `src/diagnostics/` — explicit recording diagnostics, separated from normal desktop startup.

## Diagnostics

Build a debug executable with `cargo build --locked`, then use the appropriate explicit command:

```powershell
.\target\debug\gavnel.exe --settings-info
.\target\debug\gavnel.exe --probe
.\target\debug\gavnel.exe --audio-formats
.\target\debug\gavnel.exe --verify 'path\to\clip.mp4'
.\target\debug\gavnel.exe --audio-check 'path\to\clip.mp4'
```

`--settings-info` reports the loaded configuration location without opening the app or starting capture. `--audio-check` decodes saved audio for analysis without playing it. `--probe` lists capture/encoder/device information.

The following commands **make new recordings**. Choose an appropriate local output folder and make sure the captured screen/audio is suitable for a test:

```powershell
.\target\debug\gavnel.exe --self-test --seconds 8 --output work\test-clips
.\target\debug\gavnel.exe --self-test --no-audio --seconds 8 --output work\video-only
.\target\debug\gavnel.exe --self-test --full-recording --seconds 70 --replay-seconds 5 --also-clip --output work\test-recordings
.\target\debug\gavnel.exe --pc-audio-diagnostic work\pc-audio-check
```

`--self-test` captures the primary display and PC audio unless `--no-audio` is supplied. The PC-audio diagnostic records playback devices without a microphone or video. Generated diagnostics, previews, logs, and local build outputs belong under ignored output directories.

## License

Gavnel is [MIT licensed](../LICENSE). See [third-party notices](../THIRD_PARTY_NOTICES.md) for bundled dependency licenses.
