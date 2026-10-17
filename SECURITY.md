# Security policy

## Supported versions

OpenWave is pre-1.0. Only the **latest released version** receives security fixes.

| Version | Supported |
|---|---|
| latest 0.x release | yes |
| older releases | no, please update |

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Report privately through GitHub's security advisories:
<https://github.com/bebeyat/openwave/security/advisories/new>
(this works once the repository is public and private vulnerability reporting is enabled). If that is not
available, open an issue that only asks for a private contact (no details of the problem), and a maintainer
will reach out.

Please include what you found, the version, steps to reproduce, and the impact you expect. You will get an
acknowledgement within about 7 days. OpenWave is maintained by volunteers, so fix times vary; we will keep you
informed and credit you in the release notes if you wish.

## Scope

In scope: the OpenWave application and its release artifacts, including the local audio proxy
(loopback only, per-launch token), the yt-dlp download and checksum verification, the import parsers, and
the installer. Out of scope: vulnerabilities in YouTube, Spotify, LRCLIB, yt-dlp or WebView2 themselves
(report those upstream), and social-engineering or physical-access scenarios.

What the app contacts on the network is documented in [docs/PRIVACY.md](docs/PRIVACY.md).
