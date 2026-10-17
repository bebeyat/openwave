## What and why

<!-- What does this change, and why? Link the issue. Add light and dark screenshots for UI changes. -->

## Checklist

- [ ] Focused change, described above
- [ ] I wrote this myself or it comes from a compatible-licensed source recorded in `THIRD_PARTY_NOTICES.md`; nothing copied from GPL or other incompatible apps (clean-room rule, see CONTRIBUTING.md)
- [ ] Commits are signed off (`git commit -s`); I agree my contribution is dual-licensed MIT OR Apache-2.0
- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` pass
- [ ] Tests added or updated; new UI strings exist in both `tr` and `en`
- [ ] `assets/main.css` rebuilt if classes changed (`npm run css` in `tools/`)
- [ ] `docs/PRIVACY.md` updated if a network request was added or changed
- [ ] `CHANGELOG.md` updated under "Unreleased"
