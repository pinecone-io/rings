#!/usr/bin/env bash
set -euo pipefail

REPO="pinecone-io/rings"
RELEASE="nightly"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)  OS_TAG="linux" ;;
  Darwin) OS_TAG="macos" ;;
  *)
    echo "Error: Unsupported OS: ${OS}"
    echo "Download manually from: https://github.com/${REPO}/releases/tag/${RELEASE}"
    exit 1
    ;;
esac

case "${ARCH}" in
  x86_64|amd64)  ARCH_TAG="x86_64" ;;
  aarch64|arm64) ARCH_TAG="aarch64" ;;
  *)
    echo "Error: Unsupported architecture: ${ARCH}"
    echo "Download manually from: https://github.com/${REPO}/releases/tag/${RELEASE}"
    exit 1
    ;;
esac

BINARY="rings-${OS_TAG}-${ARCH_TAG}"
URL="https://github.com/${REPO}/releases/download/${RELEASE}/${BINARY}"
CHECKSUMS_URL="https://github.com/${REPO}/releases/download/${RELEASE}/checksums-sha256.txt"
DEST="${1:-/usr/local/bin/rings}"

echo "Installing rings (${OS_TAG}-${ARCH_TAG})..."

# Download binary
curl -fsSL "${URL}" -o /tmp/rings-download
chmod +x /tmp/rings-download

# Verify checksum
if command -v sha256sum &>/dev/null; then
  SHA_CMD="sha256sum"
elif command -v shasum &>/dev/null; then
  SHA_CMD="shasum -a 256"
else
  echo "Warning: No sha256 tool found, skipping checksum verification"
  SHA_CMD=""
fi

if [ -n "${SHA_CMD}" ]; then
  echo "Verifying checksum..."
  curl -fsSL "${CHECKSUMS_URL}" -o /tmp/rings-checksums.txt
  EXPECTED=$(grep "${BINARY}" /tmp/rings-checksums.txt | awk '{print $1}')
  ACTUAL=$(${SHA_CMD} /tmp/rings-download | awk '{print $1}')
  if [ "${EXPECTED}" != "${ACTUAL}" ]; then
    echo "Error: Checksum mismatch!"
    echo "  Expected: ${EXPECTED}"
    echo "  Got:      ${ACTUAL}"
    rm -f /tmp/rings-download /tmp/rings-checksums.txt
    exit 1
  fi
  rm -f /tmp/rings-checksums.txt
  echo "Checksum verified."
fi

# Install
if [ -w "$(dirname "${DEST}")" ]; then
  mv /tmp/rings-download "${DEST}"
else
  sudo mv /tmp/rings-download "${DEST}"
fi

echo "Installed rings to ${DEST}"
"${DEST}" --version
