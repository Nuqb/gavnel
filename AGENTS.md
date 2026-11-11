# Software quality

Before planning, changing code, or adding features, read and apply
[docs/software-quality.md](docs/software-quality.md). This guide applies to all
repository work. Detect the stack first, review the affected code against its
applicable checks, and address meaningful structural issues in the change.
Treat size thresholds as review triggers, not reasons for mechanical splitting.
Preserve behavior and verify refactors with appropriate checks. Exclude generated,
vendored, and build-output files. Summarize fixes, verification, and any remaining
findings honestly; do not claim that passing tools proves the entire app is clean.

# UI copy

Keep the interface concise. Do not add persistent grey helper text, explanatory
subtitles, tips, marketing copy, or implementation details beneath settings,
controls, or panel headings unless the user explicitly requests them. Prefer
clear labels and values; use a short hover tooltip only when clarification is
needed. Preserve useful errors, validation messages, progress, and status text.
When changing a control, do not reintroduce helper text the user removed.
Examples to avoid: noise-suppression descriptions, explanations of how an input
threshold works, and reminders that changing settings clears a microphone test.

# Local development

The desktop shortcut uses scripts/launch.ps1, which builds the current source and
launches a versioned executable automatically. Do not replace this shortcut with
a direct executable link or manually deploy binaries to dist for local use.
After edits, run cargo fmt and appropriate checks. Build the runnable app with
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/launch.ps1 -BuildOnly.
Do not force-stop a running recorder to deploy changes; the launcher requests a
normal exit so pending recordings can finish saving.

# Publishing updates

When the user asks to "push an update" or "push update", publish a new stable
GitHub release as well as pushing the source, unless they explicitly request a
source-only push. Follow [docs/releases.md](docs/releases.md): prepare matching
package/lockfile versions, changelog, and versioned notes; review and commit on
main; then run `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Version X.Y.Z`.
This command waits for CI, pushes the tag, and waits for the workflow to verify
and publish the stable release. Use `-Resume` for transient failures. CI packages
and tests the actual release installer; do not duplicate local packaging for a
routine release. A source push or draft alone does not complete the request.
Do not replace assets on an existing published release. If publishing is blocked,
report the exact blocker and what remains unpublished.
