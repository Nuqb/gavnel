# Development and diagnostics

[← Back to the README](../README.md)

## Before making changes

Read [the software-quality guide](software-quality.md) before planning or editing. This is a native Rust/egui Windows app using local files, not a server-backed application. Match its existing module boundaries and verify behavior after refactoring.

For requests to push an update, follow the [release runbook](releases.md).
Prepare the version and notes, commit on `main`, then use
`scripts/release.ps1 -Version X.Y.Z`. Completion includes the published stable GitHub release;
`-Resume` continues transient failures. A source-only push must be explicitly requested.

## Requirements

- Stable Rust with the `x86_64-pc-windows-msvc` toolchain.
- Visual Studio C++ Build Tools and the Windows SDK.
- Windows 10 22H2 (build 19045) or Windows 11 x64, an SDR source, and an NVIDIA GPU with a compatible NVENC driver for live recording.

Building dependencies may require internet access. Running the recorder does not require a login or internet connection. NVENC loads from the installed NVIDIA driver. The editor uses Windows Media Foundation; building and launching Gavnel does not download an external media runtime.

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
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly
```

Run `cargo fmt` after code edits. The build-only launcher checks the runnable release build without opening the app. Some graphics and visual-export tests are opt-in; read their descriptions before running them.

## Windows compatibility checks

The installer accepts Windows 10 22H2 x64 (build 19045) and Windows 11 x64.
Windows 10 live validation is pending; a build or CI run on Windows Server does
not establish recording compatibility on either desktop OS.

Before claiming a release is validated on both systems, run these checks on
each OS with an SDR display and a supported NVIDIA GPU/driver:

- Install, launch, upgrade, and uninstall using the release installer.
- Record a monitor and a window, save a replay, and take a screenshot.
- Record all PC audio, selected playback devices, and the microphone; verify
  playback and separate audio tracks.
- On Windows 10, confirm Game only and Selected apps are disabled. Import
  settings requesting either mode and confirm recording reports an error with
  the supported alternatives, without replacing an active recording or silently
  capturing other applications' audio.
- On Windows 11, verify game-only and selected-app audio still isolate audio.
- Exercise playback, seeking, fullscreen, trims, and native H.264/AAC exports.
- Exercise global shortcuts, tray restore, close-to-tray, and graceful updates.

Windows 10 may display the system capture border. Border suppression, capture
update-interval tuning, and Windows 11 window styling are optional; unavailable
properties must not prevent recording or playback. Process-filtered audio needs
build 20348 or newer, which standard Windows 10 desktop releases do not provide
([Microsoft API requirements](https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ns-audioclientactivationparams-audioclient_process_loopback_params)).

## Source map

- `src/engine/` — recording lifecycle, command transitions, and the bounded background save queue.
- `src/audio/`, `src/video/`, `src/sources/` — audio capture/encoding, NVIDIA encoding, and Windows capture sources.
- `src/ui/` — library, playback, source selection, settings pages, shared controls, and native window integration.
- `src/replay.rs`, `src/recording.rs`, `src/export.rs` — retained packets, streamed sessions, and MP4 output.
- `src/media/` — Windows-native video inspection, frame decoding, audio mixing, and editor exports, separate from the recording pipeline.
- `src/config.rs`, `src/library.rs`, `src/storage.rs` — preference persistence, local file operations, and storage limits.
- `src/diagnostics/` — explicit recording diagnostics, separated from normal desktop startup.

## Diagnostics

### Test onboarding independently

Use **Settings → General → Setup → Preview setup**, or run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -OnboardingPreview
```

This builds and opens an isolated setup window alongside a running recorder. It
does not request that recorder to exit, register recording hotkeys, save preferences,
change startup registration, or record the screen. Finish and Skip both reach a
preview-complete screen with a **Try setup again** button. Actual setup is available
through **Run setup again**; there is no need to delete configuration files.

First-run detection and completion, draft isolation, shortcut conflicts and replay
length, navigation, and the minimum window size have automated regression checks.
To export all six screens with generated test devices and no app window:

```powershell
cargo test --locked export_onboarding_previews -- --ignored
```

The images are written under `work/onboarding/`. The preview command is also
available directly as `gavnel.exe --onboarding-preview`.

