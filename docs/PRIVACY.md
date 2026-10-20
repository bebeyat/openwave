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
| `music.youtube.com` (YouTube Music home, explore and charts pages, no login) | When Home shows the Discover rows (at most every 6 hours, or when you pick a mood) | The app language and your computer's region setting (for example `tr` and `TR`); never your history or likes | Quick picks, new releases, trending songs, charts and top artists for your country. Turn the rows off under Settings > Customize > Home |
| `www.youtube.com` (`/youtubei/v1/visitor_id`) | Once when OpenWave starts, and again after about 2 hours | An anonymous client context (no cookies, no account, no song) | Gets an anonymous visitor id that the stream lookup needs; also opens the connection |
| `www.youtube.com` (`/youtubei/v1/player`) | When you play a song, and ahead of time for the next two queue entries, the "continue where you left off" song and the first Home cards | The song id, an anonymous client context and the anonymous visitor id (no cookies, no account). Only the stream address is requested ahead of time, never audio | Finds the audio stream address |
| `*.googlevideo.com` (HTTPS port 443 only) | While a song plays or is cached | Range requests for the audio of that song | The audio data itself. Only OpenWave's local proxy fetches it; redirects to any other host are refused |
| `i.ytimg.com`, `*.ggpht.com`, `*.googleusercontent.com` | When a cover picture is shown, when the app takes the colors of a cover, and for the Windows media overlay | (1) A normal image request from the app window. (2) A download of a small copy of the cover by OpenWave's own code (user agent `OpenWave/<version>`, https and these hosts only, no cookies) to pick the accent colors; the result is cached in `cache/palette.json`. (3) The Windows media overlay (volume pop-up, lock screen) receives the cover address and fetches the picture itself | Cover and artist pictures, the cover-colored look. The backend only hands out image URLs on these hosts |
| `lrclib.net` | **Only if you turn on lyrics** (Settings > Lyrics; off by default) | Song title, artist, album and length | Finds synchronized lyrics. With the setting off, the lyrics code returns before any request is made (unit test: `disabled_never_touches_the_network`) |
| `open.spotify.com` (`/embed/playlist/<id>`), then `api-partner.spotify.com` (`/pathfinder/v1/query`) | **Only while you import a Spotify link** | The playlist id from the link you pasted; for lists longer than 100 songs also the anonymous token the embed page hands out, to read the rest of the list (at most 500 songs) | Reads the public playlist page to get song names. No Spotify login or key. The page is read through at most 2 MB; anything larger is rejected as unreadable |
| `accounts.spotify.com`, `api.spotify.com` | **Only if you sign in** with your own Spotify app (Settings > General > Spotify Client ID) | The sign-in (OAuth with PKCE, through your browser and a one-time listener on `127.0.0.1:43821`), then token refreshes and, when you import from your account, requests for your playlists, a playlist's songs or Liked Songs | Imports your own playlists and Liked Songs. The tokens are kept in `spotify-auth.json`; **Sign out** deletes it |
| `itunes.apple.com` (`/lookup`), `music.apple.com` | **Only while you import an Apple Music link** | The album id (iTunes lookup) or the playlist page from the link you pasted | Reads the song names of a public album or playlist |
| `translate.googleapis.com` | **Only when you turn on Translate** in the lyrics (off at every start) | The lyric lines of the current song and the app language | Shows a translation under each line. Translations are kept in `lyrics/` |
| `soundcloud.com` and its media and image hosts (`*.sndcdn.com`), contacted by the **yt-dlp** program (images by the app window) | When you search on the SoundCloud tab, play or download a SoundCloud song, or when YouTube cannot play a song at all (then one search for "artist title") | The search text or the SoundCloud song id | SoundCloud search and playback. The song is saved in `cache/` (kept under 512 MB) and played from there |
| Nostr relays `relay.damus.io`, `nos.lol`, `relay.primal.net`, `nostr.mom`, `relay.nostr.net` (WebSocket over TLS) | **Only when you share** a playlist or song, or **open a share link** | Sharing: the song list (song ids, titles, artists, albums, covers, lengths) and the list name, compressed, as a NIP-78 event signed with a new random key that is thrown away afterwards (nothing ties two shares together). Opening: the 10-character share id. Files from your computer are never included | Short share links (`https://openwave.pages.dev/#<id>`) without an OpenWave server or account. **The list is public**: anyone with the link, and anyone browsing those relays, can read it. The id is a hash of the content, so a relay cannot swap the list for another one. Relays may delete old data, after which the link stops working |
| `github.com`, then `objects.githubusercontent.com` or `release-assets.githubusercontent.com` (the only two hosts GitHub redirects release files to; any other host is refused) | **Only after you agree** to download or update the audio engine | A plain request for the official `yt-dlp` release and its `SHA2-256SUMS` file | Installs yt-dlp. The download is checked against the published SHA-256 before it is installed; a mismatch installs nothing |
| `github.com/bebeyat/openwave/releases`, then `release-assets.githubusercontent.com` or `objects.githubusercontent.com` | When OpenWave starts (after 20 seconds) and every 6 hours, **unless you turn off** Settings > General > Update automatically | A request for `releases/latest` (only the redirect is read). If a newer version exists and this is the installed Windows app: its `SHA256SUMS` and `OpenWave-windows-x64-setup.exe` | Updates. The installer is checked against the published SHA-256, kept in `updates/` and run silently when you press **Update** or close the app |
| `www.youtube.com` and its media hosts, contacted by the **yt-dlp** program | Only when the direct stream lookup fails or is still pending after 1.2 seconds, and yt-dlp is installed (the slower of the two is cancelled); and for song searches when YouTube Music search fails | What yt-dlp normally sends to YouTube for one song id, or the search text | Fallback way to find the audio stream and songs. OpenWave runs it with `--ignore-config --no-plugin-dirs --no-cache-dir`, from OpenWave's own `bin` folder as working directory |

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
* **Share links in a browser.** `https://openwave.pages.dev` is a static page (Cloudflare Pages,
  source in `site/`). It reads the list from the relays above in the visitor's browser and offers to
  open OpenWave; the share id sits after `#`, which browsers never send to the page's server.
  Clicking "Open in OpenWave" starts the app with `openwave://s/<id>`.
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
pictures only from the three cover hosts above and SoundCloud's `*.sndcdn.com` (plus embedded data), audio only from the local proxy,
and it can open connections only to `127.0.0.1` (the proxy and the app's own link to its window). No
frames, plugins or form posts. (Inline scripts are allowed because the window toolkit itself injects
them at start-up; nothing is evaluated from text.)

## Files OpenWave writes

| File | Content |
|---|---|
| `settings.json`, `library.json` | Your settings and library. Written atomically; a damaged file is moved aside as `*.corrupt-<time>` and OpenWave starts clean |
| `cache/` | Optional song copies (size limit in Settings > Storage). Half-finished downloads (`*.part`) older than an hour are removed at start |
| `local-files.json` | Paths of the music files you added from your computer |
| `lyrics/` | Lyrics (and translations) already looked up, at most 500 songs |
| `spotify-auth.json` | Your Spotify sign-in tokens, only if you signed in; deleted by **Sign out** |
| `open-link.txt` | A share link passed from a second launch to the open window; deleted as soon as it is read |
| `bin/yt-dlp.exe` | The audio engine, if you chose to download it |
