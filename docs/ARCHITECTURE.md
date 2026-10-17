# Architecture

OpenWave is one Rust crate: a Dioxus 0.7 desktop app (WebView2 window) with a small native backend.
`docs/CORE_API.md` describes the backend API and `docs/PRIVACY.md` lists every network destination.

## Big picture

```
 UI (Dioxus components, Tailwind CSS)           Native services (tokio)
 src/features/*  --->  src/ui/* (kit)           src/backend/*
        |                                          catalog (ytmapi-rs, no login)
        v                                          media: stream lookup + loopback proxy + cache
 src/player/*  (queue state machine,               import: Spotify embed page, YouTube links, files
   engine = bridge to one hidden <audio>)          lyrics (LRCLIB, opt-in), yt-dlp bootstrap
        |                                              ^
        v                                              |
 src/store/*  (versioned JSON documents)  <------ backend::api (single entry point for the UI)
        |
 %APPDATA%\OpenWave   (OPENWAVE_DATA_DIR overrides)
```

Audio never goes from YouTube to the web view directly. `backend::media` resolves a stream (direct
InnerTube lookup first, yt-dlp as a fallback) and returns a URL on an axum server bound to `127.0.0.1`
on a random port, guarded by a per-launch token. A hidden `<audio>` element plays that URL; a small JS
bridge (`assets/audio.js`) reports time and state back to Rust.

## Module map

| Path | Role |
|---|---|
| `src/main.rs` | Window config (`window_config`), launches `App` |
| `src/app.rs`, `src/routes.rs` | Root component (CSS, theme sync, toasts) and the router |
| `src/model.rs` | Shared domain types (tracks, albums, playlists...), all serde |
| `src/util.rs` | Small helpers |
| `src/i18n/` | Typed keys, `t()` / `t_args()`, locale JSON in `assets/locales/` |
| `src/ui/` | Design-system components: button, dialog, menu, slider, switch, tabs, toast, tooltip, icon (Lucide), theme |
| `src/features/shell/` | Titlebar, sidebar, layout, page chrome, global shortcuts, extension slots |
| `src/features/home/` | Home: welcome cards, continue listening, mixes, recents |
| `src/features/search/` | Search page, debounced and cancelable queries |
| `src/features/library/` | Playlists, liked, history, local files, playlist page |
| `src/features/album/`, `artist/` | Detail pages |
| `src/features/import/` | The one import dialog (source, link, preview, result) |
| `src/features/player/` | Player bar, queue drawer, fullscreen, lyrics UI, shortcuts popover |
| `src/features/onboarding/` | First run and the audio-engine (yt-dlp) banner |
| `src/features/settings/` | Settings sections, About, storage and data wipe |
| `src/player/` | `queue` (pure state machine), `controller` (transport API), `engine` (audio bridge), `radio` (autoplay similar), `session` (restore), `media_keys` (SMTC via souvlaki), `policy` |
| `src/store/` | `versioned` (schema + migrations), `persist` (atomic writes), `library`, `settings` |
| `src/backend/api.rs` | The only backend surface the UI calls; owns the runtime and data dir |
| `src/backend/catalog.rs`, `convert.rs` | Catalog via `ytmapi-rs` and mapping to `model` |
| `src/backend/innertube.rs`, `media.rs`, `proxy.rs` | Stream lookup, audio cache, loopback proxy |
| `src/backend/import*.rs` | Parse and match imported playlists |
| `src/backend/lyrics.rs`, `ytdlp.rs` | LRC lyrics (opt-in) and the yt-dlp download/verify/update |
| `assets/` | `main.css` (compiled, committed), `fonts.css` (Inter as data URIs), `icons/`, `locales/`, JS bridges, `branding/` |
| `build.rs` | Generates the icon lookup from `assets/icons/*.svg` |
| `tools/` | Tailwind build, asset extraction, `add-keys.mjs` (i18n) |
| `scripts/` | `gen-notices.mjs` (regenerates `THIRD_PARTY_NOTICES.md` from `cargo metadata`) |
| `installer/` | Inno Setup script for the Windows installer |

## Conventions

- Files under ~400 lines, functions under ~50, immutable updates, no `unwrap()` outside tests.
- Keep unit checks beside their source under `#[cfg(test)]`; run them with `cargo test`.
- UI classes must appear as complete string literals in `src/**/*.rs` so Tailwind can see them; rebuild with
  `npm run css` in `tools/` and commit `assets/main.css`.
- Persistence is JSON with a `version` field and per-step migrations; writes are atomic (temp file + rename).
- Anything that adds a network request must be added to `docs/PRIVACY.md`.

## Dependencies worth knowing

`dioxus` (UI + WebView2 via wry), `ytmapi-rs` (catalog, unauthenticated), `reqwest` + `rustls` (HTTP),
`axum` (loopback proxy), `souvlaki` (Windows media overlay and keys), `rfd` (file dialogs), `arboard`
(clipboard), `directories` (data dir), `sha2` (checksum verification of yt-dlp), `sys-locale` (language).
Licenses: `THIRD_PARTY_NOTICES.md`; policy: `deny.toml`.
