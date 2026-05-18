#!/usr/bin/env bash
# Build multica-daemon application image (cc-proxy + Claude CLI on top of base image)
#
# Prerequisites:
#   1. Build the base image first: ./build-base.sh (x86_64) or ./build-base-arm64.sh
#   2. Place pre-built cc-proxy binary in release/cc-proxy-amd64 or release/cc-proxy-arm64
#
# Usage:
#   ./build.sh              # build x86_64 (default)
#   ./build.sh amd64        # build x86_64
#   ./build.sh arm64        # build arm64

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "${SCRIPT_DIR}"

ARCH="${1:-amd64}"

# Map architecture to base image name and binary filename
if [[ "${ARCH}" == "arm64" ]]; then
    BASE_IMAGE="multica-daemon-base-arm64:latest"
    BINARY="release/cc-proxy-arm64"
    FINAL_IMAGE="multica-daemon-arm64"
elif [[ "${ARCH}" == "amd64" ]]; then
    BASE_IMAGE="multica-daemon-base:latest"
    BINARY="release/cc-proxy-amd64"
    FINAL_IMAGE="multica-daemon"
else
    echo "Error: unsupported architecture '${ARCH}'. Use 'amd64' or 'arm64'."
    exit 1
fi

IMAGE_TAG="${2:-latest}"

# Check that pre-built binary exists
if [[ ! -f "${BINARY}" ]]; then
    echo "Error: pre-built binary '${BINARY}' not found."
    echo "Place the cc-proxy binary for ${ARCH} in ${SCRIPT_DIR}/${BINARY} before building."
    echo "Build it with: cross build --release --target <rust-target> && cp target/<rust-target>/release/cc-proxy ${BINARY}"
    exit 1
fi

echo "==> Building ${FINAL_IMAGE}:${IMAGE_TAG} (${ARCH})..."
echo "    Base image: ${BASE_IMAGE}"
echo "    Binary: ${BINARY}"

docker build \
  -f Dockerfile \
  --build-arg BASE_IMAGE="${BASE_IMAGE}" \
  --build-arg TARGETARCH="${ARCH}" \
  -t "${FINAL_IMAGE}:${IMAGE_TAG}" \
  .

echo ""
echo "✓ Done"
docker images "${FINAL_IMAGE}" --format "table={{.Repository}}\t{{.Tag}}\t{{.Size}}"
