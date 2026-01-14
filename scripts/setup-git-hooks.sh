#!/usr/bin/env bash
# Install git hooks for this repository
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_SRC="$SCRIPT_DIR/git-hooks"
HOOKS_DST="$PROJECT_ROOT/.git/hooks"

echo "Installing git hooks..."

# Ensure hooks directory exists
mkdir -p "$HOOKS_DST"

# Install pre-push hook
if [ -f "$HOOKS_SRC/pre-push" ]; then
    cp "$HOOKS_SRC/pre-push" "$HOOKS_DST/pre-push"
    chmod +x "$HOOKS_DST/pre-push"
    echo "✅ Installed pre-push hook"
fi

echo ""
echo "Git hooks installed! The pre-push hook will verify dependency hashes before pushing."
echo ""
echo "To update hashes if they're outdated, run:"
echo "  ./scripts/update-git-deps.sh"

