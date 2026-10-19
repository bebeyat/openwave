# Third-party notices

OpenWave itself is licensed under **MIT OR Apache-2.0** (see `LICENSE-MIT` and `LICENSE-APACHE`).
It is built from the third-party components listed below. Each is used under its own license, and the
texts or pointers required by those licenses are reproduced here. This file is shipped in the portable
zip and the installer, and is regenerated with `node scripts/gen-notices.mjs` (see `docs/RELEASING.md`).

## Summary of non-code and tool components

| Component | Version | License | Where it is used |
|---|---|---|---|
| Inter (variable font, via `@fontsource-variable/inter`) | see `tools/package-lock.json` | OFL-1.1 | Embedded in `assets/fonts.css` and `assets/fonts/` (the UI typeface) |
| Plus Jakarta Sans (variable font, via `@fontsource-variable/plus-jakarta-sans`) | see `tools/package-lock.json` | OFL-1.1 | Display typeface (headings, hero titles): embedded in `assets/fonts.css` and `assets/fonts/`; license text in `licenses/PlusJakartaSans-OFL-1.1.txt` |
| Lucide icons (via `lucide-static` 1.59.0) | 1.59.0 | ISC (some icons derived from Feather, MIT) | `assets/icons/*.svg`, embedded at compile time |
| flag-icons (`flag-icons` 7.6.0) | 7.6.0 | MIT | Language menu flags: `assets/flags/*.svg`, embedded at compile time; license text in `licenses/flag-icons-MIT.txt` |
| Tailwind CSS | 4.x | MIT | Build-time only: compiles `tools/tailwind.css` into `assets/main.css`. The generated CSS keeps Tailwind's license banner. |
| Signalsmith Stretch (`signalsmith-stretch` 1.3.2, JS/WASM build) | 1.3.2 | MIT | Pitch shifting for the sound effects: `assets/fx_stretch.js` (vendored unmodified, wrapped in a function), run in an AudioWorklet |
| yt-dlp | latest official release | Unlicense (public domain dedication) | **Not distributed in this repository or in OpenWave's release files.** If you agree, OpenWave downloads the official release from github.com/yt-dlp/yt-dlp at runtime (verified against the published SHA2-256SUMS) and runs it as a separate program. |
| Microsoft Edge WebView2 Runtime | Evergreen | Microsoft Software License Terms | **Not distributed.** Already present on Windows 11 and current Windows 10; the installer links to Microsoft's bootstrapper if it is missing. |
| OpenWave mark and icon | n/a | MIT OR Apache-2.0 (original work of the project owner) | `assets/branding/` |

The Rust crates linked into the Windows build (including `souvlaki` (MIT), `arboard`, `sys-locale`,
`sha2`, `rfd`, `ytmapi-rs` (MIT), `reqwest`, `axum`, `tokio`, `serde`, Dioxus and `option-ext` (MPL-2.0))
are listed in the table further down with their versions, SPDX license identifiers and sources.

## MPL-2.0 component

`option-ext` 0.2.0 is licensed under the Mozilla Public License 2.0. OpenWave uses it **unmodified**,
as a normal compiled dependency (pulled in by the `directories` crate). Its source code is available at
https://github.com/soc/option-ext and https://crates.io/crates/option-ext/0.2.0. The full MPL-2.0 text
is reproduced in the license texts below. MPL-2.0 applies only to that crate's files; it does not
change the license of OpenWave's own code.

## Inter font (SIL Open Font License 1.1)

Inter is bundled unmodified (the Latin and Latin-Extended WOFF2 subsets of the variable font). The
copyright line below declares no Reserved Font Name. The font is not sold on its own; the OFL permits
bundling it with software. The full license text:

```text
{{INTER_OFL}}
```

## Lucide icons (ISC) and Feather icons (MIT)

```text
{{LUCIDE}}
```

## flag-icons (MIT)

```text
The MIT License (MIT)

Copyright (c) 2013 Panayiotis Lipiridis

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Tailwind CSS (MIT, build-time tool)

```text
MIT License

Copyright (c) Tailwind Labs, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the
following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR
THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## Signalsmith Stretch (MIT)

`assets/fx_stretch.js` is the official JS/WASM release of Signalsmith Stretch 1.3.2
(https://github.com/Signalsmith-Audio/signalsmith-stretch, npm `signalsmith-stretch`), vendored
unmodified apart from a function wrapper. It contains Emscripten-generated glue code (MIT). Copy of the
license: `licenses/Signalsmith-Stretch-MIT.txt`.

```text
MIT License

Copyright (c) 2022 Geraint Luff / Signalsmith Audio Ltd.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## yt-dlp (Unlicense), downloaded by the user at runtime

yt-dlp is released into the public domain under the Unlicense (https://unlicense.org). OpenWave does not
bundle it. Upstream: https://github.com/yt-dlp/yt-dlp . Some of the files in yt-dlp's own release binary
carry other licenses; they are covered by the notices shipped by the yt-dlp project.

---

# Rust dependencies
