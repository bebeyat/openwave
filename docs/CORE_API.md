# Core API (non-UI)

Quick reference for UI code. Everything below is in the single `openwave` crate. Read the doc
comments in the source for details.

## Conventions

* **Immutable values.** Store and queue operations take `&self` and return a new value.
* **Ids** are opaque strings (`model::Id` = `model::Text`, an `Arc<str>` newtype: cheap to clone,
  derefs to `&str`, compares with `&str`/`String`, serializes as a plain string).
  Tracks: YouTube video id, or `local-<uuid>` for files from disk. Playlists: random uuid.
* Durations are **milliseconds** (`Track::duration_ms`); timestamps are Unix **milliseconds**.
* **Errors are typed**, never prose. Every error implements `backend::Coded`:
  `err.code()` is a stable code that is also the i18n key (e.g. `import.error.bad_link`), and
  `err.user_message()` returns the translated text. Never show `Display` output to users.
* **All user text** goes through `i18n::{t, t_args, t_count}`; add keys to **both**
  `assets/locales/tr.json` and `en.json` (a test fails otherwise, and another test fails when a
  literal `t("...")` in `src/**/*.rs` has no key).
* Async functions in `backend::api` run on a private tokio runtime; call them from any executor.
* Consent is the UI's job: `ytdlp_install`, `ytdlp_check_update` and `lyrics(enabled = true)` are
  the only calls that reach GitHub / LRCLIB, and they must only be called after the user agreed
  (yt-dlp) or enabled lyrics.

## Startup

```rust
backend::init()?;                                   // runtime + media proxy + catalog client
let s = store::settings().get();                    // Arc<Settings>, clone-free snapshot
i18n::set_language(s.effective_language());
```

## `model`

`Track { id, title, artists: Vec<ArtistRef>, album: Option<AlbumRef>, duration_ms, cover: Option<Cover>, explicit }`
(`artists_label()`, `is_local()`), `ArtistRef`, `AlbumRef`, `Cover { url, width, height }`,
`SearchResults { tracks, artists, albums, playlists }`, `ArtistItem`, `AlbumItem`, `PlaylistItem`,
`ArtistDetails`, `CollectionDetails { kind: CollectionKind::{Album, Playlist}, .. }`,
`Lyrics { plain, synced: Vec<LyricLine>, provider }` (`is_synced()`, `line_at(position_ms)`),
`LyricLine { start_ms, text, words: Vec<LyricWord> }`, `Playlist { id, name, tracks, created_ms, updated_ms }`.

## `backend::api`

```rust
pub fn init() -> ApiResult<()>;
pub fn data_dir() -> PathBuf;
pub async fn search(kind: SearchKind, term: &str) -> ApiResult<SearchResults>;   // All|Track|Artist|Album|Playlist
pub async fn artist(id: &str) -> ApiResult<ArtistDetails>;
pub async fn collection(kind: CollectionKind, id: &str) -> ApiResult<CollectionDetails>;
pub async fn radio(track_id: &str) -> ApiResult<Vec<Track>>;                     // "autoplay similar"
pub async fn stream(track_id: &str, quality: Quality) -> ApiResult<Stream>;       // Stream { url, mime, seconds }
pub async fn stream_fresh(track_id: &str, quality: Quality) -> ApiResult<Stream>; // forgets the remembered address first (retry)
pub fn prefetch(track_ids: &[String], quality: Quality);                          // address lookup only, max 4 at once
pub async fn cache_info() -> ApiResult<CacheInfo>;  pub async fn clear_cache() -> ApiResult<()>;
pub fn open_cache_folder();
pub async fn lyrics(enabled: bool, q: &LyricsQuery<'_>) -> ApiResult<Option<Lyrics>>;
pub async fn import_audio() -> ApiResult<Vec<Track>>;                             // native file picker
pub async fn add_local_files(paths: Vec<PathBuf>) -> ApiResult<Vec<Track>>;       // e.g. drag and drop
pub async fn ytdlp_status() -> ytdlp::Status;
pub async fn ytdlp_check_update() -> ApiResult<ytdlp::UpdateCheck>;
pub async fn ytdlp_install(on_progress: impl Fn(ytdlp::Progress) + Send + Sync + 'static) -> ApiResult<ytdlp::Installed>;
```

`stream().url` is a loopback proxy URL for the single hidden `<audio>` element (Range supported).

### Import (one dialog, many entry points)

```rust
backend::import::parse_link(input) -> Result<Source, ImportError>   // BadLink | NotAPlaylist
api::import_prepare(&source).await -> Result<PreparedImport, ImportError>  // nothing saved yet
//   prepared.preview { name, count, cover, warning: Option<ImportError::Truncated{limit}> }
api::import_run(prepared, |done, total| ..).await -> Result<ImportResult, ImportError>
//   ImportResult { name, tracks, missing: Vec<MissingTrack{title, artists}>, warning }
```

Error variants: `BadLink, NotAPlaylist, PrivateOrEmpty, Offline, NothingMatched, Truncated { limit }, Unreadable`.
Saving is yours: `store::library().update(|l| l.create_playlist(..).add_to_playlist(..))`.

