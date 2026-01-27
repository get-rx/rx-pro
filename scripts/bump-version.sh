#!/bin/bash
# Bump version across all crates and pro-python
# Usage: ./scripts/bump-version.sh <new-version>
# Example: ./scripts/bump-version.sh 0.2.0

set -e

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [ -z "$1" ]; then
    # Show current versions
    echo "Current versions:"
    grep -A1 '\[workspace.package\]' Cargo.toml | grep version | head -1 | sed 's/^/  Workspace: /'
    grep '^version' pro-python/Cargo.toml | head -1 | sed 's/^/  pro-python Cargo.toml: /'
    grep '^version' pro-python/pyproject.toml | head -1 | sed 's/^/  pro-python pyproject.toml: /'
    echo ""
    echo "Usage: $0 <new-version>"
    echo "Example: $0 0.2.0"
    exit 1
fi

NEW_VERSION="$1"

# Get current version from workspace
CURRENT_VERSION=$(grep -A1 '\[workspace.package\]' Cargo.toml | grep 'version = ' | sed 's/.*"\(.*\)".*/\1/')

if [ "$CURRENT_VERSION" = "$NEW_VERSION" ]; then
    echo "Version is already $NEW_VERSION"
    exit 0
fi

echo "Bumping version: $CURRENT_VERSION -> $NEW_VERSION"
echo ""

# 1. Update root Cargo.toml (workspace version + internal crate deps)
echo "Updating Cargo.toml..."
sed -i.bak \
    -e "s/^\(version = \"\)$CURRENT_VERSION\(\".*\)$/\1$NEW_VERSION\2/" \
    -e "s/\(pro-core.*version = \"\)$CURRENT_VERSION\(\".*\)$/\1$NEW_VERSION\2/" \
    -e "s/\(pro-plugin.*version = \"\)$CURRENT_VERSION\(\".*\)$/\1$NEW_VERSION\2/" \
    Cargo.toml
rm -f Cargo.toml.bak

# 2. Update pro-python Cargo.toml
echo "Updating pro-python/Cargo.toml..."
sed -i.bak "s/^\(version = \"\)[^\"]*\(\".*\)$/\1$NEW_VERSION\2/" pro-python/Cargo.toml
rm -f pro-python/Cargo.toml.bak

# 3. Update pro-python pyproject.toml
echo "Updating pro-python/pyproject.toml..."
sed -i.bak "s/^\(version = \"\)[^\"]*\(\".*\)$/\1$NEW_VERSION\2/" pro-python/pyproject.toml
rm -f pro-python/pyproject.toml.bak

echo ""
echo "Verification:"
grep -A1 '\[workspace.package\]' Cargo.toml | grep version | head -1 | sed 's/^/  /'
grep 'pro-core.*version' Cargo.toml | sed 's/^/  /'
grep 'pro-plugin.*version' Cargo.toml | sed 's/^/  /'
grep '^version' pro-python/Cargo.toml | head -1 | sed 's/^/  pro-python: /'
grep '^version' pro-python/pyproject.toml | head -1 | sed 's/^/  pyproject: /'
echo ""
echo "Next steps:"
echo "  git add Cargo.toml pro-python/Cargo.toml pro-python/pyproject.toml"
echo "  git commit -m 'chore: bump version to $NEW_VERSION for release'"
echo "  git tag v$NEW_VERSION"
echo "  git push origin main --tags"
