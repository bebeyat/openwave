# Changelog

All notable changes to OpenWave are documented here.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - UNRELEASED

First public version (date to be filled in when the release is tagged).

### Added

- Windows desktop player built with Rust and Dioxus 0.7 (WebView2), Turkish and English interface,
  light and dark themes that follow the system.
- Search and playback of the YouTube Music catalog without any account, with a local loopback audio proxy.
- Library: playlists, liked songs, listening history, local music files.
- Import of Spotify (public embed page, no account) and YouTube / YouTube Music playlist links, with preview
  and a report of songs that could not be matched.
- Queue, fullscreen player, optional synced lyrics (off by default), Windows media keys and system media overlay.
- Optional yt-dlp audio engine, downloaded on request from the official release and verified by SHA-256.
- All data stored locally in `%APPDATA%\OpenWave`; no telemetry, no analytics.
- Open-source release files: MIT OR Apache-2.0 licenses, third-party notices, privacy statement,
  contribution guide, CI and release workflows, portable zip and installer definitions.

[Unreleased]: https://github.com/bebeyat/openwave/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/bebeyat/openwave/releases/tag/v0.1.0
