# Contributing to Olly

Thanks for contributing. This guide keeps pull requests consistent and easy to review.

## Development Setup

1. Install Rust stable toolchain.
2. Clone the repository.
3. Build once to validate setup:

```bash
cargo build
```

## Required Quality Gates

Before opening a PR, run:

```bash
./scripts/check.sh
```

PowerShell:

```powershell
./scripts/check.ps1
```

These gates are required:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`

## Optional Pre-Commit Hook

Install a pre-commit hook so checks run automatically before each commit.

Bash:

```bash
./scripts/install-hooks.sh
```

PowerShell:

```powershell
./scripts/install-hooks.ps1
```

## Test Expectations

- Add or update tests for behavior changes.
- Every bug fix should include a reproducer test.
- Keep shared fixtures in `tests/common/` deterministic (no real network calls).

## Pull Request Guidelines

- Keep PRs focused and scoped.
- Use clear commit messages.
- Document user-visible behavior changes.
- If config/commands change, update `README.md`.

## Release Policy

Release process details are in [docs/release.md](docs/release.md).

## Security

Security model and tool execution safety details are in [docs/security.md](docs/security.md).

## License

By contributing, you agree that your contributions are provided to VectorMedia for inclusion
in a proprietary codebase and may be used, modified, and distributed by VectorMedia under
the project license.
