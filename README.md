# Gavnel

A Windows gameplay replay clipper written in Rust, with NVIDIA hardware encoding and an orange, library-first desktop interface.

## Development quality

Before planning or changing this project, read [AGENTS.md](AGENTS.md) and the
[software quality guide](docs/software-quality.md). Keep responsibilities focused,
review structural warnings in context, and verify changes with formatting, tests,
and the source launcher's build command documented in AGENTS.md.

## Run

Open `Gavnel.exe`, select a game, monitor, or window from the recording control at the top, and click **Start clipping**. Press **F8** to save the last 60 seconds. Automatic game detection supports Fortnite, Valorant, Rust, Counter-Strike 2, and custom executable profiles.

Open **Settings → Recording → Recording Mode** and choose **Manual capture**, **Automatically record entire game session**, or **Recording off**. Mode changes apply immediately. The toolbar recorder dropdown provides a shortcut to this section. Click its capture-status card to open the Game/Desktop picker. The waiting-for-game card opens a refreshable running-program selector. Desktop displays have local preview cards; confirm to apply the source and start capture if it is stopped.

For a long recording, use **Manual capture**, choose a game, monitor, or window, then press **Alt+F7** or use the toolbar's **Long recording** button. Press it again to save and return to replay capture. Gavnel writes the entire session to one MP4 with an elapsed timer. The saved recording appears in your Library. The clip key adds a bookmark while a long recording is active; the dedicated **F4** shortcut also adds bookmarks. Bookmarks appear as clickable timestamps and timeline markers in Gavnel's player.

**Settings → Recording → Recording Hotkeys** contains Clip, Start/stop long recording, Bookmark (long recording), and Screenshot rows. Defaults are **F8**, **Alt+F7**, **F4**, and **F7**. Existing shortcuts are preserved; new defaults avoid existing bindings. Clicking a binding captures the next key combination; Escape cancels. **Add Hotkey** adds another binding for any action, including additional clip lengths. Each row has a **Tap / Long press** selector and a three-dot menu to **Unset hotkey** or **Remove hotkey**. Unset leaves the row unbound; Remove deletes the row. Long press activates once after 500 ms. Tap and Long press can share a key; a short release triggers Tap and a hold triggers only Long press. Duplicate key/trigger combinations are rejected. Clip lengths share one replay buffer, and changing them does not stop an active recording. Shortcuts work in the tray and do nothing in Recording off mode. Stopping an automatic session with the long-recording shortcut pauses automatic capture until enabled again. Older Full recording mode preferences migrate to Manual capture.

Every action keeps at least one hotkey row. **Remove hotkey** appears only when a second row for that same action exists; **Unset hotkey** can still clear the final row's key assignment. Missing action rows from older settings return unbound, preserving cleared shortcuts.

Screenshots capture the selected game, window, or display at its original resolution as a PNG in that source's recording folder. Capture can already be running, or Gavnel briefly opens the selected source for a standalone screenshot. A selected game/window must be in the foreground. PNG writing uses the background save queue and respects the storage limit. Screenshots appear in the Library with a **PNG** badge and open in the default image viewer. Pending screenshot writes finish during normal app exit.

**Automatically record entire game session** enables automatic game detection, starts recording when a supported game opens, saves when it closes, and waits for the next game. The mode and capture on/off state are remembered when Gavnel exits. **Manual capture** keeps recent gameplay available for the clip hotkey once clipping is started. **Recording off** stops capture and remains off after restarting.

Long recording has no configured duration limit and streams compressed video/audio to disk on a separate writer thread. Replay retention stays bounded and may shorten at high bitrates to leave recording headroom; it does not limit the recording's length. Allow about 9 GB per hour at the default 20 Mbps, plus audio. Closing to the tray continues capture; **Exit** finishes and saves the recording. The tray menu also offers **Start/stop long recording** and **Stop capture / save recording**. Both normal launch and Windows startup restore capture to its last saved on/off state. A manual long recording returns to the saved capture mode rather than continuing the previous file.

