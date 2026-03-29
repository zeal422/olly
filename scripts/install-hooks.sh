#!/usr/bin/env bash
set -euo pipefail

hook_path=".git/hooks/pre-commit"
cat > "$hook_path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
./scripts/check.sh
EOF

chmod +x "$hook_path"
echo "Installed pre-commit hook at $hook_path"
