# Changelog

All notable changes to OpenWave are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Share links: "Share" on a playlist or in a song menu makes a short link (`https://openwave.pages.dev/#<id>`,
  38 characters whatever the list length) and copies it. Opening it starts OpenWave on the playlist preview
  (or plays the song); without OpenWave the page lists the songs. Lists are stored on public Nostr relays,
  no OpenWave server or account (see docs/PRIVACY.md).

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

[Unreleased]: https://github.com/bebeyat/openwave/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bebeyat/openwave/releases/tag/v0.1.0