Switching recording modes finishes any current full recording. Changing the capture source, quality, or audio while recording saves the current file and starts another. A folder change affects future recordings and new replay clips; the current full recording finishes in its original folder. Losing the capture source, a required backend change, or a capture/storage error ends the current file and attempts to save received frames. Automatic sessions then wait or retry; manual Full recording stops. An unfinished `.mp4.partial` file is preserved if writing or finalization fails; abrupt termination or power loss can leave that file unplayable.

The Library is the home screen. Search clips, filter by game, sort them, switch card sizes, and click a thumbnail to open a clip in your default video player. Thumbnails are loaded on a background worker using the Windows thumbnail cache. No preview video plays continuously.

The top bar keeps capture status, the save key, clip duration, recording quality, microphone settings, and start/stop/save controls available throughout the app. The narrow left rail opens Library, Capture, Game Detection, the clip folder, and Settings. Settings are divided into Recording, Audio, Hotkeys & Storage, and Game Detection.

Click the recorder status at the top for quick controls: recording on/off, game/window/monitor selection, quality, microphone device, PC audio, clip folder, and full recorder settings. Quality and audio have their own compact panels.

Choose **Settings → Hotkeys & Storage → Browse…** to pick a clips folder with the Windows folder picker. The folder icon in the top bar and the **Clip folder** row in quick settings open the same picker. Folder changes take effect immediately without restarting capture or clearing the replay buffer. A save already queued finishes in its original folder. Existing clips are never moved automatically. You can also type a path and click **Use this folder**. Invalid or unwritable destinations leave the active folder unchanged.

The storage destination belongs to your Windows user profile, not the executable's folder or app version. All builds share `%APPDATA%\Gavnel\Gavnel\config\settings.json`. A separate version-independent `storage.json` protects the folder choice if other settings become unreadable or incompatible; previous settings are backed up when saving. Existing settings from older builds are read automatically. `--settings-info` reports the version and loaded storage location without opening the app, changing settings, or starting capture.

New clips and full recordings are stored in game/source subfolders beneath that destination: for example `Gavnel\Fortnite\...mp4`, `Gavnel\Counter-Strike 2\...mp4`, and `Gavnel\Desktop\...mp4`. Detected games use their configured profile names; other window captures use the executable's app name, so changing window or match titles do not create a new folder. Monitor captures share the Desktop folder and retain their display number in the filename. The Library searches both nested folders and older loose clips.

**Settings → Hotkeys & storage → PC storage → Organize existing clips** moves recognized older Gavnel recordings from the root into source folders in the background. Files already in folders are left alone; existing destinations are never replaced, and locked files or unrelated filenames remain in place. This is an explicit action; no older files move just because you update or open the app.

Both clip menus show **Delete clip** in red without trailing dots. Deletion opens a centered modal with a dimmed backdrop, recording details, Cancel, and a distinct red confirmation button. Escape, the close icon, or clicking outside cancels. Background controls are blocked, and a failed deletion leaves the dialog open with an error.

The top bar has one combined keybind/clip-duration button with a square keycap. Click either part to open **Clips**, capture a new hotkey, or choose the replay duration. **Manage Hotkeys** opens the General settings page. Escape cancels shortcut capture. Changes save automatically. Resolution and FPS remain in the recorder dropdown under **Quality**.

**Settings → General → Clip feedback** has separate sound and notification switches, a sound-volume slider, and **Preview clip feedback**. By default a short original two-note cue plays after a clip request is accepted, and a compact orange confirmation fades in and out over three seconds. It updates from **Clip captured** to **Clip saved** when writing completes; rejected requests and save failures show an error without a success sound. Repeated clips reuse the popup rather than stacking windows. Preview uses the current controls without creating a clip; preferences save automatically without resetting capture.

