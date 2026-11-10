# Security policy

Security fixes target the latest stable release. Update through Settings > General > Updates or download the latest installer from this repository.

Do not post exploit details, credentials, or private recordings in a public issue. Use GitHub's [private vulnerability reporting](https://github.com/Nuqb/gavnel/security/advisories/new) when enabled. If that form is unavailable, open a minimal issue asking for a private reporting channel without including the vulnerability details.

The updater trusts this repository and GitHub's HTTPS release API. It accepts stable, newer semantic versions, an exact Windows installer name and repository URL, and GitHub's SHA-256 asset digest. Files are verified after downloading and again before installation. This protects against corrupted or substituted downloads; it does not protect against compromise of the repository's publishing account. Releases are currently unsigned.

Updates require an explicit download and restart action. The helper waits up to ten minutes for normal application exit, never kills a recorder, and does not remove recordings or preferences. A timeout leaves the installation untouched. The installer is per-user and does not request elevation.
