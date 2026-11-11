# Third-party software

Gavnel uses open-source Rust libraries. Exact versions and checksums are recorded in Cargo.lock. The distribution's `licenses` directory contains license and notice files from the dependencies selected for Windows x64.

Gavnel's own source-available terms in `LICENSE` do not replace or restrict these third-party licenses. Third-party source, libraries, tools, and fonts retain the permissions granted by their respective rights holders, including redistribution and modification where allowed. Previously MIT-licensed Gavnel material also retains its existing permissions; see `HISTORICAL-MIT.txt` for the historical text.

Principal dependencies:

- nnnoiseless / RNNoise — BSD-3-Clause; neural microphone noise reduction — https://github.com/jneem/nnnoiseless
- rustfft — MIT or Apache-2.0; spectral microphone noise reduction — https://github.com/ejmahler/RustFFT

- egui / eframe — MIT or Apache-2.0 — https://github.com/emilk/egui
- egui_extras / resvg / usvg / tiny-skia — SVG rendering; see the dependency inventory and bundled license files.
- windows-rs — MIT or Apache-2.0 — https://github.com/microsoft/windows-rs
- windows-capture — MIT — https://github.com/NiiightmareXD/windows-capture
- mini-nvcodec / mini-cuda — MIT or Apache-2.0; NVIDIA interface headers retain NVIDIA's MIT notice — https://crates.io/crates/mini-nvcodec
- mp4 — MIT — https://github.com/alfg/mp4-rust
- tray-icon / global-hotkey / muda — MIT or Apache-2.0 — https://github.com/tauri-apps
- Other dependency licenses are listed in the distribution's dependency inventory.

Windows capture, audio, Media Foundation, and shell services are supplied by Windows. NVENC is supplied by the installed NVIDIA display driver. Neither FFmpeg binaries nor NVIDIA driver binaries are redistributed.

The application is not affiliated with Medal. No Medal artwork is distributed.

## nnnoiseless / RNNoise license

Copyright (c) 2020, Joe Neeman
Copyright (c) 2017, Mozilla
Copyright (c) 2007-2017, Jean-Marc Valin
Copyright (c) 2005-2017, Xiph.Org Foundation
Copyright (c) 2003-2004, Mark Borgerding

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

- Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

- Neither the name of the Xiph.Org Foundation nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION
OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.


## Noto fonts

Bundled Noto Sans script fonts and the Noto Sans CJK collection are distributed
under the SIL Open Font License 1.1. Original copyright notices and complete
licenses accompany each family in `assets/fonts/`. Source URLs and SHA-256 hashes
are recorded in `assets/fonts/manifest.json`. No Windows font is redistributed.

## Pinned egui text-direction changes

The MIT/Apache-2.0 egui family is pinned to
`87476c112a338f8940c64979dc6b46b90612e195` from
https://github.com/ahmedhosnypro/egui, corresponding to upstream proposal
https://github.com/emilk/egui/pull/8818. The patch adds Unicode bidirectional
layout and selection handling. See Cargo.lock for the resolved companion crates
and unicode-bidi dependency. Preserve upstream license notices with distribution.

## Translation drafts

Expanded catalogs were initially generated offline using Google's MADLAD-400 3B
(Apache-2.0), then edited for UI context and formatting. Model weights and the
translator are not distributed with Gavnel. Provenance and pending review status
are recorded in `assets/localization-status.json`.

## FFmpeg editing runtime

The local editor bundles the LGPL shared FFmpeg runtime, including FFmpeg, FFprobe, FFplay, and matching libraries. This runtime reports LGPL-3.0-or-later. Its license text, GPLv3 text, headers, import libraries, documentation, source revision, and build-recipe links accompany the runtime in the `media` directory. See `packaging/media-SOURCES.md` and `packaging/media.json` for the pinned build and acquisition details. Gavnel invokes the tools as separate processes; recording continues to use Windows capture and NVIDIA NVENC directly.
