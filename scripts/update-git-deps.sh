#!/usr/bin/env bash
# Update git dependency hashes by attempting a build and parsing errors
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
GIT_DEPS_FILE="$PROJECT_ROOT/git-deps.nix"

echo "Checking git dependency hashes..."

# Try to build and capture output
BUILD_OUTPUT=$(nix build --no-link 2>&1) && {
    echo "✅ All hashes are up to date"
    exit 0
} || true

# Check for hash mismatch errors
if echo "$BUILD_OUTPUT" | grep -q "hash mismatch in fixed-output derivation"; then
    echo "🔄 Hash mismatch detected, updating..."
    
    # Extract the new hash from the error
    # Format: "got:    sha256-XXXX"
    NEW_HASH=$(echo "$BUILD_OUTPUT" | grep -oP "got:\s+\Ksha256-[A-Za-z0-9+/=]+")
    
    if [ -z "$NEW_HASH" ]; then
        echo "❌ Could not parse new hash from build output"
        echo "$BUILD_OUTPUT"
        exit 1
    fi
    
    echo "New hash: $NEW_HASH"
    
    # Determine which dependency needs updating based on the derivation name
    if echo "$BUILD_OUTPUT" | grep -q "whisper-rs"; then
        echo "Updating whisper-rs hash..."
        sed -i "s|\"whisper-rs\" = \"sha256-[^\"]*\"|\"whisper-rs\" = \"$NEW_HASH\"|" "$GIT_DEPS_FILE"
    else
        echo "❌ Unknown dependency with hash mismatch"
        echo "$BUILD_OUTPUT"
        exit 1
    fi
    
    echo "✅ Updated $GIT_DEPS_FILE"
    echo ""
    echo "Changes made:"
    git diff "$GIT_DEPS_FILE" || true
    echo ""
    echo "Run 'git add git-deps.nix' and commit the change"
else
    echo "❌ Build failed for reasons other than hash mismatch:"
    echo "$BUILD_OUTPUT"
    exit 1
fi

