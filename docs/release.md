# Release Process

How to build, version, and release Olly.

## Prerequisites

- Rust stable toolchain (see `rust-toolchain.toml`)
- Inno Setup 6 (for Windows installer)
- `git-cliff` (for changelog generation)

## Building

### Debug Build

```bash
cargo build
```

Output: `target/debug/olly.exe`

### Release Build

```bash
cargo build --release
```

Output: `target/release/olly.exe`

### Running

```bash
# With defaults
cargo run

# With custom Ollama URL and model
cargo run -- --url http://localhost:11434 --model llama3.2:latest
```

## Quality Checks

Before any release:

```powershell
# PowerShell
./scripts/check.ps1

# Bash
./scripts/check.sh
```

This runs:
1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo build`
4. `cargo test --all-targets --all-features`

### Pre-Release Checklist Scripts

| Script | Purpose |
|--------|---------|
| `scripts/check.ps1` | Full quality gate (fmt + clippy + build + test) |
| `scripts/fmt-fix.ps1` | Auto-apply formatting and clippy fixes |
| `scripts/audit.ps1` | Check dependencies for vulnerabilities |
| `scripts/coverage.ps1 --html` | Generate coverage report |
| `scripts/docs-check.ps1` | Verify documentation health |
| `scripts/loc.ps1` | Count lines per module |

## Building Windows Installer

```powershell
./scripts/build-installer.ps1
```

Requirements:
- Inno Setup 6 (`ISCC.exe`) must be installed
- Release build must exist in `target/release/`

Output: `dist/windows-installer/Olly-Setup-<version>.exe`

## Version Management

### Current Version

Defined in `Cargo.toml`:

```toml
[package]
name = "olly"
version = "0.0.13"
```

### Version Bump Process

1. Update `Cargo.toml` version
2. Update `CHANGELOG.md` with new version section
3. Update installer script if version changed
4. Run full quality checks
5. Commit with message: `release: v0.0.X`

## Changelog

Changelog is maintained in `CHANGELOG.md` using Keep a Changelog format.

### Adding Entries

Add entries under the `[Unreleased]` section during development. On release:

1. Change `[Unreleased]` to `[<version>] - <date>`
2. Add new `[Unreleased]` section above it

### Changelog Generation

```bash
# Generate changelog from git history
git-cliff --config .git-cliff.toml

# Generate for specific range
git-cliff --config .git-cliff.toml <old>..<new>
```

## Release Scripts

### Automated Release

```powershell
# PowerShell
./scripts/release.ps1

# Bash
./scripts/release.sh
```

Uses `git-cliff` with `.git-cliff.toml` for changelog generation.

### Manual Release Checklist

- [ ] All quality checks pass (`./scripts/check.ps1`)
- [ ] Version bumped in `Cargo.toml`
- [ ] `CHANGELOG.md` updated with new version section
- [ ] Installer builds successfully (`./scripts/build-installer.ps1`)
- [ ] Manual smoke test:
  - [ ] App launches correctly
  - [ ] Chat with model works
  - [ ] Text selection and copy works
  - [ ] Theme switching works
  - [ ] Workspace/session operations work
  - [ ] Tool execution works
- [ ] Git tag created: `git tag v0.0.X`
- [ ] Git push: `git push origin main --tags`

## Distribution

### Windows

- Installer: `dist/windows-installer/Olly-Setup-<version>.exe`
- Portable: `target/release/olly.exe`

### Linux/macOS

- Build from source: `cargo build --release`
- Binary: `target/release/olly`

## Configuration

### Config File Location

- **Linux**: `~/.config/olly/config.toml`
- **macOS**: `~/Library/Application Support/olly/config.toml`
- **Windows**: `%APPDATA%\olly\config.toml`

### Data Storage

- **Linux**: `~/.local/share/olly/`
- **macOS**: `~/Library/Application Support/olly/`
- **Windows**: `%LOCALAPPDATA%\olly/`

### UI State

Persisted to `<data_dir>/ui_state.json` on shutdown.
