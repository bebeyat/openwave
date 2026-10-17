# Releasing OpenWave

**Every step on this page is performed by the repository owner.** Nothing here is automated to publish by
itself: the release workflow only creates a *draft* release, and nothing leaves your machine until you push.

## 0. Placeholders to replace

The repository is set to `bebeyat/openwave` everywhere (README links and download buttons, `Cargo.toml`,
`src/features/settings/about.rs`, the installer, issue templates). If the repository is renamed, replace that
string in all files.

There are no other placeholders: security and conduct reports go through GitHub (see `SECURITY.md`).

## 1. Decide things only you can decide

- [ ] License choice is MIT OR Apache-2.0, holder `meganek31`, year 2026 (already written; change if you disagree).
- [ ] Code signing: builds are unsigned (SmartScreen warning documented in the README). Decide whether to sign later.
- [ ] Enable "Private vulnerability reporting" in the repository settings (SECURITY.md relies on it).

## 2. Check the screenshots

The README uses the images and GIFs in `docs/media/` (light theme by default, a few dark ones, and
`split-light-dark.png`). They were captured from a demo library of popular songs; recapture them when the UI
changes so they show the current app.

The download buttons link to fixed asset names under `releases/latest/download/`
(`OpenWave-windows-x64-setup.exe`, `OpenWave-windows-x64-portable.zip`, `OpenWave-macos-arm64.zip`,
`OpenWave-linux-x86_64.tar.gz`). The release workflow uploads these copies; keep the names unchanged.

## 3. Run every check locally

```powershell
cd tools; npm ci; npm run css; cd ..
git diff --exit-code assets/main.css
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo deny check
cargo audit
node scripts/gen-notices.mjs --check     # rewrites THIRD_PARTY_NOTICES.md; commit it if it changed
cargo build --release --locked
```

Run `target\release\openwave.exe` and click through first run, search, play, import, settings and the yt-dlp
download banner. Optionally build the installer locally (needs Inno Setup 6):

```powershell
New-Item -ItemType Directory -Force stage\OpenWave, dist | Out-Null
Copy-Item target\release\openwave.exe, README.md, LICENSE-MIT, LICENSE-APACHE, THIRD_PARTY_NOTICES.md stage\OpenWave\
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" /DAppVersion=0.1.0 /DStageDir=$PWD\stage\OpenWave /DOutDir=$PWD\dist installer\openwave.iss
```

Test it on a clean Windows user: install, launch, uninstall (the data question should default to "No").

## 4. Prepare the version

1. Set `version` in `Cargo.toml` (the workflow refuses a tag that does not match it).
2. In `CHANGELOG.md` move "Unreleased" entries under the new version and set its date.
3. Commit: `git commit -s -m "chore: release 0.1.0"`.

## 5. Publish the repository (first time only)

```powershell
git remote add origin https://github.com/yourname/openwave.git
git push -u origin main
```

Before pushing, do a last sweep for anything private (local paths such as `D:\`, names of private or
license-restricted source projects, personal e-mail addresses) and check the `git log` author details.
Then in the repository settings enable Dependabot alerts and, if wanted, Discussions.

## 6. Tag and let CI create a DRAFT release

```powershell
git tag -s v0.1.0 -m "OpenWave 0.1.0"    # or: git tag -a v0.1.0 -m "OpenWave 0.1.0"
git push origin v0.1.0
```

`.github/workflows/release.yml` builds the exe, the portable zip, the installer and `SHA256SUMS`, then
creates a **draft** release. Watch the workflow under the Actions tab.

## 7. Review and publish the draft

1. Open the draft under Releases. Download both artifacts and compare them with `SHA256SUMS`.
2. Install the installer, run the portable exe, confirm the version in Settings > About.
3. Paste the `CHANGELOG.md` section into the release notes.
4. Click **Publish release**. This is the moment the release becomes public.

## If something goes wrong

Delete the draft release and the tag (`git push origin :refs/tags/v0.1.0`; `git tag -d v0.1.0`), fix, and tag
again. Never move a tag that was already published.

## Notes

- Actions in the workflows are pinned by major version (`@v4`). For stricter supply-chain hygiene, replace them
  with full commit SHAs (Dependabot will keep them updated).
- The workflows in `.github/workflows/` have not been executed yet; expect to fix small issues on the first run.
- The Windows executable icon is embedded by `build.rs` (winresource).
