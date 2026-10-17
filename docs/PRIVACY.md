# Privacy

OpenWave has no account, no telemetry, no analytics, no crash reporting and no online storage.
Your library, playlists, likes, history, settings and cached songs are files in
`%APPDATA%\OpenWave` (or the folder named by the `OPENWAVE_DATA_DIR` environment variable). They
are never uploaded. Deleting that folder removes everything OpenWave knows about you.

## What OpenWave connects to

This is the complete list of outbound network destinations, checked against the source code
(`src/backend/`). Anything not listed here is never contacted.

| Destination | When | What is sent | Why |
|---|---|---|---|
| `music.youtube.com` (YouTube Music InnerTube API, via `ytmapi-rs`, no login) | When you search or open an artist, album, list or radio | The search text or the item id, plus the standard anonymous client context | Catalog: search results, artist/album/playlist pages, similar songs |
| `www.youtube.com` (`/youtubei/v1/visitor_id`) | Once when OpenWave starts, and again after about 2 hours | An anonymous client context (no cookies, no account, no song) | Gets an anonymous visitor id that the stream lookup needs; also opens the connection |
| `www.youtube.com` (`/youtubei/v1/player`) | When you play a song, and ahead of time for the next two queue entries, the "continue where you left off" song and the first Home cards | The song id, an anonymous client context and the anonymous visitor id (no cookies, no account). Only the stream address is requested ahead of time, never audio | Finds the audio stream address |
| `*.googlevideo.com` (HTTPS port 443 only) | While a song plays or is cached | Range requests for the audio of that song | The audio data itself. Only OpenWave's local proxy fetches it; redirects to any other host are refused |
| `i.ytimg.com`, `*.ggpht.com`, `*.googleusercontent.com` | When a cover picture is shown, when the app takes the colors of a cover, and for the Windows media overlay | (1) A normal image request from the app window. (2) A download of a small copy of the cover by OpenWave's own code (user agent `OpenWave/<version>`, https and these hosts only, no cookies) to pick the accent colors; the result is cached in `cache/palette.json`. (3) The Windows media overlay (volume pop-up, lock screen) receives the cover address and fetches the picture itself | Cover and artist pictures, the cover-colored look. The backend only hands out image URLs on these hosts |
| `lrclib.net` | **Only if you turn on lyrics** (Settings > Lyrics; off by default) | Song title, artist, album and length | Finds synchronized lyrics. With the setting off, the lyrics code returns before any request is made (unit test: `disabled_never_touches_the_network`) |
| `open.spotify.com` (`/embed/playlist/<id>`) | **Only while you import a Spotify link** | The playlist id from the link you pasted | Reads the public playlist page to get song names. No Spotify login or key. The page is read through at most 2 MB; anything larger is rejected as unreadable |
| `github.com`, then `objects.githubusercontent.com` or `release-assets.githubusercontent.com` (the only two hosts GitHub redirects release files to; any other host is refused) | **Only after you agree** to download or update the audio engine | A plain request for the official `yt-dlp` release and its `SHA2-256SUMS` file | Installs yt-dlp. The download is checked against the published SHA-256 before it is installed; a mismatch installs nothing |
| `www.youtube.com` and its media hosts, contacted by the **yt-dlp** program | Only when the direct stream lookup fails or is still pending after 1.2 seconds, and yt-dlp is installed (the slower of the two is cancelled) | What yt-dlp normally sends to YouTube for one song id | Fallback way to find the audio stream. OpenWave runs it with `--ignore-config --no-plugin-dirs --no-cache-dir`, from OpenWave's own `bin` folder as working directory |

Notes:

* OpenWave does not use YouTube or Google sign-in, and `ytmapi-rs` is used in its unauthenticated
  mode only. No cookies or tokens are stored.
* **A yt-dlp found on your `PATH` may be run.** OpenWave prefers its own downloaded copy
  (`bin/yt-dlp.exe`). If there is none, it falls back to a `yt-dlp` found on your `PATH`, but only
  through absolute `PATH` entries (never the current folder or an empty entry). A copy from `PATH` is
  treated as untrusted: it is never started just to read its version (Settings shows it as
  "unverified"), and it only runs when a song's direct stream lookup failed and the fallback is
  needed. If you do not want that, do not install yt-dlp on your `PATH` or remove it.
* YouTube and Spotify can see your IP address like any website you visit. If that matters to you,
  use a VPN.
* Reading the clipboard for a playlist link (import dialog) happens locally; the text is not sent
  anywhere unless it is a supported link and you continue.
* Access to YouTube is governed by YouTube's own terms of service. OpenWave is not affiliated with
  YouTube, Google, Spotify or LRCLIB.

## The local audio proxy

Playback goes through a small web server inside the app so the audio element never talks to
YouTube directly.

* It binds to `127.0.0.1` only (loopback), on a random port chosen at launch. It cannot be reached
  from other computers.
* Every address contains a random token generated at each launch (a UUID v4), and each audio
  address also carries a short-lived random ticket. Requests with a wrong token get `403`.
  Local web pages or other programs that do not know the token cannot read your audio.
* The `Host` header must be `127.0.0.1:<port>` or `localhost:<port>` (this blocks DNS-rebinding
  pages), every response is `X-Content-Type-Options: nosniff`, and CORS is answered only for the app's own web view origin
  (`http://dioxus.index.html`, echoed exactly, never `*`; needed so the sound effects can process the audio), so
  other web pages cannot read audio through the browser's cross-origin rules. Error replies are generic and
  never contain the (signed) stream address.
* An audio address stays valid while it is used and expires after 3 hours without use.
* It only fetches `https://*.googlevideo.com` (port 443) and the music files you added from your
  computer. Local files are checked again every time one is served: the extension must be an audio
  type (mp3, m4a, aac, flac, wav, ogg, opus) and the resolved path must still be a regular file, so
  a hand-edited `local-files.json` cannot make OpenWave serve other files.
  These rules are covered by unit tests in `src/backend/media_tests.rs` and
  `src/backend/media_hardening_tests.rs`.

## Changes to the window's web engine settings

To process the audio for the sound effects (speed, pitch, reverb) the page needs a "secure" context.
The app's own internal page address (`http://dioxus.index.html`, which exists only inside the app window
and is not a website) is not treated as secure by default, so OpenWave starts the embedded Edge WebView2
with one extra browser flag that marks exactly that address as secure
(`--unsafely-treat-insecure-origin-as-secure=http://dioxus.index.html`). In plain words: it changes how the
app window treats its own page, not what the app may contact. No website is affected, and the content
policy below still limits connections to `127.0.0.1`. The local audio proxy answers cross-origin
requests only for that same address.

## The window's content policy

The app window carries a Content-Security-Policy: it can load scripts and styles only from itself,
pictures only from the three cover hosts above (plus embedded data), audio only from the local proxy,
and it can open connections only to `127.0.0.1` (the proxy and the app's own link to its window). No
frames, plugins or form posts. (Inline scripts are allowed because the window toolkit itself injects
them at start-up; nothing is evaluated from text.)

## Files OpenWave writes

| File | Content |
|---|---|
| `settings.json`, `library.json` | Your settings and library. Written atomically; a damaged file is moved aside as `*.corrupt-<time>` and OpenWave starts clean |
| `cache/` | Optional song copies (size limit in Settings > Storage). Half-finished downloads (`*.part`) older than an hour are removed at start |
| `local-files.json` | Paths of the music files you added from your computer |
| `bin/yt-dlp.exe` | The audio engine, if you chose to download it |