The quality step lists example GPU/CPU/RAM/SSD pairings as conservative
starting points for gaming and recording together. They are advisory estimates,
not benchmarked Gavnel minimums or a guarantee for every game. Lower-spec PCs may
work; game load, capture resolution, driver support, and GPU headroom matter.
The NVIDIA GPU examples support H.264 NVENC; NVIDIA documents the encoder's
capabilities and indicative performance in its [NVENC application note](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-application-note/index.html).
CPU and RAM pairings are product guidance, not requirements from NVIDIA. Each
preset offers Intel and AMD CPU examples with the same NVIDIA GPU. AMD Radeon
and Intel GPU encoding are not implemented and must not be listed as compatible
onboarding hardware.

### Recorder diagnostics

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

Gavnel uses a [source-available license](../LICENSE). You may inspect, build, and privately modify Gavnel for your own personal or internal business use. Redistribution, resale, rebranding, and reuse of its code or original assets in other products require prior written permission. Use of Gavnel for monetized content is allowed, and the license does not restrict your recordings or screenshots.

The license change applies to new material first distributed under the new terms. Previously MIT-licensed portions and existing release tags through v0.3.4 retain their MIT permissions; the original text is preserved in [HISTORICAL-MIT.txt](../HISTORICAL-MIT.txt). Public GitHub viewing and forking permissions and applicable legal exceptions are preserved. [Third-party components](../THIRD_PARTY_NOTICES.md) retain their own licenses.

Cargo points to the custom license file and disables registry publishing. Release packaging includes both the current license and the historical MIT text; the installer displays the current license. Before publishing under these terms, have the license reviewed by qualified legal counsel and verify authority to license any newly contributed material.

## Localization

Read [the localization guide](localization.md) before adding or changing user-facing
text. Bundled catalogs live under `assets/locales`; use whole-message translations
and review regional variants. Run `python scripts/check-locales.py` as well as the
Rust checks. Native-language review status is tracked in the guide.

The UI requires Rust 1.95 or newer. The text renderer and companion egui crates
are pinned to revision `87476c112a338f8940c64979dc6b46b90612e195` of the
[upstream RTL proposal](https://github.com/emilk/egui/pull/8818). Keep them aligned;
mixing registry and patched companion crates breaks their shared APIs. This is
an unreleased fix, approved for this project; review replacement revisions before
updating. Local verification passed 83 epaint tests and 11 text-selection tests,
plus the app's mixed Hebrew/Latin/numeral regression. These checks are not a
substitute for native RTL text-input testing.

Bundled font source URLs and hashes are in `assets/fonts/manifest.json`; retain
each OFL license on redistribution. Translation generation tools and model caches
are development-only ignored files under `work/`, never runtime dependencies.

When packaging a binary, include `THIRD_PARTY_NOTICES.md` and the accompanying
`assets/fonts/*-OFL.txt` and `assets/fonts/*-LICENSE.txt` files. The fonts are
embedded in the executable; the notices must still accompany redistribution.

## Editing and highlight verification

The editor uses Windows Media Foundation in-process for metadata, timeline
frames, previews, audio mixing, and H.264/AAC exports. Neither the source launcher
nor the installer includes external editing tools. Export jobs run separately
from recording and media-inspection workers. Projects and labeled event/audio
sidecars are local files; there is no database or remote service.

The normal test suite includes native editing, source-preservation, crop/trim,
audio selection, marker mapping, and decoder-padding checks. An installed app
can run the same isolated smoke path with `Gavnel.exe --editor-self-test`.
Native export uses Windows encoding and can have different performance from the
recorder's NVENC path. No capture device or audio playback is used by these tests.

The following commands exercise native media, optional OCR, and process audio:

```powershell
cargo test --locked native_editing_preserves_sources_mixes_audio_and_maps_markers
cargo test --locked windows_ocr_recognizes_synthetic_highlight -- --ignored
cargo test --locked process_loopback_isolates_selected_application -- --ignored
cargo test --locked export_editor_previews -- --ignored
```

The optional OCR and process-audio fixtures need `python scripts/get-media.py`
to prepare their FFmpeg image generator and FFplay tone players. These reference
tools are not part of Gavnel and are not needed by the normal test suite.
The process-audio test plays two quiet synthetic tones and captures only its
selected child process. The other tests do not record the desktop or microphone.
Game-event fixtures cover CS2 authentication, duplicates, spectators, transitions,
and listener cleanup. Visual detectors are experimental; synthetic OCR coverage
is not a measurement of live Fortnite, Valorant, or Rust accuracy. See
[the hands-on checklist](editor-testing.md).
