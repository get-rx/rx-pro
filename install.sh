#!/bin/bash
# Pro installer script
# Usage: curl -sSf https://raw.githubusercontent.com/pro-rx/rx/main/install.sh | bash

set -e

REPO="pro-rx/rx"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)
                TARGET="x86_64-unknown-linux-gnu"
                ;;
            aarch64)
                TARGET="aarch64-unknown-linux-gnu"
                ;;
            *)
                echo "Unsupported architecture: $ARCH"
                exit 1
                ;;
        esac
        ;;
    Darwin)
        case "$ARCH" in
            x86_64)
                TARGET="x86_64-apple-darwin"
                ;;
            arm64)
                TARGET="aarch64-apple-darwin"
                ;;
            *)
                echo "Unsupported architecture: $ARCH"
                exit 1
                ;;
        esac
        ;;
    MINGW*|MSYS*|CYGWIN*)
        TARGET="x86_64-pc-windows-msvc"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

echo "Detected: $OS $ARCH -> $TARGET"

# Get latest release
LATEST_RELEASE=$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
    echo "Failed to get latest release"
    exit 1
fi

echo "Latest release: $LATEST_RELEASE"

# Download URL
if [ "$OS" = "MINGW"* ] || [ "$OS" = "MSYS"* ] || [ "$OS" = "CYGWIN"* ]; then
    ARCHIVE="rx-$TARGET.zip"
else
    ARCHIVE="rx-$TARGET.tar.gz"
fi

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_RELEASE/$ARCHIVE"

echo "Downloading $DOWNLOAD_URL..."

# Create temp directory
TMP_DIR=$(mktemp -d)
trap "rm -rf $TMP_DIR" EXIT

cd "$TMP_DIR"

# Download
curl -sSfL "$DOWNLOAD_URL" -o "$ARCHIVE"

# Extract
if [[ "$ARCHIVE" == *.zip ]]; then
    unzip -q "$ARCHIVE"
else
    tar xzf "$ARCHIVE"
fi

# Install
mkdir -p "$INSTALL_DIR"

if [ -f "rx" ]; then
    mv rx "$INSTALL_DIR/"
elif [ -f "rx.exe" ]; then
    mv rx.exe "$INSTALL_DIR/"
else
    echo "Binary not found in archive"
    exit 1
fi

chmod +x "$INSTALL_DIR/rx" 2>/dev/null || true

echo ""
echo "Pro installed to $INSTALL_DIR/rx"
echo ""

# Check if in PATH
if ! command -v rx &> /dev/null; then
    echo "Add $INSTALL_DIR to your PATH:"
    echo ""
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    echo ""
    echo "Add this to your ~/.bashrc or ~/.zshrc for persistence."
else
    echo "Run 'rx --help' to get started!"
fi
