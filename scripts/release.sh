#!/usr/bin/env bash
set -euo pipefail

./scripts/check.sh

if ! command -v git-cliff >/dev/null 2>&1; then
  echo "git-cliff is required to generate the changelog."
  echo "Install: cargo install git-cliff"
  exit 1
fi

git-cliff --config .git-cliff.toml --output CHANGELOG.md
cargo package

echo "Release preparation complete."
echo "Next: tag with vX.Y.Z and push to trigger .github/workflows/release.yml."
