# Final application image: cc-proxy + Claude CLI on top of base image
# Pre-built cc-proxy binary must be placed in release/ directory before building.
#
# Multi-arch build (via GitHub Actions):
#   docker buildx build --platform linux/amd64,linux/arm64 ...
#
# Single-arch local build:
#   ./build.sh amd64
#   ./build.sh arm64

# Build args for base images per architecture (used by CI buildx)
ARG BASE_IMAGE_AMD64=multica-daemon-base:latest
ARG BASE_IMAGE_ARM64=multica-daemon-base-arm64:latest

# Select the correct base image based on TARGETARCH (auto-set by buildx)
FROM ${BASE_IMAGE_AMD64} AS base-amd64
FROM ${BASE_IMAGE_ARM64} AS base-arm64

# Conditional stage: picks the right base for the target platform
ARG TARGETARCH=amd64
FROM base-${TARGETARCH}

# Copy pre-built cc-proxy binary for the target architecture
COPY release/cc-proxy-${TARGETARCH} /usr/local/bin/cc-proxy
RUN chmod +x /usr/local/bin/cc-proxy

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default config directory (user mounts config.yaml here)
ENV CC_SWITCH_CONFIG_DIR=/etc/cc-proxy

ENTRYPOINT ["/entrypoint.sh"]