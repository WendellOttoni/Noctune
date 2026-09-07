#!/bin/sh
set -e

REPO="WendellOttoni/Noctune"
BIN_DIR="$HOME/.local/bin"

OS=$(uname -s)
ARCH=$(uname -m)

case "$OS-$ARCH" in
  Linux-x86_64)  ARTIFACT="noctune-linux-x64" ;;
  Darwin-arm64)  ARTIFACT="noctune-macos-arm64" ;;
  Darwin-x86_64) ARTIFACT="noctune-macos-x64" ;;
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
STAGING=$(mktemp -d "$BIN_DIR/.noctune-install.XXXXXX")
trap 'rm -f "$STAGING/$ARTIFACT" "$STAGING/checksum"; rmdir "$STAGING"' EXIT HUP INT TERM
curl -fsSL "$URL" -o "$STAGING/$ARTIFACT"
curl -fsSL "$URL.sha256" -o "$STAGING/checksum"
EXPECTED=$(awk 'NR == 1 {print $1}' "$STAGING/checksum")
NAME=$(awk 'NR == 1 {print $2}' "$STAGING/checksum")
[ "$NAME" = "$ARTIFACT" ] && [ "${#EXPECTED}" = 64 ] || { echo "Invalid checksum manifest"; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL=$(sha256sum "$STAGING/$ARTIFACT" | awk '{print $1}')
else
  ACTUAL=$(shasum -a 256 "$STAGING/$ARTIFACT" | awk '{print $1}')
fi
[ "$ACTUAL" = "$EXPECTED" ] || { echo "Checksum mismatch; installation preserved"; exit 1; }
chmod +x "$STAGING/$ARTIFACT"
if [ -f "$BIN_DIR/noctune" ]; then cp -p "$BIN_DIR/noctune" "$BIN_DIR/noctune.previous"; fi
mv -f "$STAGING/$ARTIFACT" "$BIN_DIR/noctune"

if echo ":$PATH:" | grep -q ":$BIN_DIR:"; then
  echo "noctune $VERSION installed. Run: noctune"
else
  echo "noctune $VERSION installed to $BIN_DIR"
  echo ""
  echo "Add this to your ~/.bashrc or ~/.zshrc:"
  echo '  export PATH="$HOME/.local/bin:$PATH"'
fi
