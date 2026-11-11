# Building and publishing releases

Gavnel releases target Windows 11 x64. The per-user installer writes to `%LOCALAPPDATA%\Programs\Gavnel` by default and includes the dependency inventory, license notices, and an uninstaller. Settings and recordings are stored separately and survive updates and uninstalling.

## Meaning of "push an update"

A request to push an update includes publishing a new stable GitHub release,
unless the user explicitly asks for a source-only push. Use the command below;
completion means a successful Release workflow and a published stable release.
A source push or draft alone is not completion. Use a new patch version for small
fixes. Never move a released tag or replace published assets.

## Release in three steps

1. Prepare the next version: update `Cargo.toml` and the root package in
   `Cargo.lock`, move the relevant changes from `Unreleased` into a
   `## X.Y.Z` changelog section, and add `docs/releases/vX.Y.Z.md` with reviewed,
   user-facing notes. Keep an `Unreleased` section for future work.
2. Review and commit the intended changes on `main`. Run the shared local checks
   and the source build below. Verify any affected setup, update, or editor UI;
   test capture on an NVIDIA machine before claiming recording performance.
3. Run the release command from a clean checkout:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Version X.Y.Z
```

Prerequisites: Python 3.11+, Rust, Git, and GitHub CLI (`gh`). Run `gh auth login`
once with permission to push this repository and view Actions. The command pushes
`main`, waits for CI and its optimized build cache, creates and pushes the version
tag, waits for Release, and prints the published URL. It never stages files,
commits unrelated work, force-pushes, or moves existing tags. A transient failure
does not roll back a completed source push or tag push; use the recovery steps below.

Local verification uses the same checks as CI and Release:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly
```

CI runs formatting, Python tests (including release safety and license inventory),
localization checks, Rust tests, and strict Clippy. On pushes to `main`, a parallel
job also builds the static-runtime executable and saves its distribution cache.
Debug and distribution caches are separate and can reuse dependencies across
lockfile changes. The first build is cold; later release tags can restore caches
from `main`. Do not expect one version tag to restore another tag's cache.

## What the Release workflow does

Before building, it validates the stable tag, checked-out commit, ancestry on
`main`, package and lockfile versions, changelog, release notes, and release
availability. It repeats the shared checks, packages the optimized executable,
licenses, and notices, then tests the installed native editor, update helper, and
uninstaller in disposable directories.

The tested installer and `SHA256SUMS.txt` are retained as a workflow artifact for
14 days. A separate publishing job with write permission creates or resumes a
draft, uploads only missing assets, downloads them again, and compares their bytes,
checksums, sizes, GitHub SHA-256 digests, and notes against the tested files and
tagged source. It then publishes the verified draft as the latest stable release.
Publication jobs are serialized, and an older version cannot replace a newer
latest release. Drafts are never offered to the updater.

Pushing a prepared `vX.Y.Z` tag directly also runs this complete workflow. The
release command is preferred because it waits for the cache-warming CI run first.
Do not package and install a second local copy for a routine release; CI tests the
actual installer that will be published.

## Recover an interrupted release

For a transient failure, stay on the original version's clean `main` commit and run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Version X.Y.Z -Resume
```

The command reuses the existing tag and reruns failed jobs. If only publication
failed, GitHub reuses the successful build job's artifact without rebuilding or
reinstalling. You can also select **Re-run failed jobs** in Actions. A rerun never
overwrites existing release assets. Different draft bytes, notes, or unexpected
assets stop publication for review; do not bypass the check with `--clobber`.

If source changes are needed after tagging, use a new version and tag. If the
artifact has expired, rerun all jobs to rebuild and retest; an existing draft must
still match the rebuilt bytes, or be reviewed and removed while unpublished before
retrying. Never delete or modify a published release to recover a failed attempt.
Report the exact failed job and what remains unpublished if recovery is blocked.

## Package without publishing

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-release.ps1 -ExpectedVersion X.Y.Z
```

Output is under `work/releases/vX.Y.Z/`. Packaging verifies the executable version,
uses the static MSVC runtime, collects locked dependency notices, runs the native
editor self-test, and compiles the installer. It refuses an existing payload unless
`-Resume` explicitly resumes an unpublished build. Use
`scripts/test-native-installer.ps1 -Installer FULL_PATH` for an isolated installation
and native-editor test. Never test against a running recorder or source shortcut.

The initial public release requires a public GitHub repository. Making a private repository public exposes its source and Git history, so this is a separate owner decision.

## Update behavior

Installed builds check the latest stable release on startup and at roughly six-hour intervals while active. Source builds do not check automatically and continue to use `scripts/launch.ps1`. Manual checking uses the unauthenticated public GitHub API; no access token is shipped.

Update metadata must name a newer stable semantic version and `Gavnel-Setup-VERSION-x64.exe`, with the exact download URL in this repository and GitHub's `sha256:` digest. Metadata and download sizes, redirects, HTTPS, and timeouts are bounded. Updates are downloaded only after the user chooses Download update. The installer is verified before presenting Restart to update and rechecked by the helper after graceful shutdown. The helper is a separate executable copy so Windows file locks do not interrupt recordings. The installer preserves the installation directory and leaves settings and recordings alone.

An update error is reported rather than silently accepting an unverified payload. Check the update installer log in the user's Gavnel update cache for details. A process that does not exit within ten minutes is never killed. The release installer refuses to proceed while the app mutex exists.

Inno Setup's compiler download is pinned by version and SHA-256 in `scripts/get-inno.ps1`. GitHub Actions are pinned to reviewed commits. Binaries are not Authenticode-signed; checksum integrity and GitHub trust are not a publisher signing certificate.

## Native editor verification

Clip inspection, timeline thumbnails, still previews, audio mixing, and H.264/AAC export use Windows Media Foundation inside Gavnel. Releases and source launches no longer download, copy, or execute FFmpeg/FFprobe. Signing credentials are not required to build or package Gavnel.

Packaging runs `scripts/test-media-runtime.ps1 -App PATH` against the payload. The explicit `--editor-self-test` diagnostic creates synthetic video and audio in a disposable temporary directory, inspects it, decodes timeline frames, trims/crops, mixes audio, and exports an MP4. It never records or plays sound. The Release workflow repeats this check from an isolated installation containing no bundled codec executables or DLLs, then runs the existing update/uninstall test.

For an unpublished packaging check of the current version, use `-OutputDirectory work/native-package-smoke` with a new directory so an existing version's payload and release assets are never overwritten.

Native exports currently use Windows' H.264/AAC encoding path rather than the recorder's NVENC path; export speed may differ. Before publishing, also exercise editing on Windows 11 with Smart App Control enabled. Gavnel itself remains unsigned, so removing the external-tool failure does not guarantee acceptance of the main application under every Windows policy.

Before deleting or changing the release repository, account name, installer naming convention, or install marker, plan a migration: existing clients have that repository and protocol compiled in. Do not delete historical release assets that users may need for recovery.
