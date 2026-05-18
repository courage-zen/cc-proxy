# GitHub CI: All-in-one multi-arch Dockerfile (no China mirrors)
# Pre-built cc-proxy binary must be placed in release/ directory before building.
#
# Multi-arch build (via GitHub Actions):
#   docker buildx build --platform linux/amd64,linux/arm64 ...

FROM node:24-bookworm-slim

# Install Python 3 + git
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 \
    python3-pip \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install Claude Code CLI (locked version)
RUN npm install -g @anthropic-ai/claude-code@2.1.100

# Copy pre-built cc-proxy binary for the target architecture
ARG TARGETARCH=amd64
COPY release/cc-proxy-${TARGETARCH} /usr/local/bin/cc-proxy
RUN chmod +x /usr/local/bin/cc-proxy

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default config directory (user mounts config.yaml here)
ENV CC_SWITCH_CONFIG_DIR=/etc/cc-proxy

ENTRYPOINT ["/entrypoint.sh"]