The confirmation runs independently while Gavnel is in the tray. Its native window is topmost, non-activating, absent from Alt+Tab, and transparent to clicks. It appears at the bottom-right of the foreground app's monitor, never requests foreground focus, and is excluded from Windows screen capture where supported. It uses no game injection. True exclusive fullscreen can hide a desktop popup; use borderless fullscreen for visibility. The sound remains available in exclusive fullscreen. Like other system audio, earlier confirmation sounds may be included in subsequent replay clips.

**Play clip** and thumbnail clicks open the embedded Windows media player inside Gavnel. It streams the clip with synchronized audio and video, and supports play/pause, seeking, mute, and volume. Space toggles playback, arrow keys seek five seconds, and Escape returns to the library. Closing to the tray pauses playback. An explicit **Open externally** action remains available.

Under **Settings → Audio → Microphone Settings**, choose the input, start a microphone test, choose **End Test** to keep the sample or **Playback** to listen immediately, and use **Stop Playback** to stop listening (samples can be replayed; tests stop recording after 60 seconds), set the noise threshold, assign a push-to-talk key, or enable **Mono Audio Input**. Mono uses the previously saved input channel (left by default) and duplicates it into both ears. Desktop audio remains stereo. Settings save automatically after editing, including on exit. Click a slider value to enter a percentage or dB threshold.

The dedicated **Storage** tab contains the capture folder, optional local storage limit, and actual drive usage. Storage limits stop new writes without deleting existing recordings; active full recordings are finalized when the limit is detected. Recording hotkeys are in **Recording**; feedback, library management, and startup controls are in **General**. Preferences use the same Windows user configuration folder across builds, retain unknown fields, and recover compatible settings independently if a field cannot be loaded.

Under **Recording Audio**, enable microphone and PC audio independently, set their volumes, and choose all playback devices or individual outputs. **Separate Audio Tracks** saves a combined track plus independent PC and microphone tracks in clips and full recordings. Playback devices refresh every two seconds and reconnect independently. Hardware or software mirroring audio to multiple selected outputs can duplicate sound in the mix.

**Input Sensitivity** sets the mic activation threshold, with smooth fades and a short hold to preserve word endings. Push to Talk bypasses that threshold. **Noise Suppression** is separate: **None** keeps the original sound; **Standard** uses spectral filtering for steady noise; **AI (RNNoise)** uses an on-device neural model for speech. Existing threshold preferences are kept as Input Sensitivity, with suppression initially set to None. Recording and Mic Test use the same filters before mic volume, and PC audio stays unchanged. Filters compensate their 20 ms delay using the existing audio buffer. Changing microphone settings clears the previous test so playback cannot silently use an old filter; start a fresh Mic Test to compare settings. The input level meter shows the incoming signal before filtering, so it can still move when noise suppression or sensitivity removes that sound.

Audio packet placement uses consecutive device-frame positions, anchored to the shared recording clock. Rounding each packet's timestamp separately could leave one-sample holes or overlap samples and introduce crackle into either PC audio or the microphone. Real missing-device-frame gaps remain silent, and a device reset cannot mix new packets over the previous packet's time range. An offline Windows AAC round-trip test checks generated silence and stereo tones without accessing capture devices or playing sound.

Device-position counters are converted from the device's native sample rate before placing packets on the 48-kHz recording timeline. This matters for interfaces running at 44.1 kHz and output devices running at 96 kHz even when Windows supplies resampled 48-kHz buffers. The microphone and desktop retain independent timing anchors.

Some drivers return an unflagged QPC timestamp far in the future or in a different clock epoch. These streams use a bounded arrival-time anchor while consecutive device positions preserve smooth audio. PCM retention is bounded by two seconds of samples rather than 32 packets, so short packets are not discarded before the mixer's 80 ms delay. Regression tests reproduce the observed missing-audio failure. A live PC-only test on this machine verified continuous playback audio through mixing and AAC encoding with no clipped samples; no microphone or video was captured in that test.

