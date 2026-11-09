# Built-in game detection

Gavnel matches executable filenames of open game windows, ignoring ASCII case.
The bundled catalog contains **7,579 titles and 7,922 executable aliases**.
Existing settings receive the built-in catalog on load; custom entries remain,
except where their executable is now covered by a built-in entry.

## Source and refresh

Windows executable records come from Discord's public detectable catalog:
https://discord.com/api/v9/applications/detectable (snapshot retrieved November 9, 2026).
Medal's directory (https://medal.tv/games) was consulted during the initial expansion;
this is an independent catalog, not a claim of complete Medal coverage.

To reproduce the data from the snapshot:

```powershell
python scripts/update_game_catalog.py work/detectable-games.json
python scripts/test_game_catalog.py
cargo fmt
```

The importer writes `src/config/game_catalog_data.rs`. Keep matching logic and
Rust regression checks in `src/config/game_catalog.rs`. The small legacy-name
allowlist preserves previously supported titles whose source records lack store
or theme metadata; all executable safety checks still apply to them.

## Filtering policy

New titles need game theme metadata, a store listing, and a Windows executable.
The importer strips directory prefixes, deduplicates aliases, and rejects names
associated with multiple application IDs anywhere in the source snapshot.
Launchers, installers, configuration tools, editors, servers, crash handlers,
shared runtimes, argument-dependent matches, and generic filenames are excluded.
Known software categories such as RPG Maker tools and modelers are also excluded.
The original Fortnite, Valorant, Rust, and CS2 entries remain first for compatibility.

The broader audit removes several older false matches, such as `start.exe`,
`uplayinstaller.exe`, and runtime installers previously attributed to games. Settings migration removes
these retired built-in name/executable pairs; deliberately named custom profiles
for the same executables are preserved.
Source metadata can be incomplete or inaccurate; these filters reduce false
matches but do not replace testing individual games.

Minecraft's dedicated Windows executables are included. Java-based games sharing
`javaw.exe`, including Minecraft Java and Spiral Knights, need richer process
context and are deliberately excluded from filename-only automatic detection.
Entries lacking a usable Windows executable remain unsupported. Custom profiles
are available in Settings for alternate executable variants.

## Performance and verification

Built-in membership uses a lazily initialized hash index, avoiding quadratic
settings migration and custom-profile checks. Each automatic window scan builds
one executable set and selects the foreground match without sorting all windows.
That changes matching from O(windows × profiles) to O(windows + profiles).

The browser caches grouped search results until profiles or query change and draws
only visible rows. Back and search controls stay outside the card scrolling area.
No catalog download or network lookup occurs during detection.

Regression checks cover unique safe filenames, case-insensitive lookup of every
alias, representative new and existing titles, custom profiles, foreground
selection, importer conflict handling, and utility/runtime exclusions. These
entries enable process recognition, not kill/event detection. Individual titles
and storefront variants have not all been run locally.

## Identity and executable audit

The November 9, 2026 audit cross-checks every bundled alias against its Discord
source record and checks available Steam application types and Windows launch
configuration through the SteamCMD metadata mirror (https://www.steamcmd.net/).
Missing classifications are checked against Valve's store appdetails endpoint.
An exact Steam launch match is recorded separately from a game whose executable
association is supported only by Discord: launch metadata may itself point to a
launcher, and child game processes may not appear there at all.

Run `python scripts/audit_game_catalog.py` with Python's `requests` package
installed to repeat the audit. Responses and detailed results are cached under
`work/game-audit`; no audit requests happen in the running app. Follow up on
unknown or contradictory records rather than treating them as successful checks.

`scripts/game_catalog_review.json` preserves source-backed exclusions, corrected
store links, and three confirmed replacement game executables. Fuse (the 3D
character creator) and GameGuru (the game-making tool) were removed as software.
Sixteen utility or overly generic aliases were removed, and three were replaced
with confirmed gameplay executables. Retired entries migrate out of older
settings using their original name and executable pair, preserving deliberately
named custom profiles. Individual game installations were not downloaded or run.

The completed basic pass found no remaining confirmed software titles. Of the
7,922 retained aliases, 5,921 matched a game's Windows Steam launch entry;
1,874 had a separately confirmed game identity with the executable association
still supported by Discord; 127 lacked independent store confirmation (largely
non-Steam or older titles). These categories deliberately distinguish evidence
strength. A store rate limit interrupted some fallback lookups; those entries
remain unconfirmed rather than being counted as successful checks. Cached
results allow a later audit run to resume without repeating completed requests.
