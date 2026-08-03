# Contributing

Focused bug fixes, compatibility findings, documentation improvements, and
small integration improvements are welcome.

## Before opening an issue

- Use the bug template for player-facing problems.
- Use the integration-help template for manifest or profile questions.
- Include the game and Better Mod Menu versions.
- Reproduce with the smallest practical enabled-mod list.
- Remove private paths, contacts, save names, and personal data.

The [modder guide](MODDER_GUIDE.md) covers the supported integration path. New
manifest behavior should solve a concrete mod-author need without making core
mod behavior depend on Better Mod Menu.

## Pull requests

- Keep changes focused and preserve the game's native mod data and restart flow.
- Treat manifests, settings, and action files as untrusted input.
- Preserve serialized UI references unless a controlled runtime test proves a
  change safe.
- Test UI changes at multiple window sizes.
- Update `CHANGELOG.md` under **Unreleased**.

Run before submitting:

```powershell
.\scripts\validate_repo.ps1
cargo fmt --check
cargo test --release
cargo clippy --release --all-targets -- -D warnings
```

Native Rust checks require the matching Teamfight Manager 2 Mod SDK.
