# Contributing

Read [the software-quality guide](docs/software-quality.md) and [the development guide](docs/development.md) first. Gavnel is a native Rust/egui Windows application; changes should preserve recordings and user preferences.

Use Windows 11 x64, Rust 1.95 or newer, Visual Studio C++ Build Tools, and the Windows SDK. Run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1` before submitting a pull request; CI and Release use this same command. Live recording needs an NVIDIA GPU; ordinary tests do not require recording hardware.

Build the local runnable app using `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly`. Keep source-launcher shortcuts intact and never terminate a recorder to deploy changes.

Report bugs with the app version, Windows build, GPU/driver, recording mode, and steps to reproduce. Remove private paths, device names, and recording contents from logs before sharing. For vulnerabilities, follow [SECURITY.md](SECURITY.md).

Release maintainers should follow [the release guide](docs/releases.md): prepare
the version and notes, commit on `main`, and run `scripts/release.ps1 -Version X.Y.Z`.
Use `-Resume` for an interrupted release. Completion includes the published stable
release, not just a source push or draft.