Capture uses a private Windows high-resolution waitable timer with no busy-wait loop or global timer-resolution changes. Desktop Duplication releases each acquired frame immediately after its GPU copy is queued, so a zero-timeout acquisition does not alternate fresh and stale images. The Capture page shows measured submitted FPS and fresh-source FPS separately from the selected frame-rate setting. Static scenes and unfocused games can legitimately have fewer fresh images.

Gavnel's minimal orange bullet sits on a rounded dark-gray tile. The SVG artwork is embedded in the executable and used in the window, tray, navigation, and desktop launcher. The bullet briefly splits and reassembles at startup and while a clip is saving. Animation stops when idle and does not repaint hidden windows. All navigation and utility icons use the SVG set in `assets/icons/`, rendered once per size and cached.

The quick-settings panel has independent open state so microphone, audio-device, monitor, and program dropdowns can open without dismissing it. Selecting a microphone option leaves the panel usable; clicking outside dismisses it.

Every in-app dropdown uses shared custom motion: a 180 ms eased reveal and fade when opening, a quicker 120 ms collapse and fade, and a rotating chevron. This includes recording modes, quality, sources, devices, microphone channels, clip length, library filters and sorting, quick settings, help, and clip action menus. Upward-opening lists reveal from their anchor too. Selection and Escape dismiss immediately; the closing visual is paint-only and cannot trigger menu actions or hold focus. Animations stop repainting once settled, and quick settings remains independent of its child menus.

Opening Gavnel again, including through the desktop launcher, brings the existing window forward instead of opening a duplicate recorder. The SVG artwork and Windows icon regeneration script are in `assets/`.

**Settings → General → Window & startup → When I close Gavnel** lets you choose **Exit Gavnel** or **Close to system tray**. Exit saves any full recording and removes the tray icon. Closing to the tray keeps capture running. The choice saves automatically and is retained across builds; new installations default to exiting. The Settings header contains no save, hide, or exit buttons.

While hidden, single-click or double-click the bullet tray icon to reopen Gavnel; right-click for **Open Gavnel**, recording controls, and **Exit**. Launching the executable again restores the existing window with both a native Windows restore message and the app's activation event. Explicit Exit always bypasses close-to-tray. Startup with Windows is optional and disabled by default. If you have an older copy already running, exit that copy before launching the updated executable; launching another copy otherwise restores the existing one.

## Defaults and requirements

- Windows 11 x64, SDR display, NVIDIA GPU and compatible current NVIDIA driver.
- 1920×1080, 60 FPS, 20 Mbps H.264, 60-second replay, F8 save key.
- PC audio enabled; microphone disabled. Enabled audio sources are mixed into a single AAC track.
- MP4 clips are saved beneath the configurable storage folder in game/app subfolders; monitor captures use `Desktop`.
- Settings live in the current user's roaming Gavnel configuration directory and are shared across app versions.
- One source at a time; one active and one pending export; 512 MiB limit for retained compressed packet data. That limit is not a limit on the application's total RAM or GPU memory.
- No FFmpeg installation, login, internet connection, or injected game overlay is required.

Game and window capture output black video when the target loses focus, while audio continues. Monitor capture includes everything visible on that monitor. Fullscreen compatibility capture uses the game's monitor only while the game is foreground. Switching capture backends or recovering a lost device can reset the replay buffer.

## Build

Install stable Rust for `x86_64-pc-windows-msvc`, Visual Studio C++ Build Tools, and the Windows SDK. Then run:

```powershell
cargo build --release --locked
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
```

The executable is `target\release\gavnel.exe`. NVENC is loaded from the installed NVIDIA driver at runtime. Video stays on the GPU through capture, scaling, and NV12 conversion. Export copies compressed packets into MP4 without re-encoding.

## Diagnostics

```powershell
.\target\debug\gavnel.exe --probe
.\target\debug\gavnel.exe --self-test --seconds 8 --output test-clips
.\target\debug\gavnel.exe --self-test --full-recording --seconds 70 --replay-seconds 5 --also-clip --output test-recordings
.\target\debug\gavnel.exe --verify 'test-clips\your-clip.mp4'
.\target\debug\gavnel.exe --audio-check 'test-clips\your-clip.mp4'
.\target\debug\gavnel.exe --audio-formats
.\target\debug\gavnel.exe --pc-audio-diagnostic work\pc-audio-check
```

