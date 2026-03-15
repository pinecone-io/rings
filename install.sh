#!/usr/bin/env bash
set -euo pipefail

REPO="jhamon/rings"
RELEASE="nightly"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
  Linux-x86_64)
    BINARY="rings-linux-x86_64"
    ;;
  *)
    echo "Unsupported platform: ${OS}-${ARCH}"
    echo "Download manually from: https://github.com/${REPO}/releases/tag/${RELEASE}"
    exit 1
    ;;
esac

URL="https://github.com/${REPO}/releases/download/${RELEASE}/${BINARY}"
DEST="${1:-/usr/local/bin/rings}"

echo "Downloading rings ${RELEASE} for ${OS}-${ARCH}..."
curl -fsSL "${URL}" -o /tmp/rings
chmod +x /tmp/rings

echo "Installing to ${DEST}..."
if [ -w "$(dirname "${DEST}")" ]; then
  mv /tmp/rings "${DEST}"
else
  sudo mv /tmp/rings "${DEST}"
fi

echo "rings installed to ${DEST}"
rings --version
