# Contributing

Keep changes focused and preserve the game's native mod data, serialized UI
structure, and restart behavior.

Before submitting a change:

```powershell
.\scripts\validate_repo.ps1
cargo test --release
cargo clippy --release --all-targets -- -D warnings
```

Test UI changes at multiple window sizes. Treat manifests and action files as
untrusted input, and do not broaden the supported game range without testing
the exact release DLL.
