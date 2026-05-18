# GitHub CI: All-in-one multi-arch Dockerfile (no China mirrors)
# Uses official Debian, Node.js, and npm sources.
# Pre-built cc-proxy binary must be placed in release/ directory before building.
#
# Multi-arch build (via GitHub Actions):
#   docker buildx build --platform linux/amd64,linux/arm64 ...

FROM python:3.12-slim-bookworm AS base

# Install git + download tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    curl \
    ca-certificates \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install Node.js 20.x (official binaries)
ARG TARGETARCH=amd64
ENV NODE_VERSION=20.19.0
RUN if [ "$TARGETARCH" = "arm64" ]; then \
      NODE_ARCH="arm64"; \
    else \
      NODE_ARCH="x64"; \
    fi && \
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${NODE_ARCH}.tar.xz" -o node.tar.xz \
    && tar -xJf node.tar.xz -C /usr/local --strip-components=1 \
    && rm node.tar.xz

# Install Claude Code CLI (locked version)
RUN npm install -g @anthropic-ai/claude-code@2.1.100

# Copy pre-built cc-proxy binary for the target architecture
COPY release/cc-proxy-${TARGETARCH} /usr/local/bin/cc-proxy
RUN chmod +x /usr/local/bin/cc-proxy

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default config directory (user mounts config.yaml here)
ENV CC_SWITCH_CONFIG_DIR=/etc/cc-proxy

ENTRYPOINT ["/entrypoint.sh"]