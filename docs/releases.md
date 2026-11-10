# Building and publishing releases

Gavnel releases target Windows 11 x64. The per-user installer writes to `%LOCALAPPDATA%\Programs\Gavnel` by default and includes the dependency inventory, license notices, and an uninstaller. Settings and recordings are stored separately and survive updates and uninstalling.

## Prepare a release

1. Update the Cargo package version, regenerate Cargo.lock, update CHANGELOG.md, and add `docs/releases/vVERSION.md` with user-facing release notes.
2. Run formatting, tests, strict Clippy, and the locale checker. Verify setup and update UI. Test capture on an NVIDIA machine before claiming recording performance.
3. Run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-release.ps1 -ExpectedVersion VERSION`. It builds with the static MSVC runtime, collects notices from locked dependencies, verifies the executable version, and compiles the installer. Output is under `work/releases/vVERSION/`. It refuses to overwrite an existing payload unless `-Resume` is explicitly used to resume a failed unpublished build.
4. Test installation into a disposable folder with `/VERYSILENT /NORESTART /NOSHORTCUTS=1 /NOLAUNCH=1 /NOREGISTRY=1 /DIR="FULL_PATH"`; test uninstalling without deleting user data. Never test against a running recorder or existing source shortcut.
5. Commit the reviewed source, push the version tag `vVERSION`, and let the Release workflow test installation, updating, and uninstalling before creating a **draft** release with the installer and SHA256SUMS.txt. Review its assets, checksums, notes, and CI before publishing the draft. Drafts are never offered to the updater.

The initial public release requires a public GitHub repository. Making a private repository public exposes its source and Git history, so this is a separate owner decision.

## Update behavior

Installed builds check the latest stable release on startup and at roughly six-hour intervals while active. Source builds do not check automatically and continue to use `scripts/launch.ps1`. Manual checking uses the unauthenticated public GitHub API; no access token is shipped.

Update metadata must name a newer stable semantic version and `Gavnel-Setup-VERSION-x64.exe`, with the exact download URL in this repository and GitHub's `sha256:` digest. Metadata and download sizes, redirects, HTTPS, and timeouts are bounded. Updates are downloaded only after the user chooses Download update. The installer is verified before presenting Restart to update and rechecked by the helper after graceful shutdown. The helper is a separate executable copy so Windows file locks do not interrupt recordings. The installer preserves the installation directory and leaves settings and recordings alone.

An update error is reported rather than silently accepting an unverified payload. Check the update installer log in the user's Gavnel update cache for details. A process that does not exit within ten minutes is never killed. The release installer refuses to proceed while the app mutex exists.

Inno Setup's compiler download is pinned by version and SHA-256 in `scripts/get-inno.ps1`. GitHub Actions are pinned to reviewed commits. Binaries are not Authenticode-signed yet; checksum integrity and GitHub trust are not a publisher signing certificate.

Before deleting or changing the release repository, account name, installer naming convention, or install marker, plan a migration: existing clients have that repository and protocol compiled in. Do not delete historical release assets that users may need for recovery.
