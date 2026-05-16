#!/bin/sh
set -e

REPO="WendellOttoni/Noctune"
BIN_DIR="$HOME/.local/bin"

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS-$ARCH" in
  Linux-x86_64)  ARTIFACT="noctune-linux-x64" ;;
  Darwin-arm64)  ARTIFACT="noctune-macos-arm64" ;;
  Darwin-x86_64)
    echo "Intel Mac detected — downloading ARM binary (requires Rosetta 2)."
    ARTIFACT="noctune-macos-arm64"
    ;;
  *)
    echo "Unsupported platform: $OS/$ARCH"
    exit 1
    ;;
esac

echo "Fetching latest release..."
VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

URL="https://github.com/$REPO/releases/download/$VERSION/$ARTIFACT"

echo "Downloading noctune $VERSION..."
mkdir -p "$BIN_DIR"
curl -fsSL "$URL" -o "$BIN_DIR/noctune"
chmod +x "$BIN_DIR/noctune"

if echo ":$PATH:" | grep -q ":$BIN_DIR:"; then
  echo "noctune $VERSION installed. Run: noctune"
else
  echo "noctune $VERSION installed to $BIN_DIR"
  echo ""
  echo "Add this to your ~/.bashrc or ~/.zshrc:"
  echo '  export PATH="$HOME/.local/bin:$PATH"'
fi
