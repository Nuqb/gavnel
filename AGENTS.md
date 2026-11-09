# Software quality

Before planning, changing code, or adding features, read and apply
[docs/software-quality.md](docs/software-quality.md). This guide applies to all
repository work. Detect the stack first, review the affected code against its
applicable checks, and address meaningful structural issues in the change.
Treat size thresholds as review triggers, not reasons for mechanical splitting.
Preserve behavior and verify refactors with appropriate checks. Exclude generated,
vendored, and build-output files. Summarize fixes, verification, and any remaining
findings honestly; do not claim that passing tools proves the entire app is clean.

# Local development

The desktop shortcut uses scripts/launch.ps1, which builds the current source and
launches a versioned executable automatically. Do not replace this shortcut with
a direct executable link or manually deploy binaries to dist for local use.
After edits, run cargo fmt and appropriate checks. Build the runnable app with
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly.
Do not force-stop a running recorder to deploy changes; the launcher requests a
normal exit so pending recordings can finish saving.
