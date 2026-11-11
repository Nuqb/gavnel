# Changelog

## Unreleased

## 0.3.7

- Set installer publisher and executable company metadata to Software by Brox.
- Lowered the installer minimum to Windows 10 22H2 while retaining Windows 11 support. Disabled unsupported application-audio modes on Windows 10 and reject imported settings that request them before replacing an active recording. Windows 10 live validation is pending.

## 0.3.6

- Integrated minimize, maximize/restore, and close buttons into the app, with draggable header space, double-click maximize/restore, and edge resizing.
- Attached playback controls and the timeline to the main window so they follow window movement and transitions together.
- Added fullscreen playback, refined seek and volume controls, and consistent back buttons.
- Replaced project-file actions with a video-saving dialog offering current-video replacement or a new MP4, resolution choices, and file-size controls.
- Added background saving, progress and completion notices, and protection against exiting or modifying captures while saves are active.
- Added safe capture renaming that preserves bookmarks, highlights, and audio-track metadata.
- Added compressed-copy trims for compatible selections, safer staged exports and replacement recovery, and progressive timeline thumbnails.
- Updated the quick launcher to load completed builds through the normal graceful-restart path when an older build is running.

## 0.3.5

- Added a single release command with shared checks, optimized build caching, verified automatic publication, and recovery from interrupted publishing.
- Replaced external editing tools with Windows-native clip inspection, previews, audio mixing, and H.264/AAC exports to avoid blocked helper executables.
- Adopted source-available terms for new Gavnel material: free app use and private builds, with written permission required for redistribution, resale, rebranding, and code reuse in other products.
- Preserved existing MIT permissions for previously released material, third-party licenses, and unrestricted use of users' recordings and screenshots.
- Updated Cargo metadata and installer packaging to include the current license and historical MIT text.

## 0.3.4

- Fixed missing screenshot sound and banner confirmations.
- Added screenshot capture and save failure banners.
- Clarified General settings with shared capture sound, banner, volume, and preview controls.

## 0.3.3

- Added clip editing, project saving, and montage exports.
- Added application audio selection and optional game highlights.
- Improved recording shortcuts, playback, editor navigation, and library controls.

## 0.3.2

- Long recording in the top toolbar opens the clip popup instead of starting a recording.
- Shortcut modifiers and keys appear in separate boxes without plus signs or a bullet separator.
- Removed the folder, microphone, capture, and save clip actions from the Library toolbar.

## 0.3.1

- Shared onboarding-style checkboxes and radio buttons, refined recording options, and more compact settings panels.
- Cleaner library search and settings, a dedicated custom game dialog, and animated toggles.
- Passive shortcut handling and familiar punctuation labels.
- Improved source launcher feedback and optional quick launch of the latest completed build.
- Installed builds always check for updates automatically.
## 0.3.0

- First downloadable Windows x64 installer, with per-user installation and an uninstaller.
- GitHub update checks, checksum-verified downloads, and restart-to-update without reinstalling manually.
- Updated six-step onboarding, quality presets with hardware examples, audio device selection, and shared microphone testing.
- Custom onboarding checkboxes and radio controls for recording quality.
- Automated formatting, tests, lint checks, dependency license inventory, and tagged release packaging.

Windows 11 x64, SDR capture, and an NVIDIA GPU with compatible NVENC drivers are required. AMD/Intel GPU encoding and HDR capture are not supported.
