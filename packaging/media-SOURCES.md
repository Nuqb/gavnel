# Bundled editing runtime

Gavnel bundles FFmpeg/FFprobe/FFplay 8.1.3-34-ga35157abb9 from the LGPL shared Windows x64 build dated November 9, 2026. `media.json` pins the archive SHA-256; `media-files.json` pins every executable and shared library. The binaries report LGPL version 3 or later; no GPL/nonfree build is selected.

FFmpeg source revision: https://github.com/FFmpeg/FFmpeg/tree/a35157abb9
Source archive: https://github.com/FFmpeg/FFmpeg/archive/a35157abb9.tar.gz
Build scripts and dependency recipes: https://github.com/FFmpeg-Builds/FFmpeg-Builds/tree/autobuild-2026-11-09-12-42
Archive and provenance: see media.json.

Run `python scripts/get-media.py` to reproduce acquisition and verification. It fails on a missing archive or checksum mismatch rather than substituting another build. If upstream removes an artifact, a maintainer must preserve/review a matching mirror or explicitly update the manifest and checksums.

The shared libraries can be replaced independently of Gavnel, subject to compatible interfaces. The runtime archive includes its license, headers, import libraries, and documentation. Copies of GPLv3 and LGPLv3 accompany the distribution. No editing runtime or translation service is downloaded by the application at run time.
