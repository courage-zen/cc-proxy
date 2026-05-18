#!/usr/bin/env bash
# Build multica-daemon-base-arm64 image from Dockerfile.base.claude.arm64
#
# This is the ARM64 base image: uv + python + Node.js (arm64) + Claude Code + git
# The final multica-daemon-arm64 image can be built on top via Dockerfile.
#
# Usage:
#   ./build-base-arm64.sh              # build latest
#   ./build-base-arm64.sh v2           # build with custom tag

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "${SCRIPT_DIR}"

IMAGE_TAG="${1:-latest}"
IMAGE_NAME="multica-daemon-base-arm64"

echo "==> Building ${IMAGE_NAME}:${IMAGE_TAG}..."
docker build \
  -f Dockerfile.base.claude.arm64 \
  -t "${IMAGE_NAME}:${IMAGE_TAG}" \
  .

echo ""
echo "✓ Done"
docker images "${IMAGE_NAME}" --format "table={{.Repository}}\t{{.Tag}}\t{{.Size}}"
