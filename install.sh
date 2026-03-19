#!/usr/bin/env bash
set -euo pipefail

REPO="pinecone-io/rings"
RELEASE="nightly"

# Authentication: required for private repos.
# Set GITHUB_TOKEN env var, or have `gh` CLI authenticated.
if [ -n "${GITHUB_TOKEN:-}" ]; then
  AUTH_HEADER="Authorization: token ${GITHUB_TOKEN}"
elif command -v gh &>/dev/null; then
  GITHUB_TOKEN="$(gh auth token 2>/dev/null || true)"
  if [ -n "${GITHUB_TOKEN}" ]; then
    AUTH_HEADER="Authorization: token ${GITHUB_TOKEN}"
  else
    AUTH_HEADER=""
  fi
else
  AUTH_HEADER=""
fi

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
DEST="${1:-/usr/local/bin/rings}"

echo "Installing rings (${OS_TAG}-${ARCH_TAG})..."

# Download helper: uses GitHub API for private repos, direct URL for public
download_asset() {
  local asset_name="$1"
  local output_path="$2"

  if [ -n "${AUTH_HEADER}" ]; then
    # Use GitHub API to list release assets, find the right one, download it
    local assets_url="https://api.github.com/repos/${REPO}/releases/tags/${RELEASE}"
    local asset_id
    asset_id=$(curl -fsSL -H "${AUTH_HEADER}" "${assets_url}" \
      | grep -B 3 "\"name\": \"${asset_name}\"" \
      | grep '"id"' | head -1 | grep -o '[0-9]\+')

    if [ -z "${asset_id}" ]; then
      echo "Error: Asset '${asset_name}' not found in release '${RELEASE}'"
      exit 1
    fi

    curl -fsSL \
      -H "${AUTH_HEADER}" \
      -H "Accept: application/octet-stream" \
      "https://api.github.com/repos/${REPO}/releases/assets/${asset_id}" \
      -o "${output_path}"
  else
    # Public repo: direct download
    curl -fsSL \
      "https://github.com/${REPO}/releases/download/${RELEASE}/${asset_name}" \
      -o "${output_path}"
  fi
}

# Download binary
download_asset "${BINARY}" /tmp/rings-download
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
  download_asset "checksums-sha256.txt" /tmp/rings-checksums.txt
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
