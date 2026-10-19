# Changelog

All notable changes to OpenWave are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4] - 2026-10-20

### Fixed

- Playlists that contain a song without an artist page (for example the video charts on Home) opened with
  "unexpected response from the server". They now open; only a row that cannot be read at all is skipped.
- Playlist songs read this way keep their length.
- Settings > About no longer shows "unverified" as the version of a yt-dlp found on the computer.

## [0.1.3] - 2026-10-20

### Fixed

- Settings > About shows whether OpenWave itself has an update, with **Check for updates** and **Update** for the
  app (the buttons there were only for yt-dlp).
- The yt-dlp check no longer reports the installed version as a new one when yt-dlp answers slowly.
- Updating an all-users install (under Program Files) updates that copy instead of adding a per-user one.

## [0.1.2] - 2026-10-19

### Fixed

- Seeking: the position and volume bars show the right place again and a quick click on the bar seeks (0.1.1 kept
  the fill and the handle at the start).

## [0.1.1] - 2026-10-19

### Added

- Home: localized Discover rows (new releases, trending songs, charts, moods) for your country and language.
- Lyrics: word-by-word karaoke highlight with glow and smooth line transitions in full screen.
- Liked Songs pinned at the top of the sidebar playlists.
- Language picker uses the app menu and shows flags.
- Share links: "Share" on a playlist or in a song menu makes a short link (`https://openwave.pages.dev/#<id>`,
  38 characters whatever the list length) and copies it. Opening it starts OpenWave on the playlist preview
  (or plays the song); without OpenWave the page lists the songs. Lists are stored on public Nostr relays,
  no OpenWave server or account (see docs/PRIVACY.md). Clicking a song on the share page plays the list from
  that song in OpenWave.
- Automatic updates from GitHub releases. The installed Windows app downloads and verifies the new installer in
  the background and installs it from the **Update** button or when it closes; other builds link to the release.
  Can be turned off in Settings > General.

### Fixed

- macOS and Linux: audio plays (the window origin is allowed by the local proxy), the native yt-dlp build is
  downloaded and made executable, and a second copy of the app no longer starts.
- The queue and library are saved when the window closes.
- Data safety: local files list and downloads are written atomically; a damaged library is set aside with a
  notice; deleting all data stops downloads first.
- Search keeps "&" (R&B); album and artist pages reload when you switch between them; repeat-one skips a
  song that cannot play; fast shortcut presses are no longer dropped.
- Spotify rate limits are no longer reported as a private playlist.
- The title bar stays readable over bright and dark covers.

## [0.1.0] - 2026-10-17

First public version.

### Added

- Desktop music player for Windows 10/11 (x64), with experimental macOS (Apple Silicon) and Linux (x86_64)
  builds. Built with Rust and Dioxus 0.7.
- Interface in 15 languages; light and dark themes, accent colours from album covers, custom playlist covers
  and layout options.
- Search and playback of the YouTube Music catalog without any account, through a local loopback audio proxy;
  Home with recommendations, genre rows and personal mixes; artist, album and playlist pages; artist radio.
- Library: playlists, liked songs, listening history and local music files (MP3, M4A, FLAC and more, with tags
  and embedded covers); multi-select to queue, save, download or remove songs.
- Import of Spotify (public embed page, no account) and YouTube / YouTube Music playlist links, with preview
  and a report of songs that could not be matched.
- Offline downloads of songs, playlists or likes, with progress and cancellation.
- Sound effects: speed, pitch, reverb and bass boost, with presets such as *Slowed + reverb* and *Nightcore*.
- Queue, fullscreen player, optional synced lyrics from LRCLIB, optional Discord status, keyboard shortcuts,
  and Windows media keys and system media overlay.
- Optional yt-dlp audio engine, downloaded on request from the official release and verified by SHA-256.
- All data stored locally (for example `%APPDATA%\OpenWave` on Windows), with a wipe option; no telemetry,
  no analytics.
- Windows installer and portable zip, macOS app zip and Linux tarball, with SHA-256 checksums.

[Unreleased]: https://github.com/bebeyat/openwave/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/bebeyat/openwave/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/bebeyat/openwave/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/bebeyat/openwave/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/bebeyat/openwave/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/bebeyat/openwave/releases/tag/v0.1.0