### Lyrics

`backend::lyrics::{fetch, parse, parse_lrc, parse_time}`. `fetch(false, ..)` returns
`Err(LyricsError::Disabled)` without any network access. `parse` accepts synced LRC (multiple
stamps per line, `[offset:]`, enhanced `<mm:ss.xx>` words, metadata tags skipped) or plain text.

### yt-dlp

`ytdlp::{locate, status, latest_tag, check_update, install_latest, compare_versions}`.
`locate()` looks in `<data dir>/bin` first, then `PATH`. `Progress { phase: Downloading|Verifying|Installing|Done, downloaded, total }`.
The download is verified against the release's `SHA2-256SUMS`, started in a staging file, executed
once with `--version`, then moved into place atomically.

## `store`

```rust
store::settings(): &'static Store<Settings>;   store::library(): &'static Store<Library>;
store.get() -> Arc<T>;                          store.update(|cur| next) -> Result<Arc<T>, StoreError>;
store.report() -> &LoadReport                   // recovered_from (corrupt backup), migrated_from, newer_version
```

`Settings` fields: `language: LanguagePref{System,Tr,En}`, `theme: ThemePref{Light,Dark,System}`,
`accent` (one of `ACCENT_IDS`), `low_power`, `autoplay_similar`, `double_click: DoubleClickAction`,
`resume_queue`, `pause_on_device_change`, `audio_quality`, `cache_limit_mb`, `lyrics_enabled`
(default false), `onboarding_done`. Change with `settings.apply(SettingsChange::Theme(..))` inside `update`.

`Library` (pure `&self -> Library`): `create_playlist(id, name, now)`, `rename_playlist`,
`delete_playlist`, `add_to_playlist` (skips duplicates), `remove_from_playlist`,
`move_in_playlist(id, from, to, now)`, `toggle_like`, `is_liked`, `record_play`, `clear_history`,
`remember_search`, `forget_search`, `clear_searches`, `set_last_queue`, `add_local_files`,
`remove_local_file`; helpers `library::new_id()`, `library::now_ms()`.
Caps: history 200, recent searches 12. Corrupt files are moved to `<name>.corrupt-<ms>`; v0 and
unknown fields migrate/ignore silently.

## `player::queue`

`Queue` is a pure state machine; every method returns a new `Queue` (or a `Step { queue, action }`
with `Action::{Play, Restart, Stop}` telling the player what to do).

```rust
Queue::new(); q.play_now(&tracks, start, seed); q.add_next(&tracks); q.add_last(&tracks);
q.move_entry(from, to); q.remove(uid); q.clear(); q.clear_upcoming();
q.set_repeat(Repeat::{Off,All,One}); q.cycle_repeat(); q.set_shuffle(on, seed);
q.next(auto: bool) -> Step;   // auto = track ended by itself (repeat-one restarts only then)
q.previous() -> Step;         // history first; "restart if > 3 s in" is the player's rule
q.play_uid(uid) -> Step;
q.entries(), q.current(), q.current_uid(), q.upcoming(), q.snapshot(position_ms) -> QueueSnapshot;
Queue::restore(&snapshot, seed); player::queue::fresh_seed();
```

Entries carry a unique `uid` (same track can be queued twice; key lists and drag by uid).

## `i18n`

`t(key)`, `t_args(key, &[("name", "value")])` (`{name}` placeholders), `t_count(key, n)` (uses
`key.one` / `key.other`, fills `{count}`), `set_language(Lang)`, `language()`,
`detect_system_language()`, `Lang::{Tr, En}`. Lookup order: current language, English, the key.
Every `t()` call subscribes the calling component to `i18n::LANGUAGE_REVISION` (a global signal
bumped by `set_language`), so a language switch re-renders text without extra wiring.
Key naming: `area.thing[.variant]`, e.g. `nav.home`, `import.error.bad_link`, `player.queue`.

## Sound effects (`player::effects`)

Speed 0.5x-2.0x and pitch -12..+12 semitones (independent, or "linked" like nightcore/slowed), reverb
(`ReverbPreset`: off, room, salon, hall, cathedral, plate; impulse responses generated in `assets/fx_ir.js`),
bass boost (0-12 dB) and four quick presets (`QUICK_PRESETS`). State: `effects::FX` (`Fx`, immutable; change
with `effects::update(|fx| fx.with_speed(0.8))`). The DSP lives in `assets/fx.js` (Web Audio graph around the
single `<audio>`; pitch shifting by the vendored MIT signalsmith-stretch in an AudioWorklet).

* Nothing is created while everything is neutral (the default): no AudioContext, playback is unchanged.
* Positions and durations stay in track time at any speed (`currentTime` is media time).
* Persisted in `settings.json`: `fx_reverb`, `fx_reverb_mix`, `fx_bass_db`, `fx_linked`. Speed and pitch are
  not saved: every launch starts at normal speed and pitch.
* Needs `crossorigin="anonymous"` on the element, the proxy's exact-origin CORS answer, a CSP with `blob:` and
  `'wasm-unsafe-eval'` in `script-src`, and a secure-context web view (`app::browser_args()`), see `src/app.rs`.
