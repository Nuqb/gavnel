# Third-party software

Gavnel uses open-source Rust libraries. Exact versions and checksums are recorded in Cargo.lock. The distribution's `licenses` directory contains license and notice files from the dependencies selected for Windows x64.

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

