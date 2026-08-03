## Summary

Describe the focused change and the player or mod-author problem it solves.

## Verification

- [ ] `.\scripts\validate_repo.ps1`
- [ ] `cargo fmt --check`
- [ ] `cargo test --release` with the matching TFM2 Mod SDK
- [ ] Tested affected UI at more than one window size
- [ ] Tested malformed or missing optional integration data where relevant
- [ ] Updated `CHANGELOG.md` under **Unreleased**

## Compatibility and safety

Describe file writes, action handling, path validation, serialized UI changes,
and game-version testing. Write “None” where an item does not apply.
