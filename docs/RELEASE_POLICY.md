# Release Policy

A release can be cut only when all required gates pass in CI.

## Required Gates

1. Fast PR gate:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --lib`
- `cargo test --test streaming_parser --test input_keybindings --test smoke_boot`

2. Full gate (merge/nightly and release):
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`

3. Release build matrix:
- Linux `x86_64-unknown-linux-gnu`
- Linux `aarch64-unknown-linux-gnu`
- macOS `x86_64-apple-darwin`
- macOS `aarch64-apple-darwin`
- Windows `x86_64-pc-windows-msvc`

4. Release artifacts:
- Platform archives uploaded to GitHub Releases
- `SHA256SUMS.txt` generated and uploaded

## Changelog Policy

- Changelog entries are generated with `git-cliff` using `.git-cliff.toml`.
- Local preparation command:

```bash
./scripts/release.sh
```

PowerShell:

```powershell
./scripts/release.ps1
```

## crates.io Publishing

- Publish only after release gates pass and artifacts are verified.
- Command:

```bash
cargo publish
```

- Installation target for users:

```bash
cargo install olly
```