`--self-test` records your primary desktop and PC audio to a local test clip. Use `--no-audio` for a video-only test. Diagnostic output can be redirected to a file when using the release executable.

`--audio-check` decodes the first 15 seconds of a saved file and reports channel levels, DC offset, clipping, and sample differences without playing it. `--audio-formats` reads the default Windows devices' format descriptions without starting capture. Neither diagnostic changes preferences or starts recording.

`--audio-check path --seconds 45` inspects a longer section. Only run `--audio-diagnostic output-folder` when you intend to record: it captures 15 seconds of PC audio and microphone separately as local WAV files, plus the mixed timeline and packet timing logs. It captures no screen and plays no sound.

`--pc-audio-diagnostic output-folder` explicitly records 10 seconds from all available playback endpoints, saves individual raw WAVs and a combined `pc-all.wav`, and writes device names and timing logs. It never opens a microphone or video source, plays sound, or activates a window. The older two-source diagnostic uses the default playback device only.

`--audio-reencode source.wav output.m4a` passes an existing diagnostic waveform through the production AAC encoder without recording or playing audio. The audio timing correction was checked with separate live desktop/microphone traces and an encoded speech sample; the corrected speech sample was confirmed audibly free of the reported static.

Use `--dxgi` with `--self-test` to exercise fullscreen-compatible Desktop Duplication, or `--game` to wait for an automatically detected game instead of recording the desktop. `--motion-window` shows a 45-second moving test scene; bring that window to the foreground while running a capture test. `--verify` reports decoded frame cadence, timing gaps, and approximate near-identical-frame counts. Static scenes can repeat legitimately; this diagnostic does not identify unique gameplay frames with certainty.

`--lifecycle-test` opens the app without recording, checks four close/reopen cycles through the tray-action and named launcher-event paths, then exits while hidden. It does not save settings or clips. Run it when another Gavnel instance is not running.

## Validation status

The initial recording pipeline saved an eight-second desktop/audio clip through NVENC on the development PC, reporting zero dropped frames during that run. Automated tests cover settings, compressed memory accounting, replay eviction, keyframe boundaries, H.264 packet formatting, and clip container/error handling.

Continuous recording tests cover writing beyond replay duration without retaining encoded payloads, timestamp rounding, keyframe starts, capture gaps, audio/video queue draining, empty recordings, and publishing only finalized files. Live full-session capture and long-duration soak testing remain to be completed.

Competitive-game frame-time comparisons, anti-cheat compatibility, exclusive-fullscreen transitions in each supported game, and the one-hour soak test still require validation. No claim of zero FPS loss or stutter-free gameplay has been established.

## Interface reference

The library grid, compact navigation rail, and persistent recording toolbar are informed by Medal's [Windows guide](https://medaltv.freshdesk.com/support/solutions/articles/48000959661/). Gavnel uses its own orange G icon, native Rust interface, and local-only features. Medal assets and screenshots are not included in the app.

See `THIRD_PARTY_NOTICES.md` for dependency information.

## Development launcher

The desktop shortcut runs `scripts/launch.ps1`. Install or repair it with
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-launcher.ps1`.
The launcher builds the current source with Cargo on every launch (unchanged builds
are incremental), then runs an immutable copy identified by its SHA-256 hash.
Agents do not need to copy builds into `dist`. Build failures show an error and
leave the existing app running; details are in `work/launcher.log`.
When a different build is already running, the launcher requests a normal Exit,
waits for recordings to finish saving, and starts the new build. Versions predating
this launcher must be exited from the tray once. It never forcibly terminates the
recorder. The first launch after edits may take longer while compiling.
Use `scripts/launch.ps1 -BuildOnly` to verify the build without opening the app.
