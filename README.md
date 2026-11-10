<p align="center">
  <img src="assets/readme/hero.png" alt="Gavnel — your best plays, saved on your PC. Local files. No ads. No account." width="100%">
</p>

<p align="center">
  <a href="https://github.com/Nuqb/gavnel/releases/latest"><strong>Download for Windows</strong></a> ·
  <a href="docs/usage.md">User guide</a> ·
  <a href="#development">For developers</a>
</p>

Gavnel saves your recent gameplay, full sessions, and screenshots to your PC. **No ads. No account. No subscription.**

## Install & play

**Requires Windows 11 x64, an NVIDIA GPU with NVENC, and an SDR display.** AMD/Intel encoding and HDR capture aren't supported yet. Gavnel is in active development.

1. **Download** the `Gavnel-Setup-VERSION-x64.exe` installer from the [latest release](https://github.com/Nuqb/gavnel/releases/latest).
2. **Install & set up** — open it, then choose your capture source, audio, and save folder.
3. **Turn capture ON** — play your game and press **F8** to save the last **60 seconds**.

No extra software needed. The installer is currently unsigned; release notes and `SHA256SUMS.txt` are included on the release page.

## Your moments, one key away

- **F8 → Replay clip** · Bookmark when a long recording is running.
- **Alt + F7 → Start / stop recording**
- **F4 → Bookmark**
- **F7 → Screenshot**

Change any shortcut in Settings. Find clips in the built-in library or open their folder: videos are **MP4**, screenshots are **PNG**.

**Replay clips · Full sessions · Automatic game recording · PC & mic audio · Local playback**

<details>
<summary><strong>Updates, defaults & capture tips</strong></summary>

- Update through **Settings → General → Updates → Download update → Restart to update**. Gavnel finishes saving first and keeps your preferences and recordings. Automatic release checks are optional.
- Default recording: **1080p · 60 FPS · 20 Mbps · 60-second replay**. PC audio is on; the microphone is off.
- Capture one source at a time. Game/window capture turns black when the target loses focus while audio continues; monitor capture includes everything visible on that display.
- Choose **Exit** to finish pending saves. Optional close-to-tray keeps capture running. A crash or power loss can leave a recording unfinished.
- Recordings stay local, so keep your own backups. Recording and playback work offline.

See the [user guide](docs/usage.md) for audio tracks, push to talk, noise suppression, storage, and playback.

</details>

## How it compares

All three apps save locally. Gavnel focuses on keeping your recordings on your PC.

<p align="center">
  <img src="assets/readme/comparison.png" alt="Gavnel, Medal, and Outplayed comparison: all save locally. Gavnel has no ads, paid tier, or cloud uploads. Medal and Outplayed have free-tier ads, optional paid plans, and optional uploads. Gavnel needs no account; Medal and Outplayed require accounts. Gavnel and Medal are standalone; Outplayed requires Overwolf to be installed. Interface language options: Gavnel 86 including regional choices, with native-speaker review pending; Medal 22; Outplayed 1, English." width="100%">
</p>

<details>
<summary><strong>Sources &amp; accessible text version</strong></summary>

- **Gavnel:** local files; no ads, paid tier, cloud uploads, or account requirement; standalone native app; **86 selectable language and regional options** ([localization notes](docs/localization.md)).
- **Medal:** [local files][medal-local]; [free-tier ads, Premium, and cloud options][medal-plans]; [account required][medal-account]; standalone app; **22 language options** in its [official supported-language list][medal-languages].
- **Outplayed:** [local files][outplayed-local]; [free-tier ads, Premium, and optional hosting][outplayed-plans]; [Overwolf client required][outplayed-app]. [Account required to access Outplayed][outplayed-plans], though recording continues while logged out. **English** is the sole text/interface language listed in its [publisher-provided Epic Games Store listing][outplayed-languages].

**Language options** counts selectable interface locales, including regional and writing-system variants. Gavnel's 86 choices cover 81 base language codes; 14 additional draft catalogs are withheld. Selectable translations are initial AI-assisted work awaiting independent native-speaker review. Medal's 22-item list includes Simplified and Traditional Chinese separately. Outplayed's count reflects its published store listing. Counts describe available options rather than equivalent translation quality.

Official product icons: [Medal support](https://support.medal.tv/) and [Outplayed](https://outplayed.tv/). Their marks belong to their respective owners and identify the products being compared.

Comparison checked **November 9, 2026**, using the vendors' official documentation. Plans and features can change. Gavnel's column describes this repository's implementation and stated direction, rather than a promise of future features.

Medal documents [slow interface/editor cases][medal-ui] and [choppy-recording troubleshooting][medal-performance]. Overwolf documents [capture performance issues][outplayed-performance] too. These describe problems that can occur; they do not establish that either app is always slow.

Fast startup and low recording overhead are Gavnel's goals. **There is no side-by-side startup, FPS, or memory benchmark here.** Both alternatives also support hardware encoding. Actual results depend on the game, hardware, driver, recording settings, and storage.

Gavnel currently has a narrower feature set: Windows with an NVIDIA GPU, a built-in game-detection catalog, and no automatic kill/death highlight detection, cloud sync, or built-in montage editor.

</details>



---

## Development

Native **Rust / egui** desktop app, Windows capture, and **NVIDIA NVENC** encoding. Recordings and preferences use local files; there is no application server. No separate FFmpeg installation is required.

### Build & run

Install stable Rust for Windows MSVC, Visual Studio C++ Build Tools, and the Windows SDK. From the repository folder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1
```

The launcher builds current source and opens a versioned executable. Later builds are incremental. If an older build is running, it requests a normal exit so recordings can finish saving.

To create desktop and Start menu shortcuts that use the same launcher:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/install-launcher.ps1
```

Keep local shortcuts pointed at the launcher; do not replace them with direct executable links or manually copy binaries to `dist`.

### Checks & contribution

Read the [software quality guide](docs/software-quality.md) before making changes.

```powershell
cargo fmt
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly
```

Replay retention has a **512 MiB compressed-packet budget**, not a cap on total RAM or GPU memory. Long recordings stream to disk. Startup speed and recording overhead are goals; no comparative performance benchmark is published here.

- [Development & diagnostics](docs/development.md) — source map, onboarding previews, and recording diagnostics.
- [Release packaging](docs/releases.md)
- [Game detection](docs/game-detection.md)
- [Localization](docs/localization.md)

[MIT License](LICENSE) · [Third-party notices](THIRD_PARTY_NOTICES.md)

[medal-plans]: https://support.medal.tv/support/solutions/articles/48001218475-medal-premium-faq
[medal-local]: https://support.medal.tv/support/solutions/articles/48001227747
[medal-performance]: https://support.medal.tv/support/solutions/articles/48000922094
[medal-ui]: https://support.medal.tv/support/solutions/articles/48001242234-nvidia-issues-with-desktop-app-editor-ui-blurry-slow-
[outplayed-app]: https://www.overwolf.com/app/overwolf-outplayed
[outplayed-plans]: https://support.overwolf.com/support/solutions/articles/9000284652-outplayed-account-plans
[outplayed-guide]: https://support.overwolf.com/support/solutions/articles/9000284663-outplayed-quick-set-up-guide
[outplayed-local]: https://support.overwolf.com/support/solutions/articles/9000284626-outplayed-faq
[outplayed-performance]: https://support.overwolf.com/support/solutions/articles/9000181387-common-capture-issues
[overwolf-codecs]: https://support.overwolf.com/support/solutions/articles/9000177861-video-codec-selection-and-availability

[medal-account]: https://support.medal.tv/support/solutions/articles/48001147502/

[medal-languages]: https://support.medal.tv/support/solutions/articles/48001280799-community-programs-faq
[outplayed-languages]: https://store.epicgames.com/p/outplayed/?lang=en-US
