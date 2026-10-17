# Contributing to OpenWave

Thanks for helping. OpenWave is a small project; clear, focused pull requests are the best gift.
By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Clean-room provenance rule (important)

OpenWave is a clean-room project published under **MIT OR Apache-2.0**. To keep it that way:

- **Do not copy code, text, strings, icons or assets from other applications** unless their license is
  compatible and you record it (see below). In particular never copy from GPL, AGPL, LGPL, PolyForm,
  BSD-4 or "source-available" players. Do not paste code from apps you merely used or read.
- Do not copy from proprietary software or from decompiled or leaked sources.
- Write your own implementation from public documentation and your own understanding. Ideas and general
  behaviour are free; expression is not.
- Every new dependency or bundled asset (crate, font, icon, image) needs a license that is compatible
  with MIT OR Apache-2.0 distribution. Add it in the same pull request and regenerate
  `THIRD_PARTY_NOTICES.md` (see "Dependencies" below).
- UI strings are written fresh (Turkish and English); do not lift wording from other apps.

## Your contribution is dual-licensed

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in OpenWave
by you, as defined in the Apache-2.0 license, shall be dual licensed under MIT OR Apache-2.0, without any
additional terms or conditions.

We use a simple Developer Certificate of Origin (DCO 1.1) statement instead of a CLA. Sign off each commit
with `git commit -s`, which adds `Signed-off-by: Your Name <you@example.com>` and means you certify that:
you wrote the contribution (or have the right to submit it under the project license), and you understand
it is public and kept with your sign-off. The full text is at <https://developercertificate.org/>.

## Setup

Windows 10/11, MSVC build tools, Rust 1.95 (installed by `rustup` from `rust-toolchain.toml`), Node.js.

```powershell
cd tools; npm ci; cd ..
cargo run
```

If you change anything Tailwind scans (`src/**/*.rs` class names, `tools/tailwind.css`), rebuild the CSS
and commit the result, because `assets/main.css` is committed and CI fails when it is stale:

```powershell
cd tools; npm run css; cd ..
```

## Checks (run before opening a pull request)

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check          # needs: cargo install cargo-deny --locked
```

Code quality bar: files under ~400 lines, functions under ~50 lines, immutable
updates, no `unwrap()` outside tests, errors shown to users in plain words, non-UI logic covered by tests.
Keyboard use and accessibility labels matter: every control needs a name, a tooltip where helpful, and a
visible focus ring.

## Adding or changing translations (i18n)

Strings live in `assets/locales/tr.json` and `assets/locales/en.json` and are read through typed keys in
`src/i18n/`. A key must exist in **both** languages. Add keys with the helper
instead of editing JSON by hand:

```powershell
# keys.json: { "tr": { "settings.example": "Örnek" }, "en": { "settings.example": "Example" } }
node tools/add-keys.mjs keys.json
```

The tool sorts keys, writes atomically and refuses to overwrite an existing key with a different value
unless you pass `--force`. A new language is welcome: open an issue first so we agree on the plan.

## Dependencies

After changing `Cargo.toml`/`Cargo.lock`:

```powershell
cargo deny check
node scripts/gen-notices.mjs --check   # regenerates THIRD_PARTY_NOTICES.md, fails on non-permissive licenses
```

## Pull request checklist

- [ ] The change is focused and described (what and why, screenshots for UI changes, light and dark).
- [ ] I wrote this code myself or it comes from a compatible-licensed source that is recorded in `THIRD_PARTY_NOTICES.md`; nothing is copied from GPL or other incompatible apps.
- [ ] Commits are signed off (`git commit -s`).
- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- [ ] New or changed logic has tests; UI strings exist in both `tr` and `en`.
- [ ] `assets/main.css` is rebuilt if classes changed (`npm run css` in `tools/`).
- [ ] `docs/PRIVACY.md` is updated if the change adds or alters any network request.
- [ ] `CHANGELOG.md` has an entry under "Unreleased" for user-visible changes.

## Reporting bugs and ideas

Use the issue templates. For security problems do **not** open a public issue; see [SECURITY.md](SECURITY.md).
