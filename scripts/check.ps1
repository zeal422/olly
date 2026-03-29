$ErrorActionPreference = "Stop"

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cargo test --all-targets --all-features
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
