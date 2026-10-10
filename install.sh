#!/bin/sh

set -eu

APP="orbis"
REPO="BuboTerrae/Orbis"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
    Linux-x86_64)                ASSET="orbis-x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)               ASSET="orbis-aarch64-unknown-linux-gnu" ;;
    Darwin-x86_64)               ASSET="orbis-x86_64-apple-darwin" ;;
    Darwin-arm64|Darwin-aarch64) ASSET="orbis-aarch64-apple-darwin" ;;
    MINGW*|MSYS*|CYGWIN*-x86_64) ASSET="orbis-x86_64-pc-windows-msvc.exe" ;;
    *)
        echo "Unsupported platform: $OS-$ARCH"
        echo "Supported: Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64"
        exit 1
        ;;
esac

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo -e "\e[38;5;20mFetching latest release info\033[0m..."
LATEST_URL="https://api.github.com/repos/$REPO/releases/latest"
ASSET_URL=$(curl -fsSL "$LATEST_URL" \
    | grep -o "\"browser_download_url\": \"[^\"]*$ASSET[^\"]*\"" \
    | head -1 \
    | cut -d'"' -f4)

if [ -z "$ASSET_URL" ]; then
    echo "Could not find release asset for $ASSET"
    echo "Check https://github.com/$REPO/releases for available assets"
    exit 1
fi

echo -e "\e[38;5;27mDownloading $ASSET\033[0m..."
curl -fsSL --progress-bar "$ASSET_URL" -o "$TMP_DIR/$APP"

chmod +x "$TMP_DIR/$APP"

INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
    echo "Installing to $INSTALL_DIR (requires sudo)..."
    sudo install -m 755 "$TMP_DIR/$APP" "$INSTALL_DIR/$APP"
else
    install -m 755 "$TMP_DIR/$APP" "$INSTALL_DIR/$APP"
fi

echo -e "\e[38;5;150m✓ Installed\033[0m \e[38;5;20m$APP\033[0m to $INSTALL_DIR/$APP"
echo -e "Run \e[38;5;150m'$APP --help'\033[0m to get started